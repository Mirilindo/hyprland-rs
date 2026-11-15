//! # Config mode module
//!
//! Hyprland can read its config as Lua since 0.55, and Hyprland 0.57.0 removed
//! the legacy (hyprlang) parser altogether. The parser in use decides which
//! syntax the `dispatch` request accepts: a Lua call (`hl.dsp.focus({ … })`)
//! on a Lua config, the legacy string (`focuswindow class:kitty`) otherwise.
//!
//! Hyprland reports its parser in the `status` request, as `configProvider`.
//! It picks the parser when it starts, so the answer is asked once per
//! [`crate::instance::Instance`] and then cached. One case can outdate that
//! cache: on Hyprland 0.56, `hyprctl reload full-reset` builds the config
//! manager anew, and switches parser if a `hyprland.lua` appeared meanwhile.

use crate::shared::{CommandContent, command};

/// Which config parser the running Hyprland instance uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigMode {
    /// A Lua config (`hyprland.lua`): dispatchers are Lua calls.
    Lua,
    /// The legacy hyprlang config (`hyprland.conf`).
    Legacy,
}

/// The request whose answer tells the mode. It only reads state.
pub(crate) fn status_command() -> CommandContent {
    command!(JSON, "status")
}

impl ConfigMode {
    /// Reads the answer to [`status_command`].
    ///
    /// Only an explicit `"configProvider": "lua"` means Lua. Any other
    /// provider, and any answer that is not the expected JSON (a Hyprland too
    /// old to know the request), means the legacy syntax.
    pub(crate) fn from_status_response(response: &str) -> Self {
        let provider = serde_json::from_str::<serde_json::Value>(response)
            .ok()
            .and_then(|status| status.get("configProvider")?.as_str().map(str::to_owned));
        match provider.as_deref() {
            Some("lua") => Self::Lua,
            _ => Self::Legacy,
        }
    }
}

#[test]
fn test_from_status_response() {
    // Byte for byte what Hyprland 0.57.2 answers to `j/status`.
    let lua = "\n{\n    \"configProvider\": \"lua\",\n    \"backend\": \"drm\"\n}\n";
    assert_eq!(ConfigMode::from_status_response(lua), ConfigMode::Lua);
    assert_eq!(
        ConfigMode::from_status_response(r#"{"configProvider": "hyprlang", "backend": "drm"}"#),
        ConfigMode::Legacy
    );
    assert_eq!(
        ConfigMode::from_status_response(r#"{"backend": "drm"}"#),
        ConfigMode::Legacy
    );
    assert_eq!(
        ConfigMode::from_status_response("unknown request"),
        ConfigMode::Legacy
    );
    assert_eq!(ConfigMode::from_status_response(""), ConfigMode::Legacy);
}
