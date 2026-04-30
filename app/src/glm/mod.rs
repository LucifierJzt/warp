//! GLM Assistant — fork-only AI panel that talks directly to 智谱 GLM
//! Coding Plan via its OpenAI-compatible HTTP endpoint.
//!
//! Independent of Warp's existing Agent Mode (`crate::ai`,
//! `crate::ai_assistant`); coexists peacefully. See
//! `specs/FORK-ZHIPU-AI/PRODUCT.md` for product rationale and
//! `specs/FORK-ZHIPU-AI/TECH.md` for the implementation plan.
//!
//! Module layout:
//! - [`types`]: wire-level shapes (chat messages, requests, SSE chunks).
//! - [`settings`]: API key (Keychain) + non-secret settings (UserPreferences).
//! - [`client`]: streaming OpenAI-compatible HTTP client (added in Step 1.2).
//! - [`conversation`]: multi-turn conversation `Model` (added in Step 1.3).
//! - [`panel`]: WarpUI side-panel `View` (added in Step 1.4).

pub mod client;
pub mod conversation;
pub mod settings;
pub mod types;
