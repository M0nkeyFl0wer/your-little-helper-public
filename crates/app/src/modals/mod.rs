//! Modal dialogs for the application.
//!
//! Provides reusable modal components for user interaction.

pub mod password_dialog;

pub use password_dialog::PasswordDialog;

use egui::Context;

/// Trait for modal dialogs.
pub trait Modal {
    /// Update and render the modal. Returns true if the modal should close.
    fn update(&mut self, ctx: &Context) -> bool;

    /// Returns true if the modal is currently open.
    fn is_open(&self) -> bool;

    /// Open the modal.
    fn open(&mut self);

    /// Close the modal.
    fn close(&mut self);
}

/// Result from a modal dialog.
#[derive(Debug, Clone)]
pub enum ModalResult<T> {
    /// User hasn't made a decision yet
    Pending,
    /// User confirmed/submitted
    Confirmed(T),
    /// User cancelled
    Cancelled,
}

impl<T> ModalResult<T> {
    pub fn is_pending(&self) -> bool {
        matches!(self, ModalResult::Pending)
    }

    pub fn is_confirmed(&self) -> bool {
        matches!(self, ModalResult::Confirmed(_))
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, ModalResult::Cancelled)
    }

    pub fn take_value(self) -> Option<T> {
        match self {
            ModalResult::Confirmed(v) => Some(v),
            _ => None,
        }
    }
}
