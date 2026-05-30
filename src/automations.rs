//! Automation rules.
//!
//! Automations are regex-matched rules that automatically run modules
//! without showing a GUI dialog. They are evaluated after the main pipeline.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::config::find_config_file;

/// Automation rule loaded from JSON (linear order preserved)
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AutomationRule {
    /// Regex pattern to match against the URL (or array of patterns)
    pub regex: AutomationRegex,

    /// Action to take ("open", or a module ID to run)
    pub action: String,

    /// Optional browser desktop ID (without .desktop suffix). If set, opens in this browser.
    #[serde(default)]
    pub browser: Option<String>,

    /// Optional additional arguments (module-specific)
    #[serde(default)]
    pub args: serde_json::Value,

    /// Whether to stop processing further automations after this one
    #[serde(default)]
    pub stop: bool,

    /// Whether this automation is enabled (default true)
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// Regex can be a single string or an array of strings
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum AutomationRegex {
    Single(String),
    Multiple(Vec<String>),
}

impl AutomationRegex {
    /// Check if any of the regex patterns match the URL
    pub fn is_match(&self, url: &str) -> bool {
        match self {
            AutomationRegex::Single(r) => Regex::new(r).map(|re| re.is_match(url)).unwrap_or(false),
            AutomationRegex::Multiple(patterns) => patterns.iter().any(|r| {
                Regex::new(r).map(|re| re.is_match(url)).unwrap_or(false)
            }),
        }
    }
}

impl TryFrom<serde_json::Value> for AutomationRule {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

/// Load the automation catalog from the config file.
pub fn load_catalog() -> AutomationCatalog {
    let path = find_config_file("automations.json")
        .unwrap_or_else(|| "automations.json".into());
    AutomationCatalog::from_file(path.to_str().unwrap()).unwrap_or_else(|e| {
        eprintln!("Warning: Could not load automations.json: {e}");
        AutomationCatalog::default()
    })
}

/// The automation catalog (lazy-loaded for runtime use)
static CATALOG: LazyLock<AutomationCatalog> = LazyLock::new(load_catalog);

/// Parsed automation catalog (preserves insertion order)
#[derive(Debug, Default)]
pub struct AutomationCatalog {
    /// Rules in JSON key order
    rules: Vec<(String, AutomationRule)>,
}

impl AutomationCatalog {
    fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let data = std::fs::read_to_string(path)?;
        // Use serde_json::Value to preserve key order, then deserialize
        let raw: serde_json::Value = serde_json::from_str(&data)?;
        if let serde_json::Value::Object(map) = raw {
            let rules: Vec<(String, AutomationRule)> = map
                .into_iter()
                .filter_map(|(name, value)| {
                    match value.try_into() {
                        Ok(rule) => Some((name, rule)),
                        Err(e) => {
                            eprintln!("Warning: Invalid automation rule '{name}': {e}");
                            None
                        }
                    }
                })
                .collect();
            Ok(Self { rules })
        } else {
            Ok(Self::default())
        }
    }

    /// Find the first matching automation for a URL (linear order, stop on first match).
    pub fn check(&self, url: &str) -> Option<(String, &AutomationRule)> {
        for (name, rule) in &self.rules {
            if !rule.enabled {
                continue;
            }
            if rule.regex.is_match(url) {
                return Some((name.clone(), rule));
            }
        }
        None
    }
}

/// Execute the first matching automation for a URL.
pub fn execute_automations(url: &str) -> Option<(String, AutomationRule)> {
    CATALOG.check(url).map(|(name, rule)| (name, rule.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_loads() {
        // Build a catalog from inline test data instead of live config
        let json = r#"{
            "Open Twitter/X in Tor": {
                "regex": "twitter\\\\.com|xcancel\\\\.com",
                "action": "open",
                "browser": "torbrowser"
            },
            "Open everything else in LibreWolf": {
                "regex": ".*",
                "action": "open",
                "browser": "librewolf"
            }
        }"#;
        let raw: serde_json::Value = serde_json::from_str(json).unwrap();
        let catalog = if let serde_json::Value::Object(map) = raw {
            let rules: Vec<(String, AutomationRule)> = map
                .into_iter()
                .filter_map(|(name, value)| value.try_into().ok().map(|r| (name, r)))
                .collect();
            AutomationCatalog { rules }
        } else {
            AutomationCatalog::default()
        };

        // The catch-all rule should match
        let result = catalog.check("https://example.com");
        assert!(result.is_some(), "Default automation should match all URLs");
        let (name, rule) = result.unwrap();
        assert_eq!(name, "Open everything else in LibreWolf");
        assert!(rule.enabled);
    }

    #[test]
    fn test_first_match_wins() {
        // Twitter should match the first rule, not the catch-all
        let result = CATALOG.check("https://twitter.com/test");
        assert!(result.is_some());
        let (name, _) = result.unwrap();
        assert_eq!(name, "Open Twitter/X in Tor");
    }

    #[test]
    fn test_automation_regex_array() {
        // Test that regex can be an array
        let rule: AutomationRule = serde_json::from_str(r#"{
            "regex": ["example\\.com", "test\\.org"],
            "action": "open"
        }"#).unwrap();
        assert!(rule.regex.is_match("https://example.com/path"));
        assert!(rule.regex.is_match("https://test.org/page"));
        assert!(!rule.regex.is_match("https://other.com"));
    }

    #[test]
    fn test_automation_browser_field() {
        let rule: AutomationRule = serde_json::from_str(r#"{
            "regex": "example\\.com",
            "action": "open",
            "browser": "librewolf"
        }"#).unwrap();
        assert_eq!(rule.browser, Some("librewolf".to_string()));
    }

    #[test]
    fn test_automation_no_browser_field() {
        let rule: AutomationRule = serde_json::from_str(r#"{
            "regex": "example\\.com",
            "action": "open"
        }"#).unwrap();
        assert_eq!(rule.browser, None);
    }

    #[test]
    fn test_automation_stop_field() {
        let rule: AutomationRule = serde_json::from_str(r#"{
            "regex": "example\\.com",
            "action": "open",
            "stop": true
        }"#).unwrap();
        assert!(rule.stop);
    }

    #[test]
    fn test_automation_default_stop() {
        let rule: AutomationRule = serde_json::from_str(r#"{
            "regex": "example\\.com",
            "action": "open"
        }"#).unwrap();
        assert!(!rule.stop);
    }

    #[test]
    fn test_automation_args() {
        let rule: AutomationRule = serde_json::from_str(r#"{
            "regex": "example\\.com",
            "action": "open",
            "args": {"key": "value", "number": 42}
        }"#).unwrap();
        assert!(rule.args.is_object());
        assert_eq!(rule.args["key"], "value");
        assert_eq!(rule.args["number"], 42);
    }

    #[test]
    fn test_automation_disabled() {
        let rule: AutomationRule = serde_json::from_str(r#"{
            "regex": "example\\.com",
            "action": "open",
            "enabled": false
        }"#).unwrap();
        assert!(!rule.enabled);
    }

    #[test]
    fn test_automation_default_enabled() {
        let rule: AutomationRule = serde_json::from_str(r#"{
            "regex": "example\\.com",
            "action": "open"
        }"#).unwrap();
        assert!(rule.enabled);
    }
}
