use std::path::{Path, PathBuf};

use serde::Deserialize;

/// System font discovery settings.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Allowlist — only these families are included. Empty = use built-in
    /// curated list (see [`MACOS_SANE_FONTS`]).
    #[serde(default)]
    pub include: Vec<String>,
    /// Blocklist — these families are always excluded, even if in `include`.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Extra directories to scan (in addition to platform defaults).
    #[serde(default)]
    pub dirs: Vec<PathBuf>,
}

/// A system font discovered on disk.
#[derive(Debug, Clone)]
pub struct Font {
    pub family: String,
    pub style: String,
    pub path: PathBuf,
    /// Index within a `.ttc` collection (0 for standalone files).
    pub index: u32,
}

// OpenType name table IDs.
const NAME_FAMILY: u16 = 1;
const NAME_SUBFAMILY: u16 = 2;
const NAME_TYPOGRAPHIC_FAMILY: u16 = 16;
const NAME_TYPOGRAPHIC_SUBFAMILY: u16 = 17;

/// Curated list of macOS system fonts that are generally useful.
///
/// Used as the default allowlist when [`Config::include`] is empty.
pub const MACOS_SANE_FONTS: &[&str] = &[
    "Avenir",
    "Avenir Next",
    "Avenir Next Condensed",
    "Baskerville",
    "Big Caslon",
    "Bodoni 72",
    "Bodoni 72 Oldstyle",
    "Bodoni 72 Smallcaps",
    "Bradley Hand",
    "Charter",
    "Cochin",
    "Copperplate",
    "Courier New",
    "Didot",
    "Futura",
    "Geneva",
    "Georgia",
    "Gill Sans",
    "Helvetica",
    "Helvetica Neue",
    "Hoefler Text",
    "Iowan Old Style",
    "Lucida Grande",
    "Menlo",
    "Monaco",
    "New York",
    "Optima",
    "Palatino",
    "Phosphate",
    "Rockwell",
    "San Francisco",
    "SF Pro",
    "SF Pro Display",
    "SF Pro Rounded",
    "SF Pro Text",
    "SF Mono",
    "SF Compact",
    "SF Compact Display",
    "SF Compact Rounded",
    "SF Compact Text",
    "Skia",
    "Superclarendon",
    "Times New Roman",
    "Trebuchet MS",
    "Verdana",
];

/// Default font directories for the current platform.
pub fn default_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/System/Library/Fonts"));
        dirs.push(PathBuf::from("/Library/Fonts"));
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join("Library/Fonts"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        dirs.push(PathBuf::from("/usr/share/fonts"));
        dirs.push(PathBuf::from("/usr/local/share/fonts"));
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join(".local/share/fonts"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(windir) = std::env::var_os("WINDIR") {
            dirs.push(PathBuf::from(windir).join("Fonts"));
        }
        if let Some(local) = dirs::data_local_dir() {
            dirs.push(local.join("Microsoft\\Windows\\Fonts"));
        }
    }

    dirs
}

/// Discover system fonts, filtered by the given configuration.
///
/// This is a blocking operation (directory traversal + font parsing).
/// Wrap in `tokio::task::spawn_blocking` if calling from async context.
pub fn discover(config: &Config) -> Vec<Font> {
    let dirs = if config.dirs.is_empty() {
        default_dirs()
    } else {
        let mut dirs = default_dirs();
        dirs.extend(config.dirs.iter().cloned());
        dirs
    };

    let allowlist: Vec<&str> = if config.include.is_empty() {
        #[cfg(target_os = "macos")]
        {
            MACOS_SANE_FONTS.to_vec()
        }
        #[cfg(not(target_os = "macos"))]
        {
            Vec::new() // no filtering on other platforms by default
        }
    } else {
        config.include.iter().map(String::as_str).collect()
    };

    let mut fonts = Vec::new();
    for dir in &dirs {
        scan_directory(dir, &mut fonts);
    }

    // Apply allowlist (if non-empty).
    if !allowlist.is_empty() {
        fonts.retain(|f| allowlist.iter().any(|a| f.family.eq_ignore_ascii_case(a)));
    }

    // Apply blocklist.
    if !config.exclude.is_empty() {
        fonts.retain(|f| {
            !config
                .exclude
                .iter()
                .any(|e| f.family.eq_ignore_ascii_case(e))
        });
    }

    fonts.sort_by(|a, b| a.family.cmp(&b.family).then(a.style.cmp(&b.style)));
    fonts.dedup_by(|a, b| a.family == b.family && a.style == b.style);
    fonts
}

/// Load the raw bytes of a system font file.
pub async fn load(font: &Font) -> Result<Vec<u8>, crate::Error> {
    Ok(tokio::fs::read(&font.path).await?)
}

/// List just the unique family names from a set of discovered fonts.
pub fn family_names(fonts: &[Font]) -> Vec<String> {
    let mut names: Vec<String> = fonts.iter().map(|f| f.family.clone()).collect();
    names.dedup();
    names
}

fn scan_directory(dir: &Path, fonts: &mut Vec<Font>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_directory(&path, fonts);
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match ext.as_str() {
            "ttf" | "otf" => parse_font_file(&path, 0, fonts),
            "ttc" | "otc" => parse_collection(&path, fonts),
            _ => {}
        }
    }
}

fn parse_font_file(path: &Path, index: u32, fonts: &mut Vec<Font>) {
    let Ok(data) = std::fs::read(path) else {
        return;
    };
    let Ok(face) = ttf_parser::Face::parse(&data, index) else {
        return;
    };

    if let Some(family) = extract_name(&face, NAME_TYPOGRAPHIC_FAMILY, NAME_FAMILY) {
        let style = extract_name(&face, NAME_TYPOGRAPHIC_SUBFAMILY, NAME_SUBFAMILY)
            .unwrap_or_else(|| "Regular".into());

        fonts.push(Font {
            family,
            style,
            path: path.to_owned(),
            index,
        });
    }
}

fn parse_collection(path: &Path, fonts: &mut Vec<Font>) {
    let Ok(data) = std::fs::read(path) else {
        return;
    };
    let count = match ttf_parser::fonts_in_collection(&data) {
        Some(n) => n,
        None => return,
    };

    for i in 0..count {
        let Ok(face) = ttf_parser::Face::parse(&data, i) else {
            continue;
        };

        if let Some(family) = extract_name(&face, NAME_TYPOGRAPHIC_FAMILY, NAME_FAMILY) {
            let style = extract_name(&face, NAME_TYPOGRAPHIC_SUBFAMILY, NAME_SUBFAMILY)
                .unwrap_or_else(|| "Regular".into());

            fonts.push(Font {
                family,
                style,
                path: path.to_owned(),
                index: i,
            });
        }
    }
}

fn extract_name(face: &ttf_parser::Face, primary_id: u16, fallback_id: u16) -> Option<String> {
    face.names()
        .into_iter()
        .find(|n| n.name_id == primary_id)
        .and_then(|n| n.to_string())
        .or_else(|| {
            face.names()
                .into_iter()
                .find(|n| n.name_id == fallback_id)
                .and_then(|n| n.to_string())
        })
}
