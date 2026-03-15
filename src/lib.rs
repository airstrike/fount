//! Font management for iced applications.
//!
//! Supports multiple font sources — system fonts, Google Fonts, and custom
//! URLs — configured via a `fonts.toml` file or programmatic API.
//!
//! All loading is async. [`Fount::load`] returns an iced [`Task`] that
//! downloads (or reads from disk), caches, and registers each font file
//! with iced's font system as it becomes available.

pub mod config;
pub mod error;
pub mod google;
pub mod system;

pub use config::Config;
pub use error::Error;
pub use google::Catalog;

use iced_core::Font;

/// Font manager. Aggregates font sources (system, Google Fonts) and
/// provides a unified API for resolving and loading fonts.
#[derive(Debug, Default)]
pub struct Fount {
    google_catalog: Option<google::Catalog>,
    system_families: Vec<String>,
}

impl Fount {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a [`Font`] descriptor for the given family name.
    ///
    /// This does not load the font — it only creates a descriptor that
    /// iced will resolve when rendering. Call [`load`](Self::load) to
    /// ensure the font bytes are available.
    pub fn font(&self, name: &str) -> Font {
        Font::with_family(name)
    }

    // --- Google Fonts ---

    /// Store the Google Fonts catalog once fetched.
    pub fn set_google_catalog(&mut self, catalog: google::Catalog) {
        self.google_catalog = Some(catalog);
    }

    /// The Google Fonts catalog, if loaded.
    pub fn google_catalog(&self) -> Option<&google::Catalog> {
        self.google_catalog.as_ref()
    }

    // --- System fonts ---

    /// Store discovered system font family names.
    pub fn set_system_families(&mut self, families: Vec<String>) {
        self.system_families = families;
    }

    /// Known system font family names.
    pub fn system_families(&self) -> &[String] {
        &self.system_families
    }

    // --- Unified queries ---

    /// All known family names from every source, deduplicated and sorted.
    pub fn families(&self) -> Vec<String> {
        let mut names: Vec<String> = self.system_families.clone();
        if let Some(catalog) = &self.google_catalog {
            names.extend(catalog.family_names());
        }
        names.sort();
        names.dedup();
        names
    }

    /// Whether a family name is known to any source.
    pub fn has_family(&self, name: &str) -> bool {
        self.system_families.iter().any(|n| n == name)
            || self
                .google_catalog
                .as_ref()
                .is_some_and(|c| c.get(name).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_returns_same_value() {
        let f = Fount::new();
        let a = f.font("Roboto");
        let b = f.font("Roboto");
        assert_eq!(a, b);
    }

    #[test]
    fn font_different_names() {
        let f = Fount::new();
        let a = f.font("FontA");
        let b = f.font("FontB");
        assert_ne!(a, b);
    }

    #[test]
    fn families_merges_and_deduplicates() {
        let mut f = Fount::new();
        f.set_system_families(vec!["Menlo".into(), "Helvetica".into()]);
        let names = f.families();
        assert_eq!(names, vec!["Helvetica", "Menlo"]);
    }

    #[test]
    fn has_family_checks_system() {
        let mut f = Fount::new();
        f.set_system_families(vec!["Menlo".into()]);
        assert!(f.has_family("Menlo"));
        assert!(!f.has_family("Comic Sans"));
    }
}
