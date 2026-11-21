//! CORS-safe TrueType sources for the complete bundled Google Fonts snapshot.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::Error;

const INDEX: &str = include_str!("../../data/catalog.tsv");
static SOURCES: LazyLock<BTreeMap<&'static str, Vec<&'static str>>> = LazyLock::new(|| {
    INDEX
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut columns = line.split('\t');
            Some((columns.next()?, columns.collect()))
        })
        .collect()
});

fn urls(family: &str) -> Result<Vec<String>, Error> {
    let paths = SOURCES.get(family).ok_or_else(|| Error::UnknownFamily {
        family: family.to_owned(),
    })?;
    let base = INDEX
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("# "))
        .unwrap_or_default();
    Ok(paths
        .iter()
        .map(|path| {
            if path.starts_with("https://") {
                (*path).to_owned()
            } else {
                format!("{base}{path}")
            }
        })
        .collect())
}

/// Browsers fetch complete TrueType families, including variable and italic faces.
#[cfg(target_arch = "wasm32")]
pub(super) async fn load(family: &str) -> Result<Vec<Vec<u8>>, Error> {
    let client = reqwest::Client::new();
    let mut faces = Vec::new();
    for url in urls(family)? {
        let bytes = client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
            .to_vec();
        if !supported(&bytes) {
            return Err(Error::InvalidFont {
                family: family.to_owned(),
            });
        }
        faces.push(bytes);
    }
    Ok(faces)
}

#[cfg(any(target_arch = "wasm32", test))]
fn supported(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\0\x01\0\0") || bytes.starts_with(b"OTTO") || bytes.starts_with(b"ttcf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_snapshot_family_has_downloadable_faces() {
        let catalog = super::super::snapshot().expect("valid bundled metadata");
        assert!(catalog.len() > 1900);
        for family in catalog.families() {
            let sources = urls(&family.name).expect("every Google family has sources");
            assert!(!sources.is_empty(), "{}", family.name);
            assert!(sources.iter().all(|url| {
                url.starts_with("https://raw.githubusercontent.com/google/fonts/")
                    || url.starts_with("https://fonts.gstatic.com/")
            }));
        }
    }

    #[test]
    fn unknown_families_are_errors() {
        assert!(matches!(
            urls("Not a Google Font"),
            Err(Error::UnknownFamily { .. })
        ));
    }

    #[test]
    fn rejects_web_font_formats_and_error_pages() {
        assert!(supported(b"\0\x01\0\0font"));
        assert!(supported(b"OTTOfont"));
        assert!(!supported(b"wOF2font"));
        assert!(!supported(b"<!doctype html>"));
    }
}
