//! Code input UI.
//!
//! Generates HTML for OTP code input forms.

/// Render the code input form HTML.
pub fn render_code_form(destination: &str, error: Option<&str>) -> String {
    let error_html = match error {
        Some(msg) => format!("<div class=\"error\">{}</div>", html_escape(msg)),
        None => String::new(),
    };

    format!(
        concat!(
            "<!DOCTYPE html>\n",
            "<html lang=\"en\">\n",
            "<head>\n",
            "<meta charset=\"utf-8\">\n",
            "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n",
            "<title>Enter Code</title>\n",
            "<style>{css}</style>\n",
            "</head>\n",
            "<body>\n",
            "<div class=\"container\">\n",
            "<h1>Enter Code</h1>\n",
            "<p>A code was sent to <strong>{dest}</strong></p>\n",
            "{error}\n",
            "<form method=\"POST\">\n",
            "<input type=\"hidden\" name=\"action\" value=\"verify\">\n",
            "<input type=\"hidden\" name=\"destination\" value=\"{dest_raw}\">\n",
            "<div class=\"field\"><label for=\"code\">Verification Code</label>",
            "<input type=\"text\" id=\"code\" name=\"code\" required maxlength=\"6\" pattern=\"[0-9]{{6}}\" inputmode=\"numeric\" autocomplete=\"one-time-code\" autofocus placeholder=\"000000\">",
            "</div>\n",
            "<button type=\"submit\" class=\"submit\">Verify</button>\n",
            "</form>\n",
            "</div>\n",
            "</body>\n",
            "</html>",
        ),
        css = FORM_CSS,
        dest = html_escape(destination),
        dest_raw = html_escape(destination),
        error = error_html,
    )
}

/// Render the code request form HTML (enter email/phone).
pub fn render_code_request_form(error: Option<&str>) -> String {
    let error_html = match error {
        Some(msg) => format!("<div class=\"error\">{}</div>", html_escape(msg)),
        None => String::new(),
    };

    format!(
        concat!(
            "<!DOCTYPE html>\n",
            "<html lang=\"en\">\n",
            "<head>\n",
            "<meta charset=\"utf-8\">\n",
            "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n",
            "<title>Sign In with Code</title>\n",
            "<style>{css}</style>\n",
            "</head>\n",
            "<body>\n",
            "<div class=\"container\">\n",
            "<h1>Sign In with Code</h1>\n",
            "<p>We'll send you a one-time code.</p>\n",
            "{error}\n",
            "<form method=\"POST\">\n",
            "<input type=\"hidden\" name=\"action\" value=\"request\">\n",
            "<div class=\"field\"><label for=\"destination\">Email or Phone</label>",
            "<input type=\"text\" id=\"destination\" name=\"destination\" required autofocus>",
            "</div>\n",
            "<button type=\"submit\" class=\"submit\">Send Code</button>\n",
            "</form>\n",
            "</div>\n",
            "</body>\n",
            "</html>",
        ),
        css = FORM_CSS,
        error = error_html,
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
h1{margin-bottom:1rem;font-size:1.5rem;text-align:center}\
p{margin-bottom:1rem;text-align:center;color:#666}\
.error{background:#fef2f2;border:1px solid #fca5a5;color:#991b1b;padding:.75rem;border-radius:6px;margin-bottom:1rem;font-size:.875rem}\
.field{margin-bottom:1rem}\
label{display:block;margin-bottom:.25rem;font-size:.875rem;font-weight:500}\
input[type=text]{width:100%;padding:.625rem;border:1px solid #ddd;border-radius:6px;font-size:1.25rem;text-align:center;letter-spacing:.5rem}\
input:focus{outline:none;border-color:#3b82f6;box-shadow:0 0 0 3px rgba(59,130,246,.1)}\
.submit{width:100%;padding:.75rem;background:#3b82f6;color:#fff;border:none;border-radius:6px;font-size:1rem;cursor:pointer;margin-top:.5rem}\
.submit:hover{background:#2563eb}";
