//! Password form UI.
//!
//! Generates HTML for password login/registration forms.

/// Form mode
#[derive(Debug, Clone, Copy)]
pub enum PasswordFormMode {
    Login,
    Register,
    ForgotPassword,
    ChangePassword,
}

/// Render the password form HTML.
pub fn render_password_form(mode: PasswordFormMode, error: Option<&str>) -> String {
    todo!("Implement password form UI")
}
