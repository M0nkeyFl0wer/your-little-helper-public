//! Password dialog for sudo/admin commands.
//!
//! Provides a secure password entry dialog using egui with:
//! - Masked input (shows dots)
//! - Secure memory handling with zeroize
//! - Enter key submission
//! - Escape key cancellation

use super::{Modal, ModalResult};
use egui::{Align2, Area, Context, Id, Key, RichText, TextEdit, Vec2};
use zeroize::Zeroizing;

/// Password dialog for sudo/admin commands.
pub struct PasswordDialog {
    /// Whether the dialog is open
    is_open: bool,
    /// The password being entered (securely zeroed on drop)
    password: Zeroizing<String>,
    /// The result of the dialog
    result: ModalResult<String>,
    /// Message to show the user
    message: String,
    /// Error message (e.g., "incorrect password")
    error: Option<String>,
    /// Dialog ID for egui
    id: Id,
}

impl PasswordDialog {
    /// Create a new password dialog.
    pub fn new(id: impl std::hash::Hash) -> Self {
        Self {
            is_open: false,
            password: Zeroizing::new(String::new()),
            result: ModalResult::Pending,
            message: String::new(),
            error: None,
            id: Id::new(id),
        }
    }

    /// Open the dialog with a message.
    pub fn open_with_message(&mut self, message: impl Into<String>) {
        self.is_open = true;
        self.message = message.into();
        self.password = Zeroizing::new(String::new());
        self.result = ModalResult::Pending;
        self.error = None;
    }

    /// Set an error message (e.g., after wrong password).
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
        // Clear password for retry
        self.password = Zeroizing::new(String::new());
    }

    /// Get the result (consumes the password securely).
    pub fn take_result(&mut self) -> ModalResult<String> {
        std::mem::replace(&mut self.result, ModalResult::Pending)
    }

    /// Check if there's a result ready.
    pub fn has_result(&self) -> bool {
        !self.result.is_pending()
    }
}

impl Modal for PasswordDialog {
    fn update(&mut self, ctx: &Context) -> bool {
        if !self.is_open {
            return false;
        }

        let mut should_close = false;

        // Semi-transparent background overlay
        Area::new(self.id.with("overlay"))
            .anchor(Align2::LEFT_TOP, Vec2::ZERO)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();
                ui.allocate_response(screen_rect.size(), egui::Sense::click());
                ui.painter()
                    .rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(180));
            });

        // Main dialog window
        egui::Window::new("🔐 Password Required")
            .id(self.id.with("window"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(350.0);

                ui.add_space(8.0);

                // Message
                if !self.message.is_empty() {
                    ui.label(&self.message);
                    ui.add_space(8.0);
                }

                // Error message
                if let Some(ref error) = self.error {
                    ui.colored_label(egui::Color32::RED, error);
                    ui.add_space(8.0);
                }

                // Password input
                ui.horizontal(|ui| {
                    ui.label("Password:");
                    let response = ui.add(
                        TextEdit::singleline(&mut *self.password)
                            .password(true)
                            .desired_width(200.0)
                            .hint_text("Enter password..."),
                    );

                    // Focus the text input
                    if response.gained_focus() || self.error.is_some() {
                        response.request_focus();
                    }

                    // Handle Enter key
                    if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        if !self.password.is_empty() {
                            // Take ownership of password and secure transfer
                            let password = std::mem::replace(&mut *self.password, String::new());
                            self.result = ModalResult::Confirmed(password);
                            should_close = true;
                        }
                    }
                });

                ui.add_space(12.0);

                // Buttons
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                        self.result = ModalResult::Cancelled;
                        should_close = true;
                    }

                    ui.add_space(8.0);

                    let submit_enabled = !self.password.is_empty();
                    if ui
                        .add_enabled(submit_enabled, egui::Button::new("Submit"))
                        .clicked()
                    {
                        let password = std::mem::replace(&mut *self.password, String::new());
                        self.result = ModalResult::Confirmed(password);
                        should_close = true;
                    }
                });

                ui.add_space(8.0);

                // Security note
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    ui.label(RichText::new("🔒 Password is not stored").small().weak());
                });
            });

        // Handle escape key globally
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.result = ModalResult::Cancelled;
            should_close = true;
        }

        if should_close {
            self.is_open = false;
            // Securely clear password
            self.password = Zeroizing::new(String::new());
        }

        should_close
    }

    fn is_open(&self) -> bool {
        self.is_open
    }

    fn open(&mut self) {
        self.open_with_message("Enter your password to continue:");
    }

    fn close(&mut self) {
        self.is_open = false;
        self.password = Zeroizing::new(String::new());
        self.result = ModalResult::Cancelled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_dialog_creation() {
        let dialog = PasswordDialog::new("test");
        assert!(!dialog.is_open());
    }

    #[test]
    fn test_password_secure_zeroing() {
        let mut dialog = PasswordDialog::new("test");
        dialog.open_with_message("Test");

        // Simulate entering a password
        *dialog.password = "secret123".to_string();

        // Close should clear the password
        dialog.close();

        // Password should be cleared (empty string)
        assert!(dialog.password.is_empty());
    }
}
