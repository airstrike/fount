use std::path::PathBuf;
use std::time::Duration;

use crate::Error;

pub(crate) fn cache_dir() -> Result<PathBuf, Error> {
    dirs::cache_dir()
        .map(|d| d.join("fount").join("google"))
        .ok_or(Error::NoCacheDir)
}

/// Check if a variable-range font file in the cache covers the requested variant.
///
/// E.g. `100..900.ttf` covers variant `400`, `100..900i.ttf` covers `400i`.
async fn find_variable_cache(cache_dir: &std::path::Path, variant: &str) -> Option<Vec<u8>> {
    let (weight, is_italic) = super::parse_variant(variant);
    let mut entries = tokio::fs::read_dir(cache_dir).await.ok()?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name = name.to_str()?;
        let stem = name.strip_suffix(".ttf")?;

        let (range_str, range_italic) = if let Some(s) = stem.strip_suffix('i') {
            (s, true)
        } else {
            (stem, false)
        };

        if range_italic != is_italic {
            continue;
        }

        if let Some((min, max)) = super::parse_weight_range(range_str)
            && weight >= min
            && weight <= max
        {
            return tokio::fs::read(entry.path()).await.ok();
        }
    }

    None
}

/// Load catalog metadata from disk cache, or fetch and cache it.
pub(crate) async fn load_or_fetch_metadata(max_age: Duration) -> Result<String, Error> {
    let dir = cache_dir()?;
    let path = dir.join("metadata.json");

    if let Ok(meta) = tokio::fs::metadata(&path).await
        && let Ok(modified) = meta.modified()
        && modified.elapsed().unwrap_or(Duration::MAX) < max_age
        && let Ok(data) = tokio::fs::read_to_string(&path).await
    {
        tracing::debug!("using cached metadata from {}", path.display());
        return Ok(data);
    }

    tracing::info!("fetching Google Fonts metadata");
    let data = super::fetch::metadata().await?;
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(&path, &data).await?;
    Ok(data)
}

/// Load font variant files from disk cache, fetching any that are missing.
///
/// For each requested variant, checks:
/// 1. Exact static file (e.g. `400.ttf`)
/// 2. Variable-range file (e.g. `100..900.ttf` covers variant `400`)
/// 3. Downloads from Google Fonts if neither is cached
pub(crate) async fn load_or_fetch_fonts(
    family: &str,
    variants: &[String],
) -> Result<Vec<Vec<u8>>, Error> {
    let dir = cache_dir()?.join("fonts").join(family);
    let mut all_bytes = Vec::new();
    let mut uncached = Vec::new();

    for variant in variants {
        // 1. Exact match
        let path = dir.join(format!("{variant}.ttf"));
        if let Ok(bytes) = tokio::fs::read(&path).await {
            tracing::debug!("cache hit: {family} {variant}");
            all_bytes.push(bytes);
            continue;
        }
        // 2. Variable-range match
        if let Some(bytes) = find_variable_cache(&dir, variant).await {
            tracing::debug!("variable cache hit: {family} {variant}");
            all_bytes.push(bytes);
            continue;
        }
        // 3. Need to download
        uncached.push(variant.clone());
    }

    if uncached.is_empty() {
        return Ok(all_bytes);
    }

    tracing::info!("downloading {family} variants: {uncached:?}");
    let css_text = super::fetch::css(family, &uncached).await?;
    let faces = super::css::parse(&css_text);

    if faces.is_empty() {
        tracing::warn!(
            "no font URLs found in CSS response for {family} \
             (requested variants: {uncached:?}). The font may not \
             support these variants."
        );
        return Err(Error::NoFontUrls {
            family: family.to_owned(),
        });
    }

    // Warn if we got fewer faces than requested.
    if faces.len() < uncached.len() {
        let found: Vec<_> = faces.iter().map(|f| f.variant_key()).collect();
        tracing::warn!("{family}: requested {uncached:?} but only found {found:?}");
    }

    tokio::fs::create_dir_all(&dir).await?;

    for face in &faces {
        let bytes = super::fetch::bytes(&face.url).await?;
        let path = dir.join(format!("{}.ttf", face.variant_key()));
        if let Err(e) = tokio::fs::write(&path, &bytes).await {
            tracing::warn!("failed to cache {}: {e}", path.display());
        }
        all_bytes.push(bytes);
    }

    Ok(all_bytes)
}
