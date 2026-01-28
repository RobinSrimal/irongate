//! Provider selection UI.
//!
//! Generates HTML for provider selection page.

/// Generate the provider selection HTML page.
pub fn render_provider_select(providers: &[ProviderInfo], redirect_uri: &str) -> String {
    todo!("Implement provider selection UI")
}

/// Provider information for UI display
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub name: String,
    pub display_name: String,
    pub provider_type: String,
}
