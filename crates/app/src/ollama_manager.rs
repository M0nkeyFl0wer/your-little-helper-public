//! Bundled Ollama lifecycle management.
//!
//! Finds the Ollama binary shipped alongside the app, starts it on demand,
//! selects an appropriate model based on system RAM, and pulls the model
//! on first run.

use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use sysinfo::System;

/// How much RAM is available (approximately) for choosing a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RamTier {
    /// < 4 GB — too little for any useful local model
    Tiny,
    /// 4–7 GB — smallest viable model
    Low,
    /// 8–15 GB — comfortable for 3-4B models
    Medium,
    /// 16+ GB — can run 7-8B models
    High,
}

fn ram_tier() -> RamTier {
    let mut sys = System::new();
    sys.refresh_memory();
    let gb = sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
    if gb < 4.0 {
        RamTier::Tiny
    } else if gb < 8.0 {
        RamTier::Low
    } else if gb < 16.0 {
        RamTier::Medium
    } else {
        RamTier::High
    }
}

/// Detect whether this machine has GPU acceleration for LLM inference.
///
/// Apple Silicon uses Metal (always available on M-series).
/// NVIDIA GPUs use CUDA. Intel/AMD integrated graphics don't help Ollama.
pub fn has_gpu_acceleration() -> bool {
    // Apple Silicon: always has Metal GPU acceleration for Ollama
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        return true;
    }

    // Linux/Windows: check for NVIDIA GPU (CUDA) via nvidia-smi
    if cfg!(target_os = "linux") || cfg!(target_os = "windows") {
        let nvidia_cmd = if cfg!(windows) { "nvidia-smi.exe" } else { "nvidia-smi" };
        if Command::new(nvidia_cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return true;
        }

        // Also check for AMD ROCm
        if !cfg!(windows) {
            if Command::new("rocminfo")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
            {
                return true;
            }
        }
    }

    false
}

/// Pick the best default model for this machine's RAM and GPU.
///
/// On CPU-only machines, caps at 3B even with plenty of RAM because
/// larger models are painfully slow without GPU acceleration.
///
/// Returns `(model_tag, human_description)`.
pub fn recommended_model() -> (&'static str, &'static str) {
    let gpu = has_gpu_acceleration();
    match (ram_tier(), gpu) {
        (RamTier::Tiny, _) => (
            "tinyllama",
            "TinyLlama (1.1B) — lightweight, fits on low-RAM devices",
        ),
        (RamTier::Low, _) => (
            "llama3.2:1b",
            "Llama 3.2 1B — compact but capable",
        ),
        (RamTier::Medium, _) => (
            "llama3.2:3b",
            "Llama 3.2 3B — good balance of speed and quality",
        ),
        // GPU acceleration: go big
        (RamTier::High, true) => (
            "llama3.1:8b",
            "Llama 3.1 8B — best quality for local (GPU accelerated)",
        ),
        // CPU-only with lots of RAM: cap at 3B for usable speed
        (RamTier::High, false) => (
            "llama3.2:3b",
            "Llama 3.2 3B — capped for speed (no GPU detected, try a cloud provider for smarter answers)",
        ),
    }
}

/// Locate the bundled Ollama binary, if present.
///
/// Search order:
/// - macOS app bundle: `Contents/Resources/ollama`
/// - Windows / Linux: same directory as the executable (`ollama` / `ollama.exe`)
pub fn find_bundled_ollama() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    // macOS: exe is at  …/Contents/MacOS/Little Helper
    //        ollama at  …/Contents/Resources/ollama
    if cfg!(target_os = "macos") {
        if let Some(contents) = exe_dir.parent() {
            let candidate = contents.join("Resources").join("ollama");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // Windows: same dir as exe
    let name = if cfg!(windows) { "ollama.exe" } else { "ollama" };
    let candidate = exe_dir.join(name);
    if candidate.is_file() {
        return Some(candidate);
    }

    // Linux AppImage: APPDIR/usr/bin/ollama
    if let Ok(appdir) = std::env::var("APPDIR") {
        let candidate = PathBuf::from(appdir).join("usr/bin/ollama");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

/// Check if Ollama is already listening.
pub fn ollama_reachable() -> bool {
    let addr: SocketAddr = "127.0.0.1:11434".parse().unwrap();
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

/// Result of trying to ensure Ollama is running.
#[derive(Debug)]
pub enum OllamaStatus {
    /// Already running (system-installed or previously started).
    AlreadyRunning,
    /// We started the bundled copy successfully.
    Started,
    /// Ollama binary was found but failed to start.
    StartFailed(String),
    /// No Ollama binary found anywhere.
    NotFound,
}

/// Make sure Ollama is serving. If it isn't running, start the bundled copy.
pub fn ensure_ollama_running() -> OllamaStatus {
    if ollama_reachable() {
        return OllamaStatus::AlreadyRunning;
    }

    // Also try the system-installed ollama
    if try_start_system_ollama() {
        return OllamaStatus::Started;
    }

    let Some(binary) = find_bundled_ollama() else {
        return OllamaStatus::NotFound;
    };

    match start_ollama_serve(&binary) {
        Ok(()) => OllamaStatus::Started,
        Err(e) => OllamaStatus::StartFailed(e),
    }
}

/// Try starting a system-installed `ollama serve` (on PATH).
fn try_start_system_ollama() -> bool {
    let name = if cfg!(windows) { "ollama.exe" } else { "ollama" };
    if Command::new(name)
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
    {
        // Give it a moment to bind the port
        wait_for_ollama(5)
    } else {
        false
    }
}

/// Start `ollama serve` from the given binary path.
fn start_ollama_serve(binary: &PathBuf) -> Result<(), String> {
    Command::new(binary)
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start Ollama: {}", e))?;

    if wait_for_ollama(8) {
        Ok(())
    } else {
        Err("Ollama started but didn't become reachable within 8 seconds".into())
    }
}

/// Poll until Ollama is reachable or timeout (in seconds).
fn wait_for_ollama(timeout_secs: u32) -> bool {
    for _ in 0..(timeout_secs * 4) {
        if ollama_reachable() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

/// Check whether a model is already pulled locally.
///
/// Runs `ollama list` and checks if the model tag appears.
pub fn model_available(binary: &str, model: &str) -> bool {
    let output = Command::new(binary)
        .arg("list")
        .output();
    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            // `ollama list` outputs lines like "llama3.2:3b    1.9 GB  ..."
            // Match on the model name prefix (before any whitespace)
            let model_base = model.split(':').next().unwrap_or(model);
            stdout.lines().any(|line| line.starts_with(model) || line.starts_with(model_base))
        }
        Err(_) => false,
    }
}

/// Pull a model. This is blocking and can take a while.
///
/// Returns Ok with output on success.
pub fn pull_model(binary: &str, model: &str) -> Result<String, String> {
    let output = Command::new(binary)
        .args(["pull", model])
        .output()
        .map_err(|e| format!("Failed to run ollama pull: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// Find the ollama binary to use for commands (bundled or system).
pub fn ollama_binary() -> Option<String> {
    if let Some(bundled) = find_bundled_ollama() {
        return Some(bundled.to_string_lossy().to_string());
    }
    // Fall back to system PATH
    let name = if cfg!(windows) { "ollama.exe" } else { "ollama" };
    if Command::new(name).arg("--version").output().is_ok() {
        Some(name.to_string())
    } else {
        None
    }
}
