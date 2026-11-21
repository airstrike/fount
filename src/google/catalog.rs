#[cfg(any(not(target_arch = "wasm32"), test))]
use std::collections::HashSet;
use std::sync::Arc;

use serde::Deserialize;

use super::family::{Axis, Category, Family, Variants};
use crate::Error;

/// The Google Fonts catalog — a list of families sorted by popularity.
#[derive(Debug, Clone)]
pub struct Catalog {
    families: Arc<Vec<Family>>,
}

impl Catalog {
    /// Add bundled families absent from the current live or cached catalog.
    #[cfg(any(not(target_arch = "wasm32"), test))]
    pub(crate) fn supplement(&mut self, fallback: &Self) {
        let mut names: HashSet<String> = self.family_names().into_iter().collect();
        let families = Arc::make_mut(&mut self.families);
        families.extend(
            fallback
                .families()
                .iter()
                .filter(|family| names.insert(family.name.clone()))
                .cloned(),
        );
        families.sort_by_key(|family| family.popularity);
    }

    /// All families, sorted by popularity (most popular first).
    pub fn families(&self) -> &[Family] {
        &self.families
    }

    /// Look up a family by name.
    pub fn get(&self, name: &str) -> Option<&Family> {
        self.families.iter().find(|f| f.name == name)
    }

    /// Names of all families, ordered by popularity.
    pub fn family_names(&self) -> Vec<String> {
        self.families.iter().map(|f| f.name.clone()).collect()
    }

    /// Names of the `n` most popular families.
    ///
    /// Noto script variants (e.g. "Noto Sans Arabic") are excluded — only
    /// the base Noto families are kept.
    pub fn top(&self, n: usize) -> Vec<String> {
        const NOTO_BASES: &[&str] = &[
            "Noto Sans",
            "Noto Serif",
            "Noto Sans Mono",
            "Noto Color Emoji",
        ];

        self.families
            .iter()
            .filter(|f| !f.is_noto || NOTO_BASES.contains(&f.name.as_str()))
            .take(n)
            .map(|f| f.name.clone())
            .collect()
    }

    /// Whether a family is a variable font.
    pub fn is_variable(&self, name: &str) -> Option<bool> {
        self.get(name)
            .map(|f| matches!(f.variants, Variants::Variable { .. }))
    }

    /// Number of families in the catalog.
    pub fn len(&self) -> usize {
        self.families.len()
    }

    /// Whether the catalog is empty.
    pub fn is_empty(&self) -> bool {
        self.families.is_empty()
    }
}

// --- serde types for the metadata JSON ---

#[derive(Deserialize)]
struct MetadataResponse {
    #[serde(rename = "familyMetadataList")]
    family_metadata_list: Vec<FamilyMetadata>,
}

#[derive(Deserialize)]
struct FamilyMetadata {
    family: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    popularity: u32,
    #[serde(default)]
    fonts: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    axes: Vec<AxisMetadata>,
    #[serde(rename = "isNoto", default)]
    is_noto: bool,
}

#[derive(Deserialize)]
struct AxisMetadata {
    tag: String,
    #[serde(default)]
    min: f32,
    #[serde(default)]
    max: f32,
    #[serde(alias = "defaultValue", default)]
    default: f32,
}

/// Parse the metadata JSON into a [`Catalog`].
pub(crate) fn parse(json: &str) -> Result<Catalog, Error> {
    let resp: MetadataResponse = serde_json::from_str(json)?;

    let mut families: Vec<Family> = resp
        .family_metadata_list
        .into_iter()
        .map(|m| {
            let variants = if m.axes.is_empty() {
                let mut keys: Vec<String> = m.fonts.keys().cloned().collect();
                keys.sort();
                Variants::Static { keys }
            } else {
                Variants::Variable {
                    axes: m
                        .axes
                        .into_iter()
                        .map(|a| Axis {
                            tag: a.tag,
                            min: a.min,
                            max: a.max,
                            default: a.default,
                        })
                        .collect(),
                }
            };

            Family {
                name: m.family,
                category: Category::from_metadata(&m.category),
                popularity: m.popularity,
                is_noto: m.is_noto,
                variants,
            }
        })
        .collect();

    families.sort_by_key(|f| f.popularity);

    Ok(Catalog {
        families: Arc::new(families),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"{
        "familyMetadataList": [
            {
                "family": "Open Sans",
                "category": "SANS_SERIF",
                "popularity": 2,
                "fonts": {"400": {}, "700": {}},
                "axes": []
            },
            {
                "family": "Roboto",
                "category": "SANS_SERIF",
                "popularity": 1,
                "fonts": {"400": {}, "700": {}, "400i": {}, "700i": {}},
                "axes": [{"tag": "wght", "min": 100.0, "max": 900.0, "defaultValue": 400.0}]
            },
            {
                "family": "Fira Code",
                "category": "MONOSPACE",
                "popularity": 50,
                "fonts": {"400": {}, "700": {}},
                "axes": [{"tag": "wght", "min": 300.0, "max": 700.0, "defaultValue": 400.0}]
            }
        ]
    }"#;

    #[test]
    fn older_cache_gains_snapshot_families_without_replacing_existing_metadata() {
        let mut cached = parse(
            r#"{
            "familyMetadataList": [
                {"family":"Roboto", "popularity":0, "fonts":{"400":{}}},
                {"family":"A New Live Family", "popularity":1, "fonts":{"700":{}}}
            ]
        }"#,
        )
        .unwrap();
        let bundled = crate::google::snapshot().unwrap();
        cached.supplement(&bundled);
        assert_eq!(cached.len(), bundled.len() + 1);
        assert!(cached.get("Edu SA Hand").is_some());
        assert!(cached.get("A New Live Family").is_some());
        assert_eq!(cached.get("Roboto").unwrap().variant_keys(), vec!["400"]);
        assert!(
            cached
                .families()
                .windows(2)
                .all(|pair| pair[0].popularity <= pair[1].popularity)
        );
        cached.supplement(&bundled);
        assert_eq!(cached.len(), bundled.len() + 1);
    }

    #[test]
    fn parse_sorts_by_popularity() {
        let catalog = parse(SAMPLE_JSON).unwrap();
        let names = catalog.family_names();
        assert_eq!(names, vec!["Roboto", "Open Sans", "Fira Code"]);
    }

    #[test]
    fn top_returns_n_most_popular() {
        let catalog = parse(SAMPLE_JSON).unwrap();
        assert_eq!(catalog.top(2), vec!["Roboto", "Open Sans"]);
    }

    #[test]
    fn get_finds_family() {
        let catalog = parse(SAMPLE_JSON).unwrap();
        let fira = catalog.get("Fira Code").unwrap();
        assert_eq!(fira.category, Category::Monospace);
        assert!(matches!(fira.variants, Variants::Variable { .. }));
    }

    #[test]
    fn static_vs_variable() {
        let catalog = parse(SAMPLE_JSON).unwrap();
        assert_eq!(catalog.is_variable("Open Sans"), Some(false));
        assert_eq!(catalog.is_variable("Roboto"), Some(true));
    }

    #[test]
    fn static_keys_sorted() {
        let catalog = parse(SAMPLE_JSON).unwrap();
        let open = catalog.get("Open Sans").unwrap();
        match &open.variants {
            Variants::Static { keys } => assert_eq!(keys, &["400", "700"]),
            _ => panic!("expected static"),
        }
    }

    #[test]
    fn variant_keys_static() {
        let catalog = parse(SAMPLE_JSON).unwrap();
        let open = catalog.get("Open Sans").unwrap();
        assert_eq!(open.variant_keys(), vec!["400", "700"]);
    }

    #[test]
    fn variant_keys_variable_no_ital() {
        let catalog = parse(SAMPLE_JSON).unwrap();
        // Fira Code: wght 300..700, no ital
        let fira = catalog.get("Fira Code").unwrap();
        assert_eq!(fira.variant_keys(), vec!["300..700"]);
    }

    #[test]
    fn variant_keys_variable_with_ital() {
        // Roboto only has wght in the sample — add a test with ital
        let json = r#"{
            "familyMetadataList": [{
                "family": "TestFont",
                "category": "SANS_SERIF",
                "popularity": 1,
                "fonts": {},
                "axes": [
                    {"tag": "wght", "min": 100.0, "max": 900.0, "defaultValue": 400.0},
                    {"tag": "ital", "min": 0.0, "max": 1.0, "defaultValue": 0.0}
                ]
            }]
        }"#;
        let catalog = parse(json).unwrap();
        let font = catalog.get("TestFont").unwrap();
        assert_eq!(font.variant_keys(), vec!["100..900", "100..900i"]);
    }
}
