//! Google Fonts integration — catalog browsing and on-demand font loading.
//!
//! Fonts are downloaded from public Google Fonts endpoints (no API key needed)
//! and cached to disk at `{cache_dir}/fount/google/`.

pub mod catalog;
pub mod family;

mod cache;
mod css;
mod fetch;

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

/// Load standard variants (400, 700, 400i, 700i) of a font family.
///
/// Returns raw font file bytes for each variant. The caller is responsible
/// for registering them with iced via `iced::font::load()`.
pub async fn load(family: &str) -> Result<Vec<Vec<u8>>, Error> {
    load_variants(
        family,
        &["400".into(), "700".into(), "400i".into(), "700i".into()],
    )
    .await
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
