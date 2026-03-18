use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;

/// Isolated test environment with temp directories for all paths.
#[allow(dead_code)]
pub struct TestEnv {
    pub root: TempDir,
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub home_dir: PathBuf,
    pub claude_dir: PathBuf,
    pub live_creds: PathBuf,
}

impl TestEnv {
    pub fn new() -> Self {
        let root = TempDir::new().unwrap();
        let data_dir = root.path().join("data");
        let config_dir = root.path().join("config");
        let home_dir = root.path().join("home");
        let claude_dir = home_dir.join(".claude");
        let live_creds = claude_dir.join(".credentials.json");

        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&claude_dir).unwrap();

        Self {
            root,
            data_dir,
            config_dir,
            home_dir,
            claude_dir,
            live_creds,
        }
    }

    /// Build a Command with all env vars set for isolation.
    pub fn cmd(&self) -> Command {
        let mut c = Command::cargo_bin("claude-revolver").unwrap();
        c.env("CLAUDE_REVOLVER_DATA_DIR", &self.data_dir)
            .env("CLAUDE_REVOLVER_CONFIG_DIR", &self.config_dir)
            .env("HOME", &self.home_dir);
        c
    }

    /// Build a Command with mock HTTP server URL.
    pub fn cmd_with_mock(&self, server: &mockito::Server) -> Command {
        let mut c = self.cmd();
        c.env(
            "CLAUDE_REVOLVER_USAGE_API_URL",
            format!("{}/api/oauth/usage", server.url()),
        );
        c
    }

    /// Build a Command with fake claude binary in PATH.
    pub fn cmd_with_fake_claude(&self) -> Command {
        let bin_dir = self.install_fake_claude();
        let mut c = self.cmd();
        let path = std::env::var("PATH").unwrap_or_default();
        c.env("PATH", format!("{}:{}", bin_dir.display(), path));
        c
    }

    // ── Setup ──────────────────────────────────────────────────────

    /// Create an account with fake credentials.
    pub fn add_account(&self, name: &str) {
        let dir = self.data_dir.join(name);
        fs::create_dir_all(&dir).unwrap();
        let creds = fake_credentials(name);
        let path = dir.join("credentials.json");
        fs::write(&path, serde_json::to_string_pretty(&creds).unwrap()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    /// Set the active account.
    pub fn set_active(&self, name: &str) {
        fs::write(self.data_dir.join("active"), name).unwrap();
    }

    /// Copy account credentials to the live credentials path.
    pub fn set_live_credentials(&self, name: &str) {
        let src = self.data_dir.join(name).join("credentials.json");
        fs::copy(&src, &self.live_creds).unwrap();
        fs::set_permissions(&self.live_creds, fs::Permissions::from_mode(0o600)).unwrap();
    }

    /// Write custom live credentials.
    pub fn write_live_credentials(&self, creds: &Value) {
        fs::write(&self.live_creds, serde_json::to_string_pretty(creds).unwrap()).unwrap();
        fs::set_permissions(&self.live_creds, fs::Permissions::from_mode(0o600)).unwrap();
    }

    /// Write usage cache.
    pub fn write_usage_cache(&self, cache: &Value) {
        let path = self.data_dir.join("usage-cache.json");
        fs::write(&path, serde_json::to_string_pretty(cache).unwrap()).unwrap();
    }

    /// Write config.
    pub fn write_config(&self, config: &Value) {
        let path = self.config_dir.join("config.json");
        fs::write(&path, serde_json::to_string_pretty(config).unwrap()).unwrap();
    }

    /// Write sessions.
    pub fn write_sessions(&self, sessions: &Value) {
        let path = self.data_dir.join("sessions.json");
        fs::write(&path, serde_json::to_string_pretty(sessions).unwrap()).unwrap();
    }

    /// Write a PID-namespaced signal file.
    pub fn write_signal(&self, wrapper_pid: u32, name: &str, content: &Value) {
        let dir = self.data_dir.join("signals");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{wrapper_pid}-{name}"));
        fs::write(&path, serde_json::to_string_pretty(content).unwrap()).unwrap();
    }

    /// Check if a signal file exists.
    pub fn signal_exists(&self, wrapper_pid: u32, name: &str) -> bool {
        self.data_dir
            .join("signals")
            .join(format!("{wrapper_pid}-{name}"))
            .exists()
    }

    /// Read a signal file.
    pub fn read_signal(&self, wrapper_pid: u32, name: &str) -> Value {
        let path = self.data_dir
            .join("signals")
            .join(format!("{wrapper_pid}-{name}"));
        let content = fs::read_to_string(&path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    // ── Assertions ─────────────────────────────────────────────────

    /// Read the active account name.
    pub fn read_active(&self) -> String {
        fs::read_to_string(self.data_dir.join("active"))
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    /// Read live credentials as JSON.
    pub fn read_live_creds(&self) -> Value {
        let content = fs::read_to_string(&self.live_creds).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    /// Read stored credentials for an account.
    pub fn read_stored_creds(&self, name: &str) -> Value {
        let path = self.data_dir.join(name).join("credentials.json");
        let content = fs::read_to_string(&path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    /// Read usage cache.
    pub fn read_usage_cache(&self) -> Value {
        let path = self.data_dir.join("usage-cache.json");
        let content = fs::read_to_string(&path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    /// Read sessions.
    pub fn read_sessions(&self) -> Value {
        let path = self.data_dir.join("sessions.json");
        let content = fs::read_to_string(&path).unwrap();
        serde_json::from_str(&content).unwrap()
    }


    /// Read swap history.
    #[allow(dead_code)]
    pub fn read_swap_history(&self) -> Value {
        let path = self.data_dir.join("swap-history.json");
        let content = fs::read_to_string(&path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    /// Write swap history.
    #[allow(dead_code)]
    pub fn write_swap_history(&self, entries: &Value) {
        let path = self.data_dir.join("swap-history.json");
        fs::write(&path, serde_json::to_string_pretty(entries).unwrap()).unwrap();
    }

    /// Write Claude Code settings.json.
    #[allow(dead_code)]
    pub fn write_settings(&self, settings: &Value) {
        let path = self.claude_dir.join("settings.json");
        fs::write(&path, serde_json::to_string_pretty(settings).unwrap()).unwrap();
    }

    /// Read Claude Code settings.json.
    #[allow(dead_code)]
    pub fn read_settings(&self) -> Value {
        let path = self.claude_dir.join("settings.json");
        let content = fs::read_to_string(&path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    /// Read fake claude invocation log.
    pub fn read_claude_invocations(&self) -> Vec<String> {
        let path = self.data_dir.join("claude-invocations.log");
        if !path.exists() {
            return Vec::new();
        }
        fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|l| l.to_string())
            .collect()
    }

    // ── Internal ───────────────────────────────────────────────────

    fn install_fake_claude(&self) -> PathBuf {
        let bin_dir = self.home_dir.join(".local").join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let script = bin_dir.join("claude");
        fs::write(
            &script,
            r#"#!/bin/bash
DATA_DIR="$CLAUDE_REVOLVER_DATA_DIR"
echo "$@" >> "$DATA_DIR/claude-invocations.log"
[ -f "$DATA_DIR/staged-swap-info" ] && cp "$DATA_DIR/staged-swap-info" "$DATA_DIR/swap-info"
[ -f "$DATA_DIR/staged-rate-limited" ] && cp "$DATA_DIR/staged-rate-limited" "$DATA_DIR/rate-limited"
rm -f "$DATA_DIR/staged-swap-info" "$DATA_DIR/staged-rate-limited"
exit ${FAKE_CLAUDE_EXIT_CODE:-0}
"#,
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        bin_dir
    }
}

// ── Fixture generators ─────────────────────────────────────────────────

/// Generate fake credentials for an account.
pub fn fake_credentials(name: &str) -> Value {
    json!({
        "claudeAiOauth": {
            "accessToken": format!("sk-ant-oat01-fake-{name}"),
            "refreshToken": format!("sk-ant-ort01-fake-{name}"),
            "expiresAt": 9999999999999u64,
            "scopes": ["user:inference"],
            "subscriptionType": "max",
            "rateLimitTier": "default_claude_max_20x"
        }
    })
}

/// Generate a usage cache with entries.
/// Each tuple: (account_name, five_hour_util, seven_day_util).
pub fn fake_usage_cache(entries: &[(&str, f64, f64)]) -> Value {
    let mut map = serde_json::Map::new();
    for (name, u5, u7) in entries {
        map.insert(
            name.to_string(),
            fake_usage_cache_entry(*u5, *u7, false),
        );
    }
    Value::Object(map)
}

/// Generate a single usage cache entry.
pub fn fake_usage_cache_entry(five_hour: f64, seven_day: f64, token_expired: bool) -> Value {
    json!({
        "five_hour": {
            "utilization": five_hour,
            "resets_at": "2026-03-18T10:00:00Z"
        },
        "seven_day": {
            "utilization": seven_day,
            "resets_at": "2026-03-22T00:00:00Z"
        },
        "polled_at": "2026-03-18T05:00:00Z",
        "token_expired": token_expired
    })
}

/// Generate a usage cache with expired token support.
pub fn fake_usage_cache_with_expired(entries: &[(&str, f64, f64, bool)]) -> Value {
    let mut map = serde_json::Map::new();
    for (name, u5, u7, expired) in entries {
        map.insert(
            name.to_string(),
            fake_usage_cache_entry(*u5, *u7, *expired),
        );
    }
    Value::Object(map)
}

/// Generate a usage API response for mock HTTP.
pub fn fake_api_response(five_hour: f64, seven_day: f64) -> Value {
    json!({
        "five_hour": {
            "utilization": five_hour,
            "resets_at": "2026-03-18T10:00:00Z"
        },
        "seven_day": {
            "utilization": seven_day,
            "resets_at": "2026-03-22T00:00:00Z"
        }
    })
}

/// Generate a config JSON with overrides merged into defaults.
pub fn fake_config(overrides: Value) -> Value {
    let mut base = json!({
        "poll_interval_seconds": 60,
        "thresholds": {
            "five_hour": 90,
            "seven_day": 95
        },
        "strategy": {
            "type": "drain",
            "order": []
        },
        "auto_resume": true,
        "auto_message": "Go continue.",
        "notify": true
    });
    merge_json(&mut base, &overrides);
    base
}

fn merge_json(base: &mut Value, overlay: &Value) {
    if let (Some(base_obj), Some(overlay_obj)) = (base.as_object_mut(), overlay.as_object()) {
        for (key, val) in overlay_obj {
            if val.is_object() && base_obj.get(key).map_or(false, |v| v.is_object()) {
                merge_json(base_obj.get_mut(key).unwrap(), val);
            } else {
                base_obj.insert(key.clone(), val.clone());
            }
        }
    }
}
