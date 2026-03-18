use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::paths;
use crate::util::{atomic_write_json, ensure_dir};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,
    #[serde(default)]
    pub thresholds: Thresholds,
    #[serde(default)]
    pub strategy: StrategyConfig,
    #[serde(default = "default_true")]
    pub auto_resume: bool,
    #[serde(default = "default_auto_message")]
    pub auto_message: String,
    #[serde(default = "default_true")]
    pub notify: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thresholds {
    #[serde(default = "default_five_hour")]
    pub five_hour: u32,
    #[serde(default = "default_seven_day")]
    pub seven_day: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    #[serde(rename = "type", default = "default_strategy_type")]
    pub strategy_type: String,
    #[serde(default)]
    pub order: Vec<String>,
}

fn default_poll_interval() -> u64 {
    60
}
fn default_true() -> bool {
    true
}
fn default_auto_message() -> String {
    "Go continue.".to_string()
}
fn default_five_hour() -> u32 {
    90
}
fn default_seven_day() -> u32 {
    95
}
fn default_strategy_type() -> String {
    "drain".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            poll_interval_seconds: default_poll_interval(),
            thresholds: Thresholds::default(),
            strategy: StrategyConfig::default(),
            auto_resume: default_true(),
            auto_message: default_auto_message(),
            notify: default_true(),
        }
    }
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            five_hour: default_five_hour(),
            seven_day: default_seven_day(),
        }
    }
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            strategy_type: default_strategy_type(),
            order: Vec::new(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = paths::config_file()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = paths::config_file()?;
        ensure_dir(path.parent().unwrap(), 0o755)?;
        atomic_write_json(&path, self, 0o644)
    }

    /// Set a config value by dotted key path (e.g., "thresholds.five_hour").
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        // Serialize to Value, navigate the path, set, deserialize back
        let mut v = serde_json::to_value(&*self)?;
        let parts: Vec<&str> = key.split('.').collect();

        // Parse the value: try number, then bool, then string
        let parsed: serde_json::Value = if let Ok(n) = value.parse::<u64>() {
            serde_json::Value::Number(n.into())
        } else if let Ok(n) = value.parse::<f64>() {
            serde_json::json!(n)
        } else if let Ok(b) = value.parse::<bool>() {
            serde_json::Value::Bool(b)
        } else {
            serde_json::Value::String(value.to_string())
        };

        // Navigate to the parent and set the leaf
        let mut current = &mut v;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // Handle the "type" field which is renamed in serde
                let actual_key = if *part == "type" && i > 0 && parts[i - 1] == "strategy" {
                    "type"
                } else {
                    part
                };
                current
                    .as_object_mut()
                    .ok_or_else(|| anyhow::anyhow!("key path '{key}' is not an object at '{part}'"))?
                    .insert(actual_key.to_string(), parsed.clone());
            } else {
                current = current
                    .get_mut(*part)
                    .ok_or_else(|| anyhow::anyhow!("unknown config key: '{key}'"))?;
            }
        }

        *self = serde_json::from_value(v)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.poll_interval_seconds, 60);
        assert_eq!(config.thresholds.five_hour, 90);
        assert_eq!(config.thresholds.seven_day, 95);
        assert_eq!(config.strategy.strategy_type, "drain");
        assert!(config.auto_resume);
        assert_eq!(config.auto_message, "Go continue.");
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.thresholds.five_hour, config.thresholds.five_hour);
    }

    #[test]
    fn test_set_value() {
        let mut config = Config::default();
        config.set_value("thresholds.five_hour", "80").unwrap();
        assert_eq!(config.thresholds.five_hour, 80);

        config.set_value("auto_resume", "false").unwrap();
        assert!(!config.auto_resume);

        config
            .set_value("auto_message", "Please continue.")
            .unwrap();
        assert_eq!(config.auto_message, "Please continue.");
    }
}
