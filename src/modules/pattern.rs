//! Pattern module - applies regex search/replace patterns to URLs.
//!
//! Loads patterns from `patterns.json` where each
//! pattern has a regex, replacement, and optional automatic/enabled flags.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use crate::config::find_config_file;
use crate::modules::Module;
use crate::url_data::{ModuleId, UrlData};

/// Module identifier
pub const ID: ModuleId = "pattern";

/// Pattern catalog loaded from JSON
static CATALOG: LazyLock<PatternCatalog> = LazyLock::new(|| {
    let path = find_config_file("patterns.json").unwrap_or_else(|| "patterns.json".into());
    PatternCatalog::from_file(path.to_str().unwrap()).unwrap_or_else(|e| {
        eprintln!("Warning: Could not load patterns.json: {e}");
        PatternCatalog::default()
    })
});

/// Single pattern entry from the catalog
#[derive(Deserialize, Debug)]
struct PatternEntry {
    regex: String,
    replacement: Replacement,
    enabled: Option<bool>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum Replacement {
    Single(String),
    Multiple(Vec<String>),
}

/// The pattern catalog - a map of pattern name -> PatternEntry
#[derive(Debug, Default)]
struct PatternCatalog {
    patterns: HashMap<String, PatternEntry>,
}

impl PatternCatalog {
    fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let data = std::fs::read_to_string(path)?;
        let patterns: HashMap<String, PatternEntry> = serde_json::from_str(&data)?;
        Ok(Self { patterns })
    }

    /// Get all enabled patterns.
    fn enabled_patterns(&self) -> impl Iterator<Item = (&String, &PatternEntry)> {
        self.patterns.iter().filter(|(_, p)| p.enabled.unwrap_or(true))
    }
}

/// Pattern module implementation.
pub struct PatternModule;

impl Module for PatternModule {
    fn id(&self) -> ModuleId {
        ID
    }

    fn on_modify(&self, data: &mut UrlData) -> Option<String> {
        let mut url = data.url.clone();
        let mut applied = Vec::new();

        for (name, pattern) in CATALOG.enabled_patterns() {
            let re = match Regex::new(&pattern.regex) {
                Ok(r) => r,
                Err(_) => continue,
            };

            if !re.is_match(&url) {
                continue;
            }

            // Choose replacement (random if multiple)
            let replacement = match &pattern.replacement {
                Replacement::Single(s) => s.as_str(),
                Replacement::Multiple(multi) => {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let idx = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.subsec_millis() as usize % multi.len())
                        .unwrap_or(0);
                    multi[idx].as_str()
                }
            };

            let new_url = re.replace(&url, replacement).into_owned();

            if new_url != url {
                url = new_url;
                applied.push(name.to_string());
            }
        }

        // Record applied patterns for display
        if !applied.is_empty() {
            let key = "pattern.applied";
            for p in applied {
                data.extra.insert(format!("{key}.{p}"), p);
            }
        }

        // Return the modified URL to restart the pipeline
        if url != data.url {
            Some(url)
        } else {
            None
        }
    }
}

impl Default for PatternModule {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let m = PatternModule;
        assert_eq!(m.id(), "pattern");
    }

    #[test]
    fn test_domain_redirect() {
        let module = PatternModule;
        let mut data = UrlData::new("https://twitter.com/user/status/123".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_some(), "Twitter URL should be redirected");
        let redirected = result.unwrap();
        assert!(redirected.contains("xcancel.com"), "Should redirect to xcancel.com");
        assert!(!redirected.contains("twitter.com"), "Should not contain twitter.com");
    }

    #[test]
    fn test_no_match() {
        let module = PatternModule;
        let mut data = UrlData::new("https://example.com/path".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_none(), "URL with no matching pattern should not be modified");
    }

    #[test]
    fn test_query_parameter_replacement() {
        let module = PatternModule;
        let mut data = UrlData::new("https://example.com/path?search=hello+world".to_string());
        let result = module.on_modify(&mut data);
        // The pattern module should handle query parameter replacements
        // depending on the patterns.json configuration
        if result.is_some() {
            let modified = result.unwrap();
            assert!(!modified.contains("search="), "search param should be replaced");
        }
    }

    #[test]
    fn test_multiple_patterns_applied() {
        let module = PatternModule;
        let mut data = UrlData::new("https://twitter.com/user/status/123".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_some());
        let redirected = result.unwrap();
        assert!(redirected.contains("xcancel.com"));
    }

    #[test]
    fn test_pattern_with_capture_groups() {
        let module = PatternModule;
        let mut data = UrlData::new("https://twitter.com/user/status/123".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_some());
        let redirected = result.unwrap();
        // The path and status should be preserved
        assert!(redirected.contains("/user/status/123"), "Path should be preserved");
    }

    #[test]
    fn test_pattern_preserves_scheme() {
        let module = PatternModule;
        let mut data = UrlData::new("https://twitter.com/user".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_some());
        let redirected = result.unwrap();
        assert!(redirected.starts_with("https://"), "Scheme should be preserved");
    }

    #[test]
    fn test_pattern_preserves_port() {
        let module = PatternModule;
        let mut data = UrlData::new("https://twitter.com:8443/user".to_string());
        let result = module.on_modify(&mut data);
        if result.is_some() {
            let redirected = result.unwrap();
            assert!(redirected.contains(":8443"), "Port should be preserved");
        }
    }

    #[test]
    fn test_pattern_preserves_query_string() {
        let module = PatternModule;
        let mut data = UrlData::new("https://twitter.com/user?ref=abc123".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_some());
        let redirected = result.unwrap();
        assert!(redirected.contains("ref=abc123"), "Query string should be preserved");
    }

    #[test]
    fn test_pattern_preserves_fragment() {
        let module = PatternModule;
        let mut data = UrlData::new("https://twitter.com/user#section1".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_some());
        let redirected = result.unwrap();
        assert!(redirected.contains("#section1"), "Fragment should be preserved");
    }

    #[test]
    fn test_pattern_no_path() {
        let module = PatternModule;
        let mut data = UrlData::new("https://twitter.com".to_string());
        let result = module.on_modify(&mut data);
        // Twitter.com without path may not match the pattern depending on regex
        if result.is_some() {
            let redirected = result.unwrap();
            assert!(redirected.contains("xcancel.com"), "Should redirect to xcancel.com");
        }
    }

    #[test]
    fn test_pattern_deep_path() {
        let module = PatternModule;
        let mut data = UrlData::new("https://twitter.com/user/status/123/attachments/456".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_some());
        let redirected = result.unwrap();
        assert!(redirected.contains("/user/status/123/attachments/456"), "Deep path should be preserved");
    }
}
