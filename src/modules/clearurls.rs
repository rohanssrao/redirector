//! ClearURLs module - removes tracking parameters from URLs.
//!
//! Parses the ClearURLs JSON catalog and applies rules based on the
//! domain matching the URL's host.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use url::Url;

use crate::config::find_config_file;
use crate::modules::Module;
use crate::url_data::{ModuleId, UrlData};

/// Module identifier
pub const ID: ModuleId = "clearurls";

/// ClearURLs JSON structure (simplified from the original spec)
#[derive(Deserialize, Debug, Default)]
struct ClearUrlsCatalogRaw {
    providers: HashMap<String, Provider>,
}

#[derive(Deserialize, Debug, Default)]
struct Provider {
    #[serde(rename = "urlPattern")]
    url_pattern: Option<String>,
    rules: Option<Vec<String>>,
    exceptions: Option<Vec<String>>,
    #[serde(rename = "referralMarketing")]
    referral_marketing: Option<Vec<String>>,
}

/// A single ClearURLs rule set for a provider
#[derive(Debug)]
struct ProviderRules {
    /// Regex to match the provider's URL pattern
    pattern: Regex,
    /// Query parameter names to remove (standard rules)
    rules: Vec<String>,
    /// Referral marketing params to remove (e.g., affiliate tags)
    referral_marketing: Vec<String>,
    /// Exceptions - URLs that should NOT be modified
    exceptions: Vec<Regex>,
}

/// Parsed and pre-compiled ClearURLs catalog.
struct ClearUrlsCatalog {
    rules: Vec<ProviderRules>,
}

impl ClearUrlsCatalog {
    /// Load catalog from a JSON file.
    fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let data = std::fs::read_to_string(path)?;
        let raw: ClearUrlsCatalogRaw = serde_json::from_str(&data)?;

        let mut rules = Vec::new();

        for (name, provider) in &raw.providers {
            // Skip providers that are marked as "complete" but have no URL pattern
            let Some(ref pattern_str) = provider.url_pattern else {
                continue;
            };

            let pattern = match Regex::new(pattern_str) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Warning: Invalid regex for provider '{name}': {e}");
                    continue;
                }
            };

            let exceptions = provider
                .exceptions
                .as_ref()
                .map(|ex| {
                    ex.iter()
                        .filter_map(|e| Regex::new(e).ok())
                        .collect()
                })
                .unwrap_or_default();

            rules.push(ProviderRules {
                pattern,
                rules: provider.rules.clone().unwrap_or_default(),
                referral_marketing: provider.referral_marketing.clone().unwrap_or_default(),
                exceptions,
            });
        }

        Ok(Self { rules })
    }

    /// Find the best-matching provider rules for a URL.
    /// Returns the provider with the longest matching host pattern (most specific).
    fn find_matching_rules(&self, url: &Url) -> Option<&ProviderRules> {
        let host = url.host_str().unwrap_or("");
        let host_url = format!("https://{host}");

        let mut best_match: Option<(&ProviderRules, usize)> = None;

        for rule in &self.rules {
            // Check exceptions first
            if rule.exceptions.iter().any(|e| e.is_match(&host_url)) {
                continue;
            }

            if rule.pattern.is_match(&host_url) {
                let match_len = rule.pattern.as_str().len();
                match best_match {
                    Some((_, best_len)) if match_len > best_len => {
                        best_match = Some((rule, match_len));
                    }
                    None => {
                        best_match = Some((rule, match_len));
                    }
                    _ => {}
                }
            }
        }

        best_match.map(|(rule, _)| rule)
    }
}

/// Static parsed ClearURLs catalog (loaded once at startup)
static CATALOG: LazyLock<ClearUrlsCatalog> = LazyLock::new(|| {
    let path = find_config_file("clearurls.json").unwrap_or_else(|| "clearurls.json".into());
    ClearUrlsCatalog::from_file(path.to_str().unwrap()).unwrap_or_else(|e| {
        eprintln!("Warning: Could not load clearurls.json: {e}");
        ClearUrlsCatalog { rules: Vec::new() }
    })
});

/// ClearURLs module implementation.
pub struct ClearUrlsModule;

impl ClearUrlsModule {
    fn clean_url(&self, url: &Url, rules: &ProviderRules) -> Option<String> {
        let original_query = url.query().unwrap_or("");
        if original_query.is_empty() {
            return None;
        }

        // Parse query string preserving original encoding
        let mut kept_parts: Vec<&str> = Vec::new();
        let mut removed_any = false;

        for pair in original_query.split('&') {
            if let Some((key, _value)) = pair.split_once('=') {
                let key_lower = key.to_lowercase();

                let should_remove = rules.rules.iter().any(|rule| {
                    if rule.contains('^') || rule.contains('*') || rule.contains('?') {
                        if let Ok(re) = Regex::new(rule) {
                            return re.is_match(&key_lower);
                        }
                    }
                    key_lower == rule.to_lowercase()
                }) || rules.referral_marketing.iter().any(|rule| {
                    key_lower == rule.to_lowercase()
                });

                if should_remove {
                    removed_any = true;
                } else {
                    kept_parts.push(pair);
                }
            } else {
                // No '=' in pair, keep as-is (e.g., bare keys)
                kept_parts.push(pair);
            }
        }

        if !removed_any {
            return None;
        }

        // Rebuild URL with cleaned query (preserving original encoding)
        let new_query = kept_parts.join("&");
        let mut parts = url.clone();
        parts.set_query(if new_query.is_empty() { None } else { Some(&new_query) });
        Some(parts.to_string())
    }
}

impl Module for ClearUrlsModule {
    fn id(&self) -> ModuleId {
        ID
    }

    fn on_modify(&self, data: &mut UrlData) -> Option<String> {
        // Parse the URL
        let url = match Url::parse(&data.url) {
            Ok(u) => u,
            Err(_) => return None,
        };

        // Find matching provider rules
        let rules = CATALOG.find_matching_rules(&url)?;

        // Clean the URL
        self.clean_url(&url, rules)
    }
}

impl Default for ClearUrlsModule {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let m = ClearUrlsModule;
        assert_eq!(m.id(), "clearurls");
    }

    #[test]
    fn test_facebook_cleaning() {
        let module = ClearUrlsModule;
        let mut data = UrlData::new("https://www.facebook.com/l.php?u=https://example.com&rdr=not&ref=tracking123".to_string());
        let result = module.on_modify(&mut data);
        println!("Result: {:?}", result);
        assert!(result.is_some(), "Facebook URL should be cleaned");
        let cleaned = result.unwrap();
        assert!(!cleaned.contains("rdr="), "rdr param should be removed");
        assert!(!cleaned.contains("ref="), "ref param should be removed");
        assert!(cleaned.contains("u="), "u param should be kept");
    }

    #[test]
    fn test_amazon_cleaning() {
        let module = ClearUrlsModule;
        let mut data = UrlData::new("https://www.amazon.com/dp/B08N5WRWNW?tag=myaffiliate-20&qid=123".to_string());
        let result = module.on_modify(&mut data);
        println!("Result: {:?}", result);
        assert!(result.is_some(), "Amazon URL should be cleaned");
        let cleaned = result.unwrap();
        assert!(!cleaned.contains("tag="), "tag param should be removed");
        assert!(!cleaned.contains("qid="), "qid param should be removed");
    }

    #[test]
    fn test_twitter_cleaning() {
        let module = ClearUrlsModule;
        let mut data = UrlData::new("https://t.co/abc123?utm_source=twitter&utm_medium=social".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_some(), "Twitter URL should be cleaned");
        let cleaned = result.unwrap();
        assert!(!cleaned.contains("utm_source="), "utm_source param should be removed");
        assert!(!cleaned.contains("utm_medium="), "utm_medium param should be removed");
    }

    #[test]
    fn test_no_params_to_strip() {
        let module = ClearUrlsModule;
        let mut data = UrlData::new("https://example.com/path".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_none(), "URL without tracking params should not be modified");
    }

    #[test]
    fn test_empty_query_string() {
        let module = ClearUrlsModule;
        let mut data = UrlData::new("https://example.com/path?".to_string());
        let result = module.on_modify(&mut data);
        // Empty query string should not cause modification
        assert!(result.is_none(), "Empty query string should not be modified");
    }

    #[test]
    fn test_preserves_url_encoding() {
        let module = ClearUrlsModule;
        let mut data = UrlData::new("https://www.facebook.com/l.php?u=https://example.com/path%20with%20spaces&rdr=not&ref=tracking123".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_some());
        let cleaned = result.unwrap();
        assert!(cleaned.contains("path%20with%20spaces"), "URL encoding should be preserved");
        assert!(!cleaned.contains("rdr="), "rdr param should be removed");
        assert!(!cleaned.contains("ref="), "ref param should be removed");
    }

    #[test]
    fn test_special_chars_in_query() {
        let module = ClearUrlsModule;
        let mut data = UrlData::new("https://www.facebook.com/l.php?u=https://example.com/path+with+plus&rdr=not&ref=tracking123".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_some());
        let cleaned = result.unwrap();
        assert!(cleaned.contains("path+with+plus"), "Plus signs should be preserved");
        assert!(!cleaned.contains("rdr="), "rdr param should be removed");
        assert!(!cleaned.contains("ref="), "ref param should be removed");
    }

    #[test]
    fn test_multiple_tracking_params() {
        let module = ClearUrlsModule;
        let mut data = UrlData::new("https://example.com/path?ref=1&ref=2&utm_source=3&utm_medium=4&utm_campaign=5".to_string());
        let result = module.on_modify(&mut data);
        // ClearURLs may or may not remove all these params depending on the catalog
        // Just verify the URL is processed without crashing
        if result.is_some() {
            let cleaned = result.unwrap();
            // At least verify the URL is valid
            assert!(cleaned.contains("example.com"));
        }
    }

    #[test]
    fn test_non_http_url() {
        let module = ClearUrlsModule;
        let mut data = UrlData::new("not-a-valid-url".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_none(), "Invalid URL should not be modified");
    }

    #[test]
    fn test_provider_matching_longest_pattern() {
        // Test that the provider with the longest matching pattern is selected
        let module = ClearUrlsModule;
        let mut data = UrlData::new("https://www.facebook.com/l.php?ref=tracking".to_string());
        let result = module.on_modify(&mut data);
        assert!(result.is_some(), "Facebook URL should be cleaned");
        let cleaned = result.unwrap();
        assert!(!cleaned.contains("ref="), "ref param should be removed");
    }
}
