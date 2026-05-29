//! URL data container that flows through the processing pipeline.
//!
//! Mirrors the UrlData concept from URLCheck, carrying the URL string,
//! extra key-value data, and flags for controlling pipeline behavior.

use std::collections::HashMap;

/// The unique identifier for a module (e.g., "clearurls", "pattern")
pub type ModuleId = &'static str;

/// Data that flows through the URL processing pipeline.
///
/// This is mutable and shared across modules during processing.
#[derive(Debug, Clone)]
pub struct UrlData {
    /// The current URL string
    pub url: String,

    /// Extra key-value data carried across modules
    pub extra: HashMap<String, String>,

    /// The module that triggered this update (for avoiding self-loops)
    pub trigger: Option<ModuleId>,

    /// If true, the triggering module will be notified of all callbacks
    pub trigger_own: bool,

    /// If true, future URL modifications will be ignored
    pub disable_updates: bool,
}

impl UrlData {
    /// Create a new UrlData with the given URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            extra: HashMap::new(),
            trigger: None,
            trigger_own: true,
            disable_updates: false,
        }
    }

    /// Mark this URL as not triggering the originating module.
    #[allow(dead_code)]
    pub fn dont_trigger_own(mut self) -> Self {
        self.trigger_own = false;
        self
    }

    /// Disable further URL updates.
    #[allow(dead_code)]
    pub fn disable_updates(mut self) -> Self {
        self.disable_updates = true;
        self
    }

    /// Store extra data.
    #[allow(dead_code)]
    pub fn put_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }

    /// Get extra data by key.
    #[allow(dead_code)]
    pub fn get_data(&self, key: &str) -> Option<&String> {
        self.extra.get(key)
    }

    /// Get all data with a given prefix, in insertion order.
    #[allow(dead_code)]
    pub fn get_data_by_prefix(&self, prefix: &str) -> Vec<(&String, &String)> {
        self.extra
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .collect()
    }

    /// Merge data from another UrlData. Keeps [...other_data, ...this_data].
    #[allow(dead_code)]
    pub fn merge_data(&mut self, other: &UrlData) {
        let mut new_extra = HashMap::new();
        // Other's data first, then this's data (duplicates from other win)
        for (k, v) in &other.extra {
            new_extra.insert(k.clone(), v.clone());
        }
        for (k, v) in &self.extra {
            new_extra.insert(k.clone(), v.clone());
        }
        self.extra = new_extra;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_url() {
        let data = UrlData::new("https://example.com/path");
        assert_eq!(data.url, "https://example.com/path");
        assert!(data.extra.is_empty());
        assert!(data.trigger.is_none());
        assert!(data.trigger_own);
        assert!(!data.disable_updates);
    }

    #[test]
    fn test_dont_trigger_own() {
        let data = UrlData::new("https://example.com").dont_trigger_own();
        assert!(!data.trigger_own);
        assert!(data.trigger_own == false);
    }

    #[test]
    fn test_disable_updates() {
        let data = UrlData::new("https://example.com").disable_updates();
        assert!(data.disable_updates);
    }

    #[test]
    fn test_put_data() {
        let data = UrlData::new("https://example.com").put_data("key", "value");
        assert_eq!(data.get_data("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_get_data_not_found() {
        let data = UrlData::new("https://example.com").put_data("key", "value");
        assert_eq!(data.get_data("not_found"), None);
    }

    #[test]
    fn test_get_data_by_prefix() {
        let data = UrlData::new("https://example.com")
            .put_data("prefix.key1", "value1")
            .put_data("prefix.key2", "value2")
            .put_data("other.key", "value3");
        
        let results = data.get_data_by_prefix("prefix.");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|(k, v)| **k == "prefix.key1" && **v == "value1"));
        assert!(results.iter().any(|(k, v)| **k == "prefix.key2" && **v == "value2"));
    }

    #[test]
    fn test_get_data_by_prefix_empty() {
        let data = UrlData::new("https://example.com").put_data("key", "value");
        let results = data.get_data_by_prefix("not_found.");
        assert!(results.is_empty());
    }

    #[test]
    fn test_merge_data_other_first() {
        let mut data1 = UrlData::new("https://example.com")
            .put_data("key1", "value1")
            .put_data("key2", "value2");
        
        let data2 = UrlData::new("https://other.com")
            .put_data("key2", "value2_override")
            .put_data("key3", "value3");
        
        data1.merge_data(&data2);
        
        // key2 should be overridden by data1 (self wins)
        assert_eq!(data1.get_data("key2"), Some(&"value2".to_string()));
        // key3 should be present
        assert_eq!(data1.get_data("key3"), Some(&"value3".to_string()));
    }

    #[test]
    fn test_merge_data_preserves_existing() {
        let mut data1 = UrlData::new("https://example.com")
            .put_data("key1", "value1");
        
        let data2 = UrlData::new("https://other.com")
            .put_data("key2", "value2");
        
        data1.merge_data(&data2);
        
        assert_eq!(data1.get_data("key1"), Some(&"value1".to_string()));
        assert_eq!(data1.get_data("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_clone() {
        let data = UrlData::new("https://example.com")
            .put_data("key", "value");
        let cloned = data.clone();
        
        assert_eq!(cloned.url, data.url);
        assert_eq!(cloned.get_data("key"), data.get_data("key"));
    }

    #[test]
    fn test_debug_format() {
        let data = UrlData::new("https://example.com").put_data("key", "value");
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("UrlData"));
        assert!(debug_str.contains("https://example.com"));
    }

    #[test]
    fn test_empty_url() {
        let data = UrlData::new("");
        assert_eq!(data.url, "");
    }

    #[test]
    fn test_url_with_all_components() {
        let url = "https://user:pass@example.com:8443/path?query=value#fragment";
        let data = UrlData::new(url);
        assert_eq!(data.url, url);
    }

    #[test]
    fn test_trigger_field() {
        let mut data = UrlData::new("https://example.com");
        data.trigger = Some("test_module");
        assert_eq!(data.trigger, Some("test_module"));
    }

    #[test]
    fn test_disable_updates_flag() {
        let mut data = UrlData::new("https://example.com");
        assert!(!data.disable_updates);
        data.disable_updates = true;
        assert!(data.disable_updates);
    }
}
