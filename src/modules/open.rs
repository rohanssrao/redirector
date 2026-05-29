//! Open module - the final action that opens the URL in the default browser.
//!
//! This is typically the last module in the pipeline and is triggered
//! by a user action or an automation rule.

use crate::modules::Module;
use crate::url_data::{ModuleId, UrlData};

/// Module identifier
pub const ID: ModuleId = "open";

/// Open module implementation.
pub struct OpenModule;

impl Module for OpenModule {
    fn id(&self) -> ModuleId {
        ID
    }

    fn on_display(&self, _data: &UrlData) {
        // This module doesn't auto-open; it's triggered manually via the dialog button
        // or via automation. The actual opening happens in main.rs's show_dialog.
    }
}

impl Default for OpenModule {
    fn default() -> Self {
        Self
    }
}
