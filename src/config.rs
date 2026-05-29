//! Configuration management.
//!
//! Handles loading, saving, and discovering configuration files.

use std::path::PathBuf;

/// Find a configuration file, searching in order:
/// 1. Current directory
/// 2. User's config directory (~/.config/redirector/)
/// 3. XDG data directory (~/.local/share/redirector/)
pub fn find_config_file(filename: &str) -> Option<PathBuf> {
    // 1. Current directory
    if let Ok(cwd) = std::env::current_dir() {
        let path = cwd.join(filename);
        if path.exists() {
            return Some(path);
        }
    }

    // 2. XDG config directory
    if let Some(config_dir) = xdg_config_dir() {
        let path = config_dir.join("redirector").join(filename);
        if path.exists() {
            return Some(path);
        }
    }

    // 3. XDG data directory
    if let Some(data_dir) = xdg_data_dir() {
        let path = data_dir.join("redirector").join(filename);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Get the XDG config directory (~/.config)
fn xdg_config_dir() -> Option<PathBuf> {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config"))
        })
}

/// Get the XDG data directory (~/.local/share)
fn xdg_data_dir() -> Option<PathBuf> {
    std::env::var("XDG_DATA_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".local/share"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xdg_dirs() {
        let config = xdg_config_dir();
        assert!(config.is_some());
        let data = xdg_data_dir();
        assert!(data.is_some());
    }

    #[test]
    fn test_find_config_file_xdg() {
        let path = find_config_file("automations.json");
        assert!(path.is_some(), "Should find automations.json");
        let path = path.unwrap();
        assert!(path.exists(), "Path should exist: {:?}", path);
        println!("Found: {:?}", path);
        
        // Also test XDG config dir directly
        let xdg_path = xdg_config_dir().unwrap().join("redirector").join("automations.json");
        println!("XDG path: {:?}", xdg_path);
        assert!(xdg_path.exists(), "XDG path should exist: {:?}", xdg_path);
    }
}
