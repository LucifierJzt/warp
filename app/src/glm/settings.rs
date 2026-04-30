//! Persistence for the GLM Assistant panel.
//!
//! Two stores:
//!
//! - **Secret (API key):** macOS Keychain via `warpui_extras::secure_storage`.
//!   Same backend Warp already uses for `AiApiKeys`. Never written to
//!   `settings.yaml`, never logged.
//! - **Non-secret (base URL, model, system prompt, temperature, max_tokens):**
//!   `private_user_preferences` (JSON-serialized, single key).
//!
//! Defaults match 智谱 GLM Coding Plan documentation as of 2026-04:
//! - base URL: `https://open.bigmodel.cn/api/coding/paas/v4`
//! - model:    `glm-4.6`
//! No system prompt / temperature / max_tokens by default — the provider
//! picks reasonable defaults.

use serde::{Deserialize, Serialize};
use warp_core::user_preferences::GetUserPreferences;
use warpui_extras::secure_storage::{self, AppContextExt, Error as SecureStorageError};

/// Default endpoint per https://docs.bigmodel.cn (2026-04). Coding-Plan-only;
/// the *generic* bigmodel endpoint is `.../api/paas/v4` (no `coding/`).
pub const DEFAULT_BASE_URL: &str = "https://open.bigmodel.cn/api/coding/paas/v4";

/// A safe coding model. Operator can pick another (`GLM-4.7`, `GLM-5`, ...)
/// from the settings page.
pub const DEFAULT_MODEL: &str = "glm-4.6";

/// Keychain key for the API key. Distinct from the `AiApiKeys` key used by
/// Warp's existing BYOK system so the two never alias.
pub const API_KEY_STORAGE_KEY: &str = "GlmAssistantApiKey";

/// `private_user_preferences` key for the non-secret config blob.
pub const SETTINGS_PREFS_KEY: &str = "GlmAssistantSettings";

/// Non-secret configuration. Persisted as a JSON blob in
/// `private_user_preferences`. Adding optional fields is forward-compatible
/// because `serde(default)` will fill them on read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlmSettings {
    pub base_url: String,
    pub model: String,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

impl Default for GlmSettings {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
            system_prompt: None,
            temperature: None,
            max_tokens: None,
        }
    }
}

impl GlmSettings {
    /// Load from `private_user_preferences`. Falls back to defaults silently
    /// on missing key or parse failure (logs a warning).
    pub fn load(ctx: &dyn GetUserPreferences) -> Self {
        let raw = match ctx.private_user_preferences().read_value(SETTINGS_PREFS_KEY) {
            Ok(value) => value,
            Err(err) => {
                log::warn!(
                    "GLM Assistant: failed to read {SETTINGS_PREFS_KEY} from preferences: {err:?}",
                );
                None
            }
        };
        let Some(raw) = raw else { return Self::default() };
        match serde_json::from_str::<Self>(&raw) {
            Ok(parsed) => parsed,
            Err(err) => {
                log::warn!(
                    "GLM Assistant: stored settings are not valid JSON ({err}); using defaults",
                );
                Self::default()
            }
        }
    }

    /// Persist to `private_user_preferences`. Returns `Err` only on
    /// JSON-serialization failure (which would be a bug — this struct
    /// always serializes cleanly).
    pub fn save(&self, ctx: &dyn GetUserPreferences) -> Result<(), serde_json::Error> {
        let serialized = serde_json::to_string(self)?;
        if let Err(err) = ctx
            .private_user_preferences()
            .write_value(SETTINGS_PREFS_KEY, serialized)
        {
            log::warn!("GLM Assistant: failed to persist settings: {err:?}");
        }
        Ok(())
    }

    /// Trimmed base URL with any trailing `/` removed. Combined endpoint URL
    /// is built by appending `/chat/completions`.
    pub fn chat_completions_url(&self) -> String {
        let trimmed = self.base_url.trim_end_matches('/');
        format!("{trimmed}/chat/completions")
    }
}

/// Reads the API key from secure storage. Returns `Ok(None)` if no key has
/// ever been saved (vs. `Err` for actual storage failures the operator
/// should know about).
pub fn load_api_key(ctx: &warpui::AppContext) -> Result<Option<String>, SecureStorageError> {
    match ctx.secure_storage().read_value(API_KEY_STORAGE_KEY) {
        Ok(value) => Ok(Some(value)),
        Err(secure_storage::Error::NotFound) => Ok(None),
        Err(err) => Err(err),
    }
}

/// Writes the API key to secure storage. Empty `key` is rejected — use
/// [`clear_api_key`] to remove.
pub fn save_api_key(ctx: &warpui::AppContext, key: &str) -> Result<(), SecureStorageError> {
    if key.is_empty() {
        return Err(SecureStorageError::Unknown(anyhow::anyhow!(
            "refusing to save empty GLM API key; call clear_api_key instead",
        )));
    }
    ctx.secure_storage()
        .write_value(API_KEY_STORAGE_KEY, key)
}

/// Removes the API key from secure storage. Idempotent — succeeds whether
/// or not a key was previously stored.
pub fn clear_api_key(ctx: &warpui::AppContext) -> Result<(), SecureStorageError> {
    match ctx.secure_storage().remove_value(API_KEY_STORAGE_KEY) {
        Ok(()) => Ok(()),
        Err(secure_storage::Error::NotFound) => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_zhipu_coding_plan() {
        let s = GlmSettings::default();
        assert_eq!(s.base_url, DEFAULT_BASE_URL);
        assert_eq!(s.model, DEFAULT_MODEL);
        assert!(s.system_prompt.is_none());
    }

    #[test]
    fn url_assembly_strips_trailing_slash() {
        let s = GlmSettings {
            base_url: "https://example.test/api/v4/".into(),
            ..GlmSettings::default()
        };
        assert_eq!(
            s.chat_completions_url(),
            "https://example.test/api/v4/chat/completions"
        );
    }

    #[test]
    fn url_assembly_handles_no_trailing_slash() {
        let s = GlmSettings {
            base_url: "https://example.test/api/v4".into(),
            ..GlmSettings::default()
        };
        assert_eq!(
            s.chat_completions_url(),
            "https://example.test/api/v4/chat/completions"
        );
    }

    #[test]
    fn settings_roundtrip_through_json_with_partial_fields() {
        // Forward-compat: missing optional fields fall back to defaults.
        let raw = r#"{"base_url":"https://x/y","model":"glm-4.7"}"#;
        let parsed: GlmSettings = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.base_url, "https://x/y");
        assert_eq!(parsed.model, "glm-4.7");
        assert!(parsed.system_prompt.is_none());
        assert!(parsed.temperature.is_none());
    }

    #[test]
    fn empty_json_object_yields_default_settings() {
        let parsed: GlmSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, GlmSettings::default());
    }
}
