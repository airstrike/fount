use std::path::Path;

use serde::Deserialize;

use crate::Error;
use crate::google;
use crate::system;

/// Top-level configuration, typically loaded from `fonts.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub system: system::Config,
    pub google: google::Config,
    #[serde(default)]
    pub custom: Vec<Custom>,
}

/// A custom font loaded from a URL.
#[derive(Debug, Clone, Deserialize)]
pub struct Custom {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub variants: Vec<String>,
}

impl Config {
    /// Load configuration from a TOML file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&text)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_loads_nothing() {
        let config: Config = toml::from_str("").unwrap();
        assert!(!config.system.enabled);
        assert!(!config.google.enabled);
        assert!(config.custom.is_empty());
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
[system]
enabled = true
include = ["Helvetica Neue", "Menlo"]
exclude = ["Apple Symbols"]

[google]
enabled = true
preload = ["Inter", "IBM Plex Sans"]
catalog_limit = 50

[[custom]]
name = "My Font"
url = "https://example.com/font.ttf"
variants = ["400", "700"]
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.system.include.len(), 2);
        assert_eq!(config.system.exclude, vec!["Apple Symbols"]);
        assert_eq!(config.google.preload, vec!["Inter", "IBM Plex Sans"]);
        assert_eq!(config.google.catalog_limit, 50);
        assert_eq!(config.custom.len(), 1);
        assert_eq!(config.custom[0].name, "My Font");
    }
}
