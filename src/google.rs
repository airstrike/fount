//! Google Fonts integration — catalog browsing and on-demand font loading.
//!
//! Fonts are downloaded from public Google Fonts endpoints (no API key needed)
//! and cached to disk at `{cache_dir}/fount/google/`.

pub mod catalog;
pub mod family;

mod cache;
pub(crate) mod css;
pub(crate) mod fetch;

pub use catalog::Catalog;
pub use family::{Axis, Category, Family, Variants};

use std::time::Duration;

use serde::Deserialize;

use crate::Error;

/// Google Fonts settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Families to download eagerly at startup.
    #[serde(default)]
    pub preload: Vec<String>,
    /// Max families to show in a picker UI.
    pub catalog_limit: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            preload: Vec::new(),
            catalog_limit: 100,
        }
    }
}

/// Default max age for cached catalog metadata (7 days).
pub const DEFAULT_CATALOG_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Fetch the Google Fonts catalog, using a disk cache with the given max age.
pub async fn catalog(max_age: Duration) -> Result<Catalog, Error> {
    let raw = cache::load_or_fetch_metadata(max_age).await?;
    catalog::parse(&raw)
}

/// Load a font family, using the catalog to determine available variants.
///
/// If the family is found in the catalog, only its actual variants are
/// requested — this avoids requesting an `ital` axis for fonts that don't
/// support italic, which would cause the CSS2 API to return empty CSS.
///
/// Without a catalog, falls back to common variants (400, 700, 400i, 700i).
///
/// Returns raw font file bytes for each variant. The caller is responsible
/// for registering them with iced via `iced::font::load()`.
pub async fn load(family: &str, catalog: Option<&Catalog>) -> Result<Vec<Vec<u8>>, Error> {
    let variants = catalog
        .and_then(|c| c.get(family))
        .map(|f| f.variant_keys())
        .unwrap_or_else(|| vec!["400".into(), "700".into(), "400i".into(), "700i".into()]);
    load_variants(family, &variants).await
}

/// Load specific variants of a font family.
///
/// Variant keys follow Google Fonts conventions: `"400"`, `"700"`,
/// `"400i"` (italic), `"700i"`, etc.
///
/// Returns raw font file bytes. The caller registers them with iced.
pub async fn load_variants(family: &str, variants: &[String]) -> Result<Vec<Vec<u8>>, Error> {
    cache::load_or_fetch_fonts(family, variants).await
}

/// Check if a variable-range font file in the cache covers the requested variant.
///
/// Variant `"400"` is covered by `"100..900.ttf"` if 100 <= 400 <= 900.
/// Variant `"400i"` is covered by `"100..900i.ttf"`.
#[cfg(test)]
fn find_variable_cache(cache_dir: &std::path::Path, variant: &str) -> Option<Vec<u8>> {
    let (weight, is_italic) = parse_variant(variant);
    let entries = std::fs::read_dir(cache_dir).ok()?;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_str()?;
        let stem = name.strip_suffix(".ttf")?;

        // Only match range patterns like "100..900" or "100..900i"
        let (range_str, range_italic) = if let Some(s) = stem.strip_suffix('i') {
            (s, true)
        } else {
            (stem, false)
        };

        if range_italic != is_italic {
            continue;
        }

        if let Some((min, max)) = parse_weight_range(range_str)
            && weight >= min
            && weight <= max
        {
            return std::fs::read(entry.path()).ok();
        }
    }

    None
}

/// Parse a variant key like `"400"` → (400, false) or `"700i"` → (700, true).
pub(crate) fn parse_variant(variant: &str) -> (u16, bool) {
    if let Some(w) = variant.strip_suffix('i') {
        (w.parse().unwrap_or(400), true)
    } else {
        (variant.parse().unwrap_or(400), false)
    }
}

/// Parse a weight range like `"100..900"` → Some((100, 900)).
pub(crate) fn parse_weight_range(s: &str) -> Option<(u16, u16)> {
    let (min, max) = s.split_once("..")?;
    Some((min.parse().ok()?, max.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_variant_normal() {
        assert_eq!(parse_variant("400"), (400, false));
        assert_eq!(parse_variant("700"), (700, false));
    }

    #[test]
    fn parse_variant_italic() {
        assert_eq!(parse_variant("400i"), (400, true));
        assert_eq!(parse_variant("700i"), (700, true));
    }

    #[test]
    fn parse_weight_range_valid() {
        assert_eq!(parse_weight_range("100..900"), Some((100, 900)));
        assert_eq!(parse_weight_range("400..700"), Some((400, 700)));
    }

    #[test]
    fn parse_weight_range_invalid() {
        assert_eq!(parse_weight_range("400"), None);
        assert_eq!(parse_weight_range("abc..def"), None);
    }

    #[test]
    fn find_variable_cache_matches_range() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path();

        // Create a variable font file "100..900.ttf"
        std::fs::write(cache.join("100..900.ttf"), b"fake-font-data").unwrap();

        // "400" should match 100..900
        assert!(find_variable_cache(cache, "400").is_some());
        // "700" should match too
        assert!(find_variable_cache(cache, "700").is_some());
        // "400i" should NOT match (no italic range file)
        assert!(find_variable_cache(cache, "400i").is_none());

        // Add italic range
        std::fs::write(cache.join("100..900i.ttf"), b"fake-italic-data").unwrap();
        assert!(find_variable_cache(cache, "400i").is_some());
    }
}
