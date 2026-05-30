//! Configuration management.
//!
//! Looks for config files in `$XDG_CONFIG_HOME/redirector/` with a fallback
//! to `~/.config/redirector/`.

use std::path::PathBuf;

/// Get the XDG config directory (~/.config)
fn xdg_config_dir() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")).unwrap_or_else(|| PathBuf::from(".").join(".config")))
}

/// Find a configuration file in the redirector config directory.
pub fn find_config_file(filename: &str) -> Option<PathBuf> {
    let path = xdg_config_dir().join("redirector").join(filename);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xdg_dir() {
        let config = xdg_config_dir();
        assert!(config.is_absolute() || config.ends_with(".config"));
    }

    #[test]
    fn test_find_config_file_xdg() {
        let path = find_config_file("automations.json");
        assert!(path.is_some(), "Should find automations.json");
        let path = path.unwrap();
        assert!(path.exists(), "Path should exist: {:?}", path);
        println!("Found: {:?}", path);
    }
}
