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
    let (title, action_label, extra_fields, switch_link) = match mode {
        PasswordFormMode::Login => (
            "Sign In",
            "Sign In",
            "",
            "<p class=\"switch\">Don&apos;t have an account? <a href=\"#\" onclick=\"this.closest(&quot;form&quot;).querySelector(&quot;[name=action]&quot;).value=&quot;register&quot;;this.closest(&quot;form&quot;).submit();return false;\">Register</a></p>",
        ),
        PasswordFormMode::Register => (
            "Create Account",
            "Register",
            "<div class=\"field\"><label for=\"confirm\">Confirm Password</label><input type=\"password\" id=\"confirm\" name=\"confirm_password\" required minlength=\"8\"></div>",
            "<p class=\"switch\">Already have an account? <a href=\"#\" onclick=\"this.closest(&quot;form&quot;).querySelector(&quot;[name=action]&quot;).value=&quot;login&quot;;this.closest(&quot;form&quot;).submit();return false;\">Sign In</a></p>",
        ),
        PasswordFormMode::ForgotPassword => (
            "Reset Password",
            "Send Reset Code",
            "",
            "",
        ),
        PasswordFormMode::ChangePassword => (
            "Change Password",
            "Change Password",
            "<div class=\"field\"><label for=\"new_password\">New Password</label><input type=\"password\" id=\"new_password\" name=\"new_password\" required minlength=\"8\"></div>",
            "",
        ),
    };

    let error_html = match error {
        Some(msg) => format!("<div class=\"error\">{}</div>", html_escape(msg)),
        None => String::new(),
    };

    let action_value = match mode {
        PasswordFormMode::Login => "login",
        PasswordFormMode::Register => "register",
        PasswordFormMode::ForgotPassword => "forgot",
        PasswordFormMode::ChangePassword => "change",
    };

    format!(
        concat!(
            "<!DOCTYPE html>\n",
            "<html lang=\"en\">\n",
            "<head>\n",
            "<meta charset=\"utf-8\">\n",
            "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n",
            "<title>{title}</title>\n",
            "<style>{css}</style>\n",
            "</head>\n",
            "<body>\n",
            "<div class=\"container\">\n",
            "<h1>{title}</h1>\n",
            "{error}\n",
            "<form method=\"POST\">\n",
            "<input type=\"hidden\" name=\"action\" value=\"{action_value}\">\n",
            "<div class=\"field\"><label for=\"email\">Email</label><input type=\"email\" id=\"email\" name=\"email\" required autofocus></div>\n",
            "<div class=\"field\"><label for=\"password\">Password</label><input type=\"password\" id=\"password\" name=\"password\" required minlength=\"8\"></div>\n",
            "{extra}\n",
            "<button type=\"submit\" class=\"submit\">{action_label}</button>\n",
            "</form>\n",
            "{switch}\n",
            "</div>\n",
            "</body>\n",
            "</html>",
        ),
        title = html_escape(title),
        css = FORM_CSS,
        error = error_html,
        action_value = action_value,
        extra = extra_fields,
        action_label = html_escape(action_label),
        switch = switch_link,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const FORM_CSS: &str = "\
*{box-sizing:border-box;margin:0;padding:0}\
body{font-family:-apple-system,BlinkMacSystemFont,\"Segoe UI\",Roboto,sans-serif;background:#f5f5f5;color:#333;display:flex;justify-content:center;align-items:center;min-height:100vh}\
.container{background:#fff;border-radius:8px;box-shadow:0 2px 8px rgba(0,0,0,.1);padding:2rem;max-width:400px;width:100%}\
h1{margin-bottom:1.5rem;font-size:1.5rem;text-align:center}\
.error{background:#fef2f2;border:1px solid #fca5a5;color:#991b1b;padding:.75rem;border-radius:6px;margin-bottom:1rem;font-size:.875rem}\
.field{margin-bottom:1rem}\
label{display:block;margin-bottom:.25rem;font-size:.875rem;font-weight:500}\
input[type=email],input[type=password],input[type=text]{width:100%;padding:.625rem;border:1px solid #ddd;border-radius:6px;font-size:1rem}\
input:focus{outline:none;border-color:#3b82f6;box-shadow:0 0 0 3px rgba(59,130,246,.1)}\
.submit{width:100%;padding:.75rem;background:#3b82f6;color:#fff;border:none;border-radius:6px;font-size:1rem;cursor:pointer;margin-top:.5rem}\
.submit:hover{background:#2563eb}\
.switch{text-align:center;margin-top:1rem;font-size:.875rem;color:#666}\
.switch a{color:#3b82f6;text-decoration:none}";
