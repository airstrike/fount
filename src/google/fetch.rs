use crate::error::Error;

const METADATA_URL: &str = "https://fonts.google.com/metadata/fonts";

/// User-Agent that requests TrueType fonts from Google Fonts CSS2 API.
///
/// Google Fonts serves different formats depending on the User-Agent header.
/// An empty or default reqwest UA may return no `url()` entries at all.
/// This UA string gets us `.ttf` files which cosmic-text can load directly.
const USER_AGENT: &str = "Mozilla/4.0";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .expect("build reqwest client")
}

/// Fetch the full catalog metadata JSON from Google Fonts.
pub(crate) async fn metadata() -> Result<String, Error> {
    let text = client().get(METADATA_URL).send().await?.text().await?;
    // The endpoint may prefix the JSON with ")]}'\n" as XSS protection.
    let json = text
        .strip_prefix(")]}'")
        .map(|s| s.trim_start())
        .unwrap_or(&text);
    Ok(json.to_owned())
}

/// Fetch the CSS2 stylesheet for a family's variants.
pub(crate) async fn css(family: &str, variants: &[String]) -> Result<String, Error> {
    let url = super::css::build_url(family, variants);
    tracing::debug!("fetching CSS: {url}");
    let text = client().get(&url).send().await?.text().await?;
    Ok(text)
}

/// Download raw bytes from a URL (typically a font file on fonts.gstatic.com).
pub(crate) async fn bytes(url: &str) -> Result<Vec<u8>, Error> {
    let data = client().get(url).send().await?.bytes().await?;
    Ok(data.to_vec())
}
