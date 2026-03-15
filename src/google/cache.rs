use std::path::PathBuf;
use std::time::Duration;

use crate::Error;

fn cache_dir() -> Result<PathBuf, Error> {
    dirs::cache_dir()
        .map(|d| d.join("fount").join("google"))
        .ok_or(Error::NoCacheDir)
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
pub(crate) async fn load_or_fetch_fonts(
    family: &str,
    variants: &[String],
) -> Result<Vec<Vec<u8>>, Error> {
    let dir = cache_dir()?.join("fonts").join(family);
    let mut all_bytes = Vec::new();
    let mut uncached = Vec::new();

    for variant in variants {
        let path = dir.join(format!("{variant}.ttf"));
        match tokio::fs::read(&path).await {
            Ok(bytes) => {
                tracing::debug!("cache hit: {family} {variant}");
                all_bytes.push(bytes);
            }
            Err(_) => uncached.push(variant.clone()),
        }
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
