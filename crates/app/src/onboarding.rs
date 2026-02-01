//! Onboarding flow for the Interactive Preview Companion feature.
//!
//! This module provides the first-run onboarding experience that:
//! - Welcomes new users
//! - Requests terminal/command execution permissions
//! - Checks and installs required dependencies
//! - Verifies the setup is working

use serde::{Deserialize, Serialize};
use shared::settings::AppSettings;

/// Current step in the onboarding flow
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnboardingStep {
    Welcome,
    TerminalPermission,
    DependencyCheck,
    DependencyInstall { name: String, status: InstallStatus },
    Verification,
    Complete,
}

impl Default for OnboardingStep {
    fn default() -> Self {
        OnboardingStep::Welcome
    }
}

/// Status of a dependency installation
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallStatus {
    Pending,
    Installing,
    Installed,
    Failed(String),
    Skipped,
}

/// Status of a single dependency
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DependencyStatus {
    /// Name of the dependency (e.g., "wkhtmltoimage", "curl")
    pub name: String,
    /// Whether this dependency is required vs nice-to-have
    pub required: bool,
    /// Whether it was detected on the system
    pub detected: bool,
    /// Version if detected
    pub version: Option<String>,
    /// Command to install if missing
    pub install_command: Option<String>,
}

/// Current state of the onboarding flow
#[derive(Clone, Debug)]
pub struct OnboardingState {
    pub step: OnboardingStep,
    pub terminal_approved: bool,
    pub dependencies: Vec<DependencyStatus>,
    pub verification_result: Option<Result<(), String>>,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            step: OnboardingStep::Welcome,
            terminal_approved: false,
            dependencies: Vec::new(),
            verification_result: None,
        }
    }
}

/// Result of completing onboarding
#[derive(Clone, Debug)]
pub struct OnboardingResult {
    pub terminal_enabled: bool,
    pub dependencies_installed: Vec<String>,
    pub verification_passed: bool,
}

/// The onboarding flow controller
pub struct OnboardingFlow {
    state: OnboardingState,
}

impl OnboardingFlow {
    /// Create a new onboarding flow
    pub fn new() -> Self {
        Self {
            state: OnboardingState::default(),
        }
    }

    /// Check if onboarding is needed
    pub fn is_needed(settings: &AppSettings) -> bool {
        !settings.user_profile.onboarding_complete
    }

    /// Get current step
    pub fn current_step(&self) -> &OnboardingStep {
        &self.state.step
    }

    /// Get current state
    pub fn state(&self) -> &OnboardingState {
        &self.state
    }

    /// Move to next step
    pub fn next(&mut self) {
        self.state.step = match &self.state.step {
            OnboardingStep::Welcome => OnboardingStep::TerminalPermission,
            OnboardingStep::TerminalPermission => OnboardingStep::DependencyCheck,
            OnboardingStep::DependencyCheck => {
                // Check if any dependencies need installation
                let needs_install = self
                    .state
                    .dependencies
                    .iter()
                    .any(|d| d.required && !d.detected);
                if needs_install {
                    if let Some(dep) = self
                        .state
                        .dependencies
                        .iter()
                        .find(|d| d.required && !d.detected)
                    {
                        OnboardingStep::DependencyInstall {
                            name: dep.name.clone(),
                            status: InstallStatus::Pending,
                        }
                    } else {
                        OnboardingStep::Verification
                    }
                } else {
                    OnboardingStep::Verification
                }
            }
            OnboardingStep::DependencyInstall { .. } => {
                // Check if more dependencies need installation
                let installed: Vec<_> = self
                    .state
                    .dependencies
                    .iter()
                    .filter(|d| d.detected)
                    .map(|d| d.name.clone())
                    .collect();

                if let Some(dep) = self
                    .state
                    .dependencies
                    .iter()
                    .find(|d| d.required && !d.detected && !installed.contains(&d.name))
                {
                    OnboardingStep::DependencyInstall {
                        name: dep.name.clone(),
                        status: InstallStatus::Pending,
                    }
                } else {
                    OnboardingStep::Verification
                }
            }
            OnboardingStep::Verification => OnboardingStep::Complete,
            OnboardingStep::Complete => OnboardingStep::Complete,
        };
    }

    /// Go back to previous step
    pub fn back(&mut self) {
        self.state.step = match &self.state.step {
            OnboardingStep::Welcome => OnboardingStep::Welcome,
            OnboardingStep::TerminalPermission => OnboardingStep::Welcome,
            OnboardingStep::DependencyCheck => OnboardingStep::TerminalPermission,
            OnboardingStep::DependencyInstall { .. } => OnboardingStep::DependencyCheck,
            OnboardingStep::Verification => OnboardingStep::DependencyCheck,
            OnboardingStep::Complete => OnboardingStep::Verification,
        };
    }

    /// Skip current step (if allowed)
    pub fn skip(&mut self) {
        match &self.state.step {
            OnboardingStep::TerminalPermission => {
                self.state.terminal_approved = false;
                self.next();
            }
            OnboardingStep::DependencyInstall { name, .. } => {
                // Mark as skipped
                if let Some(dep) = self.state.dependencies.iter_mut().find(|d| &d.name == name) {
                    dep.detected = true; // Treat as "handled"
                }
                self.next();
            }
            _ => self.next(),
        }
    }

    /// Approve terminal access
    pub fn approve_terminal(&mut self) {
        self.state.terminal_approved = true;
    }

    /// Deny terminal access
    pub fn deny_terminal(&mut self) {
        self.state.terminal_approved = false;
    }

    /// Set dependency check results
    pub fn set_dependencies(&mut self, deps: Vec<DependencyStatus>) {
        self.state.dependencies = deps;
    }

    /// Mark a dependency as installed
    pub fn mark_installed(&mut self, name: &str) {
        if let Some(dep) = self.state.dependencies.iter_mut().find(|d| d.name == name) {
            dep.detected = true;
        }
    }

    /// Set verification result
    pub fn set_verification_result(&mut self, result: Result<(), String>) {
        self.state.verification_result = Some(result);
    }

    /// Complete onboarding
    pub fn complete(&mut self) -> OnboardingResult {
        self.state.step = OnboardingStep::Complete;

        OnboardingResult {
            terminal_enabled: self.state.terminal_approved,
            dependencies_installed: self
                .state
                .dependencies
                .iter()
                .filter(|d| d.detected)
                .map(|d| d.name.clone())
                .collect(),
            verification_passed: self
                .state
                .verification_result
                .as_ref()
                .map(|r| r.is_ok())
                .unwrap_or(false),
        }
    }

    /// Render onboarding UI
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Clone step to avoid borrow conflicts
        let step = self.state.step.clone();

        ui.vertical_centered(|ui| match step {
            OnboardingStep::Welcome => self.render_welcome(ui),
            OnboardingStep::TerminalPermission => self.render_terminal_permission(ui),
            OnboardingStep::DependencyCheck => self.render_dependency_check(ui),
            OnboardingStep::DependencyInstall { name, status } => {
                self.render_dependency_install(ui, &name, &status)
            }
            OnboardingStep::Verification => self.render_verification(ui),
            OnboardingStep::Complete => self.render_complete(ui),
        });
    }

    fn render_welcome(&mut self, ui: &mut egui::Ui) {
        ui.heading("Welcome to Little Helper!");
        ui.add_space(20.0);

        ui.label("Your friendly AI assistant for finding files, fixing problems,");
        ui.label("researching topics, analyzing data, and creating content.");

        ui.add_space(20.0);

        ui.label("Let's get you set up. This will only take a minute.");

        ui.add_space(30.0);

        if ui.button("Get Started").clicked() {
            self.next();
        }
    }

    fn render_terminal_permission(&mut self, ui: &mut egui::Ui) {
        ui.heading("Terminal Access");
        ui.add_space(20.0);

        ui.label("Little Helper can run commands on your behalf to:");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("  • Search for files and folders");
        });
        ui.horizontal(|ui| {
            ui.label("  • Check system status");
        });
        ui.horizontal(|ui| {
            ui.label("  • Install software (with your approval)");
        });
        ui.horizontal(|ui| {
            ui.label("  • Fix problems automatically");
        });

        ui.add_space(20.0);

        ui.colored_label(
            egui::Color32::from_rgb(100, 149, 237),
            "All potentially dangerous commands require your explicit approval.",
        );

        ui.add_space(30.0);

        ui.horizontal(|ui| {
            if ui.button("Allow Terminal Access").clicked() {
                self.approve_terminal();
                self.next();
            }

            ui.add_space(10.0);

            if ui.button("Skip (Limited Mode)").clicked() {
                self.deny_terminal();
                self.next();
            }
        });
    }

    fn render_dependency_check(&mut self, ui: &mut egui::Ui) {
        ui.heading("Checking Dependencies");
        ui.add_space(20.0);

        ui.label("Checking for required tools...");

        ui.add_space(20.0);

        for dep in &self.state.dependencies {
            ui.horizontal(|ui| {
                let status = if dep.detected { "✓" } else { "✗" };
                let color = if dep.detected {
                    egui::Color32::GREEN
                } else if dep.required {
                    egui::Color32::RED
                } else {
                    egui::Color32::YELLOW
                };

                ui.colored_label(color, status);
                ui.label(&dep.name);

                if let Some(version) = &dep.version {
                    ui.small(version);
                }
            });
        }

        ui.add_space(30.0);

        ui.horizontal(|ui| {
            if ui.button("Continue").clicked() {
                self.next();
            }

            if ui.button("Re-check").clicked() {
                // TODO: Trigger dependency re-check
            }
        });
    }

    fn render_dependency_install(&mut self, ui: &mut egui::Ui, name: &str, status: &InstallStatus) {
        ui.heading(format!("Installing {}", name));
        ui.add_space(20.0);

        match status {
            InstallStatus::Pending => {
                ui.label(format!("{} is required but not installed.", name));
                ui.add_space(20.0);

                if ui.button("Install").clicked() {
                    // TODO: Trigger installation
                }

                if ui.button("Skip").clicked() {
                    self.skip();
                }
            }
            InstallStatus::Installing => {
                ui.label("Installing...");
                ui.spinner();
            }
            InstallStatus::Installed => {
                ui.colored_label(egui::Color32::GREEN, "Successfully installed!");
                ui.add_space(10.0);
                if ui.button("Continue").clicked() {
                    self.next();
                }
            }
            InstallStatus::Failed(error) => {
                ui.colored_label(egui::Color32::RED, "Installation failed:");
                ui.label(error);
                ui.add_space(10.0);

                if ui.button("Retry").clicked() {
                    // TODO: Retry installation
                }

                if ui.button("Skip").clicked() {
                    self.skip();
                }
            }
            InstallStatus::Skipped => {
                ui.label("Skipped");
                if ui.button("Continue").clicked() {
                    self.next();
                }
            }
        }
    }

    fn render_verification(&mut self, ui: &mut egui::Ui) {
        ui.heading("Verifying Setup");
        ui.add_space(20.0);

        match &self.state.verification_result {
            None => {
                ui.label("Running a quick test to make sure everything works...");
                ui.spinner();
                // TODO: Trigger verification
            }
            Some(Ok(())) => {
                ui.colored_label(egui::Color32::GREEN, "Everything is working!");
                ui.add_space(20.0);

                if ui.button("Finish Setup").clicked() {
                    self.next();
                }
            }
            Some(Err(error)) => {
                ui.colored_label(egui::Color32::RED, "Verification failed:");
                ui.label(error);
                ui.add_space(10.0);

                if ui.button("Retry").clicked() {
                    self.state.verification_result = None;
                }

                if ui.button("Continue Anyway").clicked() {
                    self.next();
                }
            }
        }
    }

    fn render_complete(&mut self, ui: &mut egui::Ui) {
        ui.heading("You're All Set!");
        ui.add_space(20.0);

        ui.label("Little Helper is ready to help you.");

        ui.add_space(20.0);

        // Summary
        ui.label("Setup Summary:");
        ui.add_space(10.0);

        let terminal_status = if self.state.terminal_approved {
            ("✓", "Terminal access enabled", egui::Color32::GREEN)
        } else {
            (
                "✗",
                "Terminal access disabled (limited mode)",
                egui::Color32::YELLOW,
            )
        };

        ui.horizontal(|ui| {
            ui.colored_label(terminal_status.2, terminal_status.0);
            ui.label(terminal_status.1);
        });

        let deps_installed = self
            .state
            .dependencies
            .iter()
            .filter(|d| d.detected)
            .count();
        let deps_total = self.state.dependencies.len();

        ui.horizontal(|ui| {
            ui.colored_label(egui::Color32::GREEN, "✓");
            ui.label(format!(
                "{}/{} dependencies ready",
                deps_installed, deps_total
            ));
        });

        ui.add_space(30.0);

        if ui.button("Start Using Little Helper").clicked() {
            // Signal completion to main app
        }
    }
}

impl Default for OnboardingFlow {
    fn default() -> Self {
        Self::new()
    }
}

/// Check for a specific dependency
pub async fn check_dependency(name: &str) -> DependencyStatus {
    let (command, required) = match name {
        "wkhtmltoimage" => ("wkhtmltoimage --version", false),
        "curl" => ("curl --version", true),
        "wsl" => ("wsl --status", false), // Windows only
        _ => (name, false),
    };

    // Try to run the command
    let detected = tokio::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" })
        .args(if cfg!(windows) {
            vec!["/C", command]
        } else {
            vec!["-c", command]
        })
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    DependencyStatus {
        name: name.to_string(),
        required,
        detected,
        version: None,
        install_command: get_install_command(name),
    }
}

/// Check all required dependencies
pub async fn check_all_dependencies() -> Vec<DependencyStatus> {
    let deps = if cfg!(windows) {
        vec!["curl", "wsl", "wkhtmltoimage"]
    } else {
        vec!["curl", "wkhtmltoimage"]
    };

    let mut results = Vec::new();
    for dep in deps {
        results.push(check_dependency(dep).await);
    }
    results
}

/// Get the install command for a dependency
fn get_install_command(name: &str) -> Option<String> {
    match name {
        "wkhtmltoimage" => {
            if cfg!(target_os = "macos") {
                Some("brew install wkhtmltopdf".to_string())
            } else if cfg!(target_os = "linux") {
                Some("sudo apt-get install wkhtmltopdf".to_string())
            } else {
                Some("Download from https://wkhtmltopdf.org/downloads.html".to_string())
            }
        }
        "curl" => {
            if cfg!(target_os = "macos") {
                Some("brew install curl".to_string())
            } else if cfg!(target_os = "linux") {
                Some("sudo apt-get install curl".to_string())
            } else {
                Some("winget install curl".to_string())
            }
        }
        "wsl" => Some("wsl --install".to_string()),
        _ => None,
    }
}

/// Attempt to install a dependency
pub async fn install_dependency(name: &str) -> Result<(), String> {
    let install_cmd = get_install_command(name)
        .ok_or_else(|| format!("No install command known for {}", name))?;

    // This is a placeholder - actual installation would need platform-specific handling
    // and possibly elevated permissions
    Err(format!(
        "Automatic installation not yet implemented. Please run manually: {}",
        install_cmd
    ))
}
