use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── Claude Code credentials ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    #[serde(rename = "claudeAiOauth")]
    pub claude_ai_oauth: OAuthCredentials,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCredentials {
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: u64,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(rename = "subscriptionType", skip_serializing_if = "Option::is_none")]
    pub subscription_type: Option<String>,
    #[serde(rename = "rateLimitTier", skip_serializing_if = "Option::is_none")]
    pub rate_limit_tier: Option<String>,
}

// ── Usage cache ─────────────────────────────────────────────────────────────

pub type UsageCache = HashMap<String, CachedAccountUsage>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAccountUsage {
    pub five_hour: Option<UsageWindow>,
    pub seven_day: Option<UsageWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seven_day_sonnet: Option<UsageWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seven_day_opus: Option<UsageWindow>,
    pub polled_at: String,
    #[serde(default)]
    pub token_expired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageWindow {
    pub utilization: f64,
    pub resets_at: String,
}

// ── Usage API response ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageApiResponse {
    pub five_hour: Option<UsageWindow>,
    pub seven_day: Option<UsageWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seven_day_sonnet: Option<UsageWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seven_day_opus: Option<UsageWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seven_day_oauth_apps: Option<UsageWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seven_day_cowork: Option<UsageWindow>,
}

// ── Swap info ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapInfo {
    pub session_id: String,
    pub from_account: String,
    pub to_account: String,
    pub reason: String,
    pub swapped_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_after: Option<String>,
}

// ── Session tracking ────────────────────────────────────────────────────────

pub type Sessions = HashMap<String, SessionEntry>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub account: String,
    pub started_at: String,
    pub source: String,
    pub cwd: String,
}

// ── Hook stdin payloads ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct StopHookInput {
    pub session_id: String,
    #[serde(default)]
    pub stop_hook_active: bool,
    #[allow(dead_code)]
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionStartHookInput {
    pub session_id: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitHookInput {
    #[serde(default)]
    pub error: Option<HookError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HookError {
    #[serde(rename = "type", default)]
    pub error_type: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}
