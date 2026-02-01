//! Device Capability Detection
//!
//! Analyzes system specs to recommend local vs cloud AI,
//! and suggests optimal settings for the user's hardware.
//!
//! Features:
//! - Detects RAM, CPU, GPU capabilities
//! - Calculates local LLM suitability score
//! - Recommends optimal model size (7B, 13B, 70B)
//! - Suggests when to use API vs local
//! - Provides onboarding guidance for hardware setup

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{Mode, PermissionLevel, Skill, SkillContext, SkillInput, SkillOutput};
use std::process::Command;

/// System specifications
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemSpecs {
    /// Total RAM in GB
    pub total_ram_gb: f32,
    /// Available RAM in GB
    pub available_ram_gb: f32,
    /// CPU cores
    pub cpu_cores: usize,
    /// CPU architecture
    pub cpu_arch: String,
    /// Has GPU acceleration (Metal, CUDA, ROCm)
    pub has_gpu: bool,
    /// GPU type
    pub gpu_type: Option<String>,
    /// Free disk space in GB
    pub free_disk_gb: f32,
    /// Operating system
    pub os: String,
    /// OS version
    pub os_version: String,
}

/// Local LLM capability assessment
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LocalLlmsuitability {
    /// Can run any local LLM
    pub can_run_local: bool,
    /// Recommended model size in billions of parameters
    pub recommended_model_size: ModelSize,
    /// Maximum model size possible
    pub max_model_size: ModelSize,
    /// Performance expectation
    pub performance: PerformanceLevel,
    /// Suitability score 0-100
    pub score: u8,
    /// Specific recommendations
    pub recommendations: Vec<String>,
}

/// Model size categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModelSize {
    Tiny,    // 1-3B (e.g., Phi-2, TinyLlama)
    Small,   // 7-8B (e.g., Llama 2 7B, Mistral 7B)
    Medium,  // 13-15B (e.g., Llama 2 13B)
    Large,   // 30-35B (e.g., CodeLlama 34B)
    XLarge,  // 65-70B (e.g., Llama 2 70B)
    TooBig,  // Cannot run locally
}

impl ModelSize {
    pub fn display(&self) -> &'static str {
        match self {
            ModelSize::Tiny => "1-3B parameters (Fast)",
            ModelSize::Small => "7-8B parameters (Balanced)",
            ModelSize::Medium => "13-15B parameters (Capable)",
            ModelSize::Large => "30-35B parameters (Powerful)",
            ModelSize::XLarge => "65-70B parameters (Maximum)",
            ModelSize::TooBig => "Requires cloud API",
        }
    }
    
    /// RAM required for this model (with overhead)
    pub fn ram_required_gb(&self) -> f32 {
        match self {
            ModelSize::Tiny => 2.0,
            ModelSize::Small => 6.0,
            ModelSize::Medium => 12.0,
            ModelSize::Large => 24.0,
            ModelSize::XLarge => 48.0,
            ModelSize::TooBig => 80.0,
        }
    }
    
    /// VRAM required for GPU acceleration
    pub fn vram_required_gb(&self) -> f32 {
        match self {
            ModelSize::Tiny => 1.5,
            ModelSize::Small => 5.0,
            ModelSize::Medium => 10.0,
            ModelSize::Large => 20.0,
            ModelSize::XLarge => 40.0,
            ModelSize::TooBig => 80.0,
        }
    }
}

/// Performance expectation
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PerformanceLevel {
    /// Fast responses, good for real-time chat
    Fast,
    /// Moderate speed, usable for most tasks
    Moderate,
    /// Slower but functional
    Slow,
    /// Too slow to be practical
    Impractical,
}

impl PerformanceLevel {
    pub fn description(&self) -> &'static str {
        match self {
            PerformanceLevel::Fast => "Fast responses (1-2 seconds)",
            PerformanceLevel::Moderate => "Moderate speed (3-5 seconds)",
            PerformanceLevel::Slow => "Slower responses (5-10 seconds)",
            PerformanceLevel::Impractical => "Too slow for regular use",
        }
    }
    
    pub fn icon(&self) -> &'static str {
        match self {
            PerformanceLevel::Fast => "⚡",
            PerformanceLevel::Moderate => "✓",
            PerformanceLevel::Slow => "⏱️",
            PerformanceLevel::Impractical => "❌",
        }
    }
}

/// Device capability assessment result
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CapabilityResult {
    /// System specifications
    pub specs: SystemSpecs,
    /// Local LLM suitability
    pub local_llm: LocalLlmsuitability,
    /// Recommendation summary
    pub recommendation: Recommendation,
}

/// High-level recommendation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Recommendation {
    /// Use local AI, API, or hybrid
    pub mode: AiMode,
    /// Suggested provider
    pub suggested_provider: String,
    /// Reasoning for recommendation
    pub reasoning: String,
    /// Priority actions for user
    pub action_items: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AiMode {
    /// Run everything locally
    LocalOnly,
    /// Use local for quick tasks, API for complex
    Hybrid,
    /// Use API for everything
    ApiOnly,
}

/// Device Capability Detector Skill
pub struct DeviceCapabilityDetector;

impl DeviceCapabilityDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detect system specifications
    fn detect_specs(&self) -> Result<SystemSpecs> {
        #[cfg(target_os = "macos")]
        return self.detect_macos_specs();
        
        #[cfg(target_os = "windows")]
        return self.detect_windows_specs();
        
        #[cfg(target_os = "linux")]
        return self.detect_linux_specs();
        
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        return Err(anyhow::anyhow!("Unsupported platform"));
    }

    /// Detect macOS specs
    #[cfg(target_os = "macos")]
    fn detect_macos_specs(&self) -> Result<SystemSpecs> {
        // Get RAM info
        let ram_output = Command::new("sysctl")
            .args(&["-n", "hw.memsize"])
            .output()?;
        let ram_bytes: u64 = String::from_utf8_lossy(&ram_output.stdout)
            .trim()
            .parse()?;
        let total_ram_gb = ram_bytes as f32 / (1024.0 * 1024.0 * 1024.0);
        
        // Get available RAM
        let vm_output = Command::new("vm_stat")
            .output()?;
        let vm_stats = String::from_utf8_lossy(&vm_output.stdout);
        let available_ram_gb = self.parse_macos_vm_stats(&vm_stats, total_ram_gb);
        
        // Get CPU cores
        let cpu_output = Command::new("sysctl")
            .args(&["-n", "hw.ncpu"])
            .output()?;
        let cpu_cores: usize = String::from_utf8_lossy(&cpu_output.stdout)
            .trim()
            .parse()?;
        
        // Get CPU architecture
        let arch_output = Command::new("uname")
            .arg("-m")
            .output()?;
        let cpu_arch = String::from_utf8_lossy(&arch_output.stdout).trim().to_string();
        
        // Check for Metal GPU
        let has_gpu = cpu_arch == "arm64" || self.check_metal_support();
        let gpu_type = if has_gpu {
            Some("Apple Silicon Metal".to_string())
        } else {
            None
        };
        
        // Get disk space
        let disk_output = Command::new("df")
            .args(&["-h", "/"])
            .output()?;
        let free_disk_gb = self.parse_disk_output(&String::from_utf8_lossy(&disk_output.stdout));
        
        // Get OS version
        let os_version = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());
        
        Ok(SystemSpecs {
            total_ram_gb,
            available_ram_gb,
            cpu_cores,
            cpu_arch,
            has_gpu,
            gpu_type,
            free_disk_gb,
            os: "macOS".to_string(),
            os_version,
        })
    }

    #[cfg(target_os = "macos")]
    fn parse_macos_vm_stats(&self, vm_stats: &str, _total_ram: f32) -> f32 {
        // Parse vm_stat output to calculate available memory
        let mut free_pages = 0u64;
        let mut inactive_pages = 0u64;
        let page_size = 4096u64; // Default page size
        
        for line in vm_stats.lines() {
            if line.contains("Pages free:") {
                if let Some(num) = line.split_whitespace().nth(2) {
                    free_pages = num.parse().unwrap_or(0);
                }
            }
            if line.contains("Pages inactive:") {
                if let Some(num) = line.split_whitespace().nth(2) {
                    inactive_pages = num.parse().unwrap_or(0);
                }
            }
        }
        
        let available_bytes = (free_pages + inactive_pages) * page_size;
        available_bytes as f32 / (1024.0 * 1024.0 * 1024.0)
    }

    #[cfg(target_os = "macos")]
    fn check_metal_support(&self) -> bool {
        // Check if Metal is available
        Command::new("system_profiler")
            .args(&["SPDisplaysDataType"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("Metal"))
            .unwrap_or(false)
    }

    /// Detect Windows specs
    #[cfg(target_os = "windows")]
    fn detect_windows_specs(&self) -> Result<SystemSpecs> {
        // Use PowerShell to get system info
        let ps_script = r#"
            $ram = (Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
            $avail = (Get-CimInstance -ClassName Win32_OperatingSystem).FreePhysicalMemory / 1MB / 1024
            $cores = (Get-CimInstance -ClassName Win32_Processor).NumberOfCores
            $arch = (Get-CimInstance -ClassName Win32_Processor).Architecture
            $disk = (Get-CimInstance -ClassName Win32_LogicalDisk -Filter "DeviceID='C:'").FreeSpace / 1GB
            $os = (Get-CimInstance -ClassName Win32_OperatingSystem).Caption
            $ver = (Get-CimInstance -ClassName Win32_OperatingSystem).Version
            $gpu = (Get-CimInstance -ClassName Win32_VideoController).Name
            
            Write-Output "RAM:$([math]::Round($ram, 2))"
            Write-Output "AVAIL:$([math]::Round($avail, 2))"
            Write-Output "CORES:$cores"
            Write-Output "ARCH:$arch"
            Write-Output "DISK:$([math]::Round($disk, 2))"
            Write-Output "OS:$os"
            Write-Output "VER:$ver"
            Write-Output "GPU:$gpu"
        "#;
        
        let output = Command::new("powershell")
            .args(&["-Command", ps_script])
            .output()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut specs = SystemSpecs {
            total_ram_gb: 8.0,
            available_ram_gb: 4.0,
            cpu_cores: 4,
            cpu_arch: "x86_64".to_string(),
            has_gpu: false,
            gpu_type: None,
            free_disk_gb: 100.0,
            os: "Windows".to_string(),
            os_version: "Unknown".to_string(),
        };
        
        for line in stdout.lines() {
            if line.starts_with("RAM:") {
                specs.total_ram_gb = line[4..].parse().unwrap_or(8.0);
            } else if line.starts_with("AVAIL:") {
                specs.available_ram_gb = line[6..].parse().unwrap_or(4.0);
            } else if line.starts_with("CORES:") {
                specs.cpu_cores = line[6..].parse().unwrap_or(4);
            } else if line.starts_with("DISK:") {
                specs.free_disk_gb = line[5..].parse().unwrap_or(100.0);
            } else if line.starts_with("OS:") {
                specs.os = line[3..].to_string();
            } else if line.starts_with("VER:") {
                specs.os_version = line[4..].to_string();
            } else if line.starts_with("GPU:") {
                let gpu = line[4..].to_string();
                specs.has_gpu = !gpu.is_empty() && gpu != "null";
                specs.gpu_type = if specs.has_gpu { Some(gpu) } else { None };
            }
        }
        
        Ok(specs)
    }

    /// Detect Linux specs
    #[cfg(target_os = "linux")]
    fn detect_linux_specs(&self) -> Result<SystemSpecs> {
        // Get RAM from /proc/meminfo
        let meminfo = std::fs::read_to_string("/proc/meminfo")?;
        let mut total_ram_kb: u64 = 0;
        let mut available_ram_kb: u64 = 0;
        
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                total_ram_kb = line.split_whitespace().nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
            if line.starts_with("MemAvailable:") {
                available_ram_kb = line.split_whitespace().nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
        }
        
        let total_ram_gb = total_ram_kb as f32 / (1024.0 * 1024.0);
        let available_ram_gb = available_ram_kb as f32 / (1024.0 * 1024.0);
        
        // Get CPU cores
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo")?;
        let cpu_cores = cpuinfo.lines().filter(|l| l.starts_with("processor")).count();
        
        // Get architecture
        let arch_output = Command::new("uname").arg("-m").output()?;
        let cpu_arch = String::from_utf8_lossy(&arch_output.stdout).trim().to_string();
        
        // Check for GPU (NVIDIA, AMD)
        let has_nvidia = std::path::Path::new("/proc/driver/nvidia/gpus").exists();
        let has_amd = std::path::Path::new("/sys/class/kfd/kfd").exists();
        let has_gpu = has_nvidia || has_amd;
        let gpu_type = if has_nvidia {
            Some("NVIDIA CUDA".to_string())
        } else if has_amd {
            Some("AMD ROCm".to_string())
        } else {
            None
        };
        
        // Get disk space
        let disk_output = Command::new("df").args(&["-h", "/"]).output()?;
        let free_disk_gb = self.parse_disk_output(&String::from_utf8_lossy(&disk_output.stdout));
        
        // Get OS version
        let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        let os_name = os_release.lines()
            .find(|l| l.starts_with("PRETTY_NAME="))
            .map(|l| l.split('=').nth(1)
                .map(|s| s.trim_matches('"').to_string())
                .unwrap_or_else(|| "Linux".to_string()))
            .unwrap_or_else(|| "Linux".to_string());
        
        Ok(SystemSpecs {
            total_ram_gb,
            available_ram_gb,
            cpu_cores,
            cpu_arch,
            has_gpu,
            gpu_type,
            free_disk_gb,
            os: os_name,
            os_version: String::new(),
        })
    }

    fn parse_disk_output(&self, output: &str) -> f32 {
        for line in output.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let size_str = parts[3];
                // Parse size like "150Gi" or "500G"
                let num: f32 = size_str.chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0.0);
                let unit = size_str.chars()
                    .skip_while(|c| c.is_ascii_digit() || *c == '.')
                    .next()
                    .unwrap_or('G');
                
                return match unit {
                    'T' | 't' => num * 1024.0,
                    'G' | 'g' => num,
                    'M' | 'm' => num / 1024.0,
                    _ => num,
                };
            }
        }
        100.0 // Default 100GB
    }

    /// Calculate local LLM suitability
    fn assess_local_llm(&self, specs: &SystemSpecs) -> LocalLlmsuitability {
        let mut score = 0u8;
        let mut can_run_local = false;
        let mut recommendations = Vec::new();
        
        // Score based on RAM
        if specs.total_ram_gb >= 48.0 {
            score += 40;
        } else if specs.total_ram_gb >= 24.0 {
            score += 30;
        } else if specs.total_ram_gb >= 16.0 {
            score += 20;
        } else if specs.total_ram_gb >= 8.0 {
            score += 10;
        } else {
            recommendations.push("Your device has limited RAM. Cloud API recommended for best experience.".to_string());
        }
        
        // Score based on CPU cores
        if specs.cpu_cores >= 8 {
            score += 25;
        } else if specs.cpu_cores >= 4 {
            score += 15;
        } else {
            score += 5;
            recommendations.push("More CPU cores would improve local AI performance.".to_string());
        }
        
        // Score based on GPU
        if specs.has_gpu {
            score += 35;
            recommendations.push(format!("Great! Your {} will accelerate AI responses.", specs.gpu_type.as_ref().unwrap()));
        } else {
            recommendations.push("No GPU detected. Responses will be slower without GPU acceleration.".to_string());
        }
        
        // Determine model sizes
        let max_model = if specs.total_ram_gb >= 48.0 && specs.has_gpu {
            can_run_local = true;
            ModelSize::XLarge
        } else if specs.total_ram_gb >= 24.0 {
            can_run_local = true;
            if specs.has_gpu {
                ModelSize::Large
            } else {
                ModelSize::Medium
            }
        } else if specs.total_ram_gb >= 12.0 {
            can_run_local = true;
            ModelSize::Small
        } else if specs.total_ram_gb >= 4.0 {
            can_run_local = true;
            ModelSize::Tiny
        } else {
            ModelSize::TooBig
        };
        
        // Recommended model (one step down for comfortable operation)
        let recommended_model = match max_model {
            ModelSize::XLarge => ModelSize::Large,
            ModelSize::Large => ModelSize::Medium,
            ModelSize::Medium => ModelSize::Small,
            ModelSize::Small => ModelSize::Tiny,
            ModelSize::Tiny => ModelSize::Tiny,
            ModelSize::TooBig => ModelSize::TooBig,
        };
        
        // Performance expectation
        let performance = if !can_run_local {
            PerformanceLevel::Impractical
        } else if specs.has_gpu && specs.total_ram_gb >= 16.0 {
            PerformanceLevel::Fast
        } else if specs.total_ram_gb >= 8.0 {
            PerformanceLevel::Moderate
        } else {
            PerformanceLevel::Slow
        };
        
        // Disk space check
        let required_disk = recommended_model.ram_required_gb() * 2.0; // Model file + overhead
        if specs.free_disk_gb < required_disk {
            recommendations.push(format!(
                "Need {:.0}GB free disk space for {} model. You have {:.0}GB free.",
                required_disk,
                recommended_model.display(),
                specs.free_disk_gb
            ));
            score = score.saturating_sub(10);
        }
        
        LocalLlmsuitability {
            can_run_local,
            recommended_model_size: recommended_model,
            max_model_size: max_model,
            performance,
            score,
            recommendations,
        }
    }

    /// Generate overall recommendation
    fn generate_recommendation(&self, _specs: &SystemSpecs, local_llm: &LocalLlmsuitability) -> Recommendation {
        let (mode, provider, reasoning) = if !local_llm.can_run_local {
            (
                AiMode::ApiOnly,
                "OpenAI GPT-3.5".to_string(),
                "Your device has limited resources. Cloud API will provide the best experience.".to_string(),
            )
        } else if local_llm.score >= 80 {
            (
                AiMode::LocalOnly,
                format!("Ollama with {} model", local_llm.recommended_model_size.display()),
                "Your device is powerful! Local AI will be fast and private.".to_string(),
            )
        } else if local_llm.score >= 50 {
            (
                AiMode::Hybrid,
                "Local for quick tasks, API for complex work".to_string(),
                "Your device can run local AI, but cloud API may be better for demanding tasks.".to_string(),
            )
        } else {
            (
                AiMode::ApiOnly,
                "OpenAI GPT-3.5 or Anthropic Claude".to_string(),
                "While local AI is possible, cloud API will provide faster responses.".to_string(),
            )
        };
        
        let mut action_items = local_llm.recommendations.clone();
        
        // Add setup instructions
        if local_llm.can_run_local {
            action_items.push("Install Ollama: https://ollama.ai".to_string());
            action_items.push(format!(
                "Download recommended model: ollama pull {}",
                self.model_name_for_size(&local_llm.recommended_model_size)
            ));
        } else {
            action_items.push("Add OpenAI API key in Settings".to_string());
            action_items.push("Or add pre-loaded key for team access".to_string());
        }
        
        Recommendation {
            mode,
            suggested_provider: provider,
            reasoning,
            action_items,
        }
    }
    
    fn model_name_for_size(&self, size: &ModelSize) -> &'static str {
        match size {
            ModelSize::Tiny => "phi",
            ModelSize::Small => "llama3.2",
            ModelSize::Medium => "llama3.1:13b",
            ModelSize::Large => "llama3.1:70b",
            ModelSize::XLarge => "mixtral",
            ModelSize::TooBig => "",
        }
    }

    /// Format results for user
    fn format_results(&self, result: &CapabilityResult) -> String {
        let mut output = String::new();
        
        output.push_str("## 💻 Your Device Analysis\n\n");
        
        // Specs summary
        let specs = &result.specs;
        output.push_str("### System Specs\n\n");
        output.push_str(&format!("🧠 **RAM:** {:.1} GB ({} GB available)\n", 
            specs.total_ram_gb, specs.available_ram_gb));
        output.push_str(&format!("⚙️ **CPU:** {} cores ({} architecture)\n", 
            specs.cpu_cores, specs.cpu_arch));
        output.push_str(&format!("💾 **Disk:** {:.1} GB free\n", specs.free_disk_gb));
        
        if specs.has_gpu {
            output.push_str(&format!("🎮 **GPU:** {}\n", specs.gpu_type.as_ref().unwrap()));
        } else {
            output.push_str("🎮 **GPU:** None detected\n");
        }
        
        output.push('\n');
        
        // Local AI assessment
        let local = &result.local_llm;
        output.push_str("### Local AI Capability\n\n");
        output.push_str(&format!("**Suitability Score:** {}/100\n\n", local.score));
        
        if local.can_run_local {
            output.push_str(&format!("{} **Status:** Can run local AI\n\n", 
                local.performance.icon()));
            output.push_str(&format!("**Recommended model:** {}\n", local.recommended_model_size.display()));
            output.push_str(&format!("**Expected performance:** {}\n\n", local.performance.description()));
        } else {
            output.push_str("❌ **Status:** Local AI not recommended\n\n");
            output.push_str("Your device has limited resources. Cloud API will work best.\n\n");
        }
        
        // Recommendations
        let rec = &result.recommendation;
        output.push_str("### Recommendation\n\n");
        output.push_str(&format!("**Mode:** {:?}\n", rec.mode));
        output.push_str(&format!("**Suggested:** {}\n\n", rec.suggested_provider));
        output.push_str(&format!("_{}_\n\n", rec.reasoning));
        
        // Action items
        if !rec.action_items.is_empty() {
            output.push_str("### Next Steps\n\n");
            for (i, item) in rec.action_items.iter().enumerate() {
                output.push_str(&format!("{}. {}\n", i + 1, item));
            }
        }
        
        output
    }
}

#[async_trait]
impl Skill for DeviceCapabilityDetector {
    fn id(&self) -> &'static str {
        "device_capability"
    }
    
    fn name(&self) -> &'static str {
        "Device Capability Check"
    }
    
    fn description(&self) -> &'static str {
        "Analyzes your device to recommend local AI vs cloud API"
    }
    
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Fix]
    }
    
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe // Read-only system info
    }
    
    async fn execute(&self, _input: SkillInput, _ctx: &SkillContext) -> anyhow::Result<SkillOutput> {
        // Detect system specs
        let specs = self.detect_specs()?;
        
        // Assess local LLM suitability
        let local_llm = self.assess_local_llm(&specs);
        
        // Generate recommendation
        let recommendation = self.generate_recommendation(&specs, &local_llm);
        
        let result = CapabilityResult {
            specs,
            local_llm,
            recommendation,
        };
        
        let formatted_text = self.format_results(&result);
        
        Ok(SkillOutput {
            result_type: shared::skill::ResultType::Text,
            text: Some(formatted_text),
            files: Vec::new(),
            data: Some(serde_json::to_value(result)?),
            citations: Vec::new(),
            suggested_actions: Vec::new(),
        })
    }
}

impl Default for DeviceCapabilityDetector {
    fn default() -> Self {
        Self::new()
    }
}