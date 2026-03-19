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

/// Load a single font variant (blocking). Checks the disk cache first,
/// then downloads from Google Fonts if missing.
///
/// `variant` follows Google Fonts conventions: `"400"`, `"700"`, `"400i"`, etc.
///
/// Returns raw TTF bytes, or `None` if the font can't be loaded.
pub fn load_variant_blocking(family: &str, variant: &str) -> Option<Vec<u8>> {
    // Check disk cache first
    let cache_dir = dirs::cache_dir()?
        .join("fount")
        .join("google")
        .join("fonts")
        .join(family);
    let path = cache_dir.join(format!("{variant}.ttf"));
    if let Ok(bytes) = std::fs::read(&path) {
        return Some(bytes);
    }

    // Download via blocking HTTP
    let variants = vec![variant.to_string()];
    let url = css::build_url(family, &variants);
    let client = reqwest::blocking::Client::builder()
        .user_agent(fetch::USER_AGENT)
        .build()
        .ok()?;

    let css_text = client.get(&url).send().ok()?.text().ok()?;
    let faces = css::parse(&css_text);
    let face = faces.into_iter().find(|f| f.variant_key() == variant)?;

    let bytes = client.get(&face.url).send().ok()?.bytes().ok()?.to_vec();

    // Cache to disk
    let _ = std::fs::create_dir_all(&cache_dir);
    let _ = std::fs::write(&path, &bytes);

    Some(bytes)
}
