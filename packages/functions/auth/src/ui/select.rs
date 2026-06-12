//! Provider selection UI.
//!
//! Generates HTML for provider selection page.

/// Generate the provider selection HTML page.
pub fn render_provider_select(providers: &[ProviderInfo], session_key: &str) -> String {
    let mut buttons = String::new();
    for p in providers {
        let icon = match p.provider_type.as_str() {
            "oidc" | "oauth2" => "&#x1F310;",
            "password" => "&#x1F512;",
            "code" => "&#x2709;",
            _ => "&#x2022;",
        };
        buttons.push_str(&format!(
            r#"<a class="btn" href="/{}/authorize?session={}">{} {}</a>"#,
            html_escape(&p.name),
            html_escape(session_key),
            icon,
            html_escape(&p.display_name),
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Sign In</title>
<style>{}</style>
</head>
<body>
<div class="container">
<h1>Sign In</h1>
<p>Choose a sign-in method:</p>
<div class="providers">{}</div>
</div>
</body>
</html>"#,
        COMMON_CSS, buttons,
    )
}

/// Provider information for UI display
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub name: String,
    pub display_name: String,
    pub provider_type: String,
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const COMMON_CSS: &str = r#"
*{box-sizing:border-box;margin:0;padding:0}
body{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;background:#f5f5f5;color:#333;display:flex;justify-content:center;align-items:center;min-height:100vh}
.container{background:#fff;border-radius:8px;box-shadow:0 2px 8px rgba(0,0,0,.1);padding:2rem;max-width:400px;width:100%}
h1{margin-bottom:1rem;font-size:1.5rem;text-align:center}
p{margin-bottom:1rem;text-align:center;color:#666}
.providers{display:flex;flex-direction:column;gap:.75rem}
.btn{display:block;padding:.75rem 1rem;border:1px solid #ddd;border-radius:6px;text-decoration:none;color:#333;text-align:center;font-size:1rem;transition:background .15s}
.btn:hover{background:#f0f0f0}
"#;
