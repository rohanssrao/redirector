//! Module trait and module manager.
//!
//! Each module implements the `Module` trait and can optionally provide
//! automations. Modules are processed in a fixed order through the pipeline.

pub mod clearurls;
pub mod open;
pub mod pattern;

use crate::url_data::{ModuleId, UrlData};

/// A single transformation step in the URL pipeline.
///
/// Modules implement up to 4 lifecycle hooks:
/// - `on_prepare`: Always called for every URL (initialization)
/// - `on_modify`: May change the URL; returns new URL or None
/// - `on_display`: Called with the final URL for UI updates
/// - `on_finish`: Final cleanup/notification
pub trait Module {
    /// Unique identifier for this module (e.g., "clearurls")
    fn id(&self) -> ModuleId;

    /// Called once per URL, before any modifications
    fn on_prepare(&self, _data: &mut UrlData) {}

    /// Called once per URL, may modify the URL.
    /// Return Some(new_url) to apply the change (which restarts the pipeline),
    /// or None to keep the current URL.
    fn on_modify(&self, _data: &mut UrlData) -> Option<String> {
        None
    }

    /// Called with the final URL for display purposes
    fn on_display(&self, _data: &UrlData) {}

    /// Called after all processing is complete
    fn on_finish(&self, _data: &UrlData) {}
}

/// A record of a URL transformation performed by a module
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChangeRecord {
    pub module: ModuleId,
    pub original: String,
    pub result: String,
}

/// Manages the collection of active modules.
pub struct ModuleManager {
    modules: Vec<Box<dyn Module>>,
}

impl ModuleManager {
    /// Create a new ModuleManager with the given modules.
    pub fn new(modules: Vec<Box<dyn Module>>) -> Self {
        Self { modules }
    }

    /// Get all modules (in registration order).
    pub fn modules(&self) -> &[Box<dyn Module>] {
        &self.modules
    }
}
