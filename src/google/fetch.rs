use std::sync::LazyLock;

use crate::error::Error;

const METADATA_URL: &str = "https://fonts.google.com/metadata/fonts";

/// User-Agent that requests TrueType fonts from Google Fonts CSS2 API.
///
/// Google Fonts serves different formats depending on the User-Agent header.
/// An empty or default UA may return no `url()` entries at all.
/// This UA string gets us `.ttf` files which cosmic-text can load directly.
pub(crate) const USER_AGENT: &str = "Mozilla/4.0";

/// The catalog metadata runs to several megabytes; font files are smaller.
const BODY_LIMIT: u64 = 64 * 1024 * 1024;

static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .build()
        .into()
});

/// Fetch the full catalog metadata JSON from Google Fonts.
pub(crate) fn metadata() -> Result<String, Error> {
    let text = AGENT
        .get(METADATA_URL)
        .call()?
        .body_mut()
        .with_config()
        .limit(BODY_LIMIT)
        .read_to_string()?;
    // The endpoint may prefix the JSON with ")]}'\n" as XSS protection.
    let json = text
        .strip_prefix(")]}'")
        .map(|s| s.trim_start())
        .unwrap_or(&text);
    Ok(json.to_owned())
}

/// Fetch the CSS2 stylesheet for a family's variants.
pub(crate) fn css<V: AsRef<str>>(family: &str, variants: &[V]) -> Result<String, Error> {
    let url = super::css::build_url(family, variants);
    tracing::debug!("fetching CSS: {url}");
    Ok(AGENT.get(&url).call()?.body_mut().read_to_string()?)
}

/// Download raw bytes from a URL (typically a font file on fonts.gstatic.com).
pub(crate) fn bytes(url: &str) -> Result<Vec<u8>, Error> {
    Ok(AGENT
        .get(url)
        .call()?
        .body_mut()
        .with_config()
        .limit(BODY_LIMIT)
        .read_to_vec()?)
}
