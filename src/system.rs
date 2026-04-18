use std::path::{Path, PathBuf};

use serde::Deserialize;

/// System font discovery settings.
///
/// `Config::default()` is the "scan everything sensible" preset: it
/// populates [`Config::dirs`] with [`default_dirs`], which includes
/// platform system font directories and — when the `office` cargo feature
/// is enabled — Microsoft Office's private font directories as well.
///
/// To customize, build from `Default`:
///
/// ```no_run
/// let mut cfg = fount::system::Config::default();
/// cfg.dirs.push("/my/extra/font/dir".into());         // add
/// cfg.dirs = vec!["/only/scan/this".into()];          // replace
/// cfg.exclude.push("Comic Sans MS".into());           // filter
/// fount::system::discover(&cfg);
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Config {
    /// Allowlist — only these families are included. Empty = use the
    /// built-in curated list (see [`MACOS_SANE_FONTS`]) on macOS, or no
    /// filtering at all on other platforms.
    pub include: Vec<String>,
    /// Blocklist — these families are always excluded, even if in `include`.
    pub exclude: Vec<String>,
    /// Directories to scan. Defaults to [`default_dirs`], which is the
    /// platform's system font directories plus Office private font paths
    /// when the `office` cargo feature is enabled.
    pub dirs: Vec<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            include: Vec::new(),
            exclude: Vec::new(),
            dirs: default_dirs(),
        }
    }
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
    "Aptos",
    "Arial",
    "Arial Black",
    "Avenir",
    "Avenir Next",
    "Avenir Next Condensed",
    "Baskerville",
    "Big Caslon",
    "Bodoni 72",
    "Bodoni 72 Oldstyle",
    "Bodoni 72 Smallcaps",
    "Book Antiqua",
    "Bradley Hand",
    "Calibri",
    "Cambria",
    "Candara",
    "Century Gothic",
    "Charter",
    "Cochin",
    "Comic Sans MS",
    "Consolas",
    "Constantia",
    "Copperplate",
    "Corbel",
    "Courier New",
    "Didot",
    "Franklin Gothic Medium",
    "Futura",
    "Garamond",
    "Geneva",
    "Georgia",
    "Gill Sans",
    "Grandview",
    "Helvetica",
    "Helvetica Neue",
    "Hoefler Text",
    "Impact",
    "Iowan Old Style",
    "Lucida Grande",
    "Lucida Sans Unicode",
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
    "Seaford",
    "Skeena",
    "Skia",
    "Superclarendon",
    "Tahoma",
    "Tenorite",
    "Times New Roman",
    "Trebuchet MS",
    "Verdana",
];

/// Directories where Microsoft Office stores its bundled (non-system)
/// fonts. Returns an empty Vec on platforms where Office isn't installed
/// or when the `office` cargo feature is disabled.
///
/// These fonts are licensed for use on machines with a valid Office
/// installation. Only enable scanning on such machines.
#[cfg(feature = "office")]
fn office_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // Office for Mac bundles fonts inside each app's Resources/DFonts
        // directory. The same family typically appears across all apps;
        // duplicates are deduped later by family+style.
        const APPS: &[&str] = &[
            "Microsoft Word",
            "Microsoft Excel",
            "Microsoft PowerPoint",
            "Microsoft Outlook",
            "Microsoft OneNote",
        ];
        for app in APPS {
            dirs.push(PathBuf::from(format!(
                "/Applications/{app}.app/Contents/Resources/DFonts"
            )));
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Click-to-Run Office stores private fonts under its VFS layout.
        // Both 32- and 64-bit install roots are checked.
        for root in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(pf) = std::env::var_os(root) {
                dirs.push(
                    PathBuf::from(pf)
                        .join("Microsoft Office")
                        .join("root")
                        .join("VFS")
                        .join("Fonts")
                        .join("private"),
                );
            }
        }
    }

    dirs
}

#[cfg(not(feature = "office"))]
fn office_dirs() -> Vec<PathBuf> {
    Vec::new()
}

/// Default font directories for the current platform.
///
/// Includes the OS's standard system font directories. When the `office`
/// cargo feature is enabled, also includes the directories where Microsoft
/// Office bundles its private fonts (Aptos, Calibri, Cambria, etc.).
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

    // Office-bundled fonts (Aptos, Calibri, Cambria, ...). Empty when the
    // `office` feature is disabled, so callers don't need a cfg gate.
    dirs.extend(office_dirs());

    dirs
}

/// Discover system fonts, filtered by the given configuration.
///
/// This is a blocking operation (directory traversal + font parsing).
/// Wrap in `tokio::task::spawn_blocking` if calling from async context.
pub fn discover(config: &Config) -> Vec<Font> {
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
    for dir in &config.dirs {
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
    decode_name_id(face, primary_id).or_else(|| decode_name_id(face, fallback_id))
}

/// Find a name table entry by id and decode it. Tries UTF-16BE first (the
/// usual Windows/Unicode platform encoding), then falls back to a byte-level
/// decode for Macintosh-platform names.
///
/// Microsoft's Office-bundled Aptos files only carry Macintosh-platform name
/// records, so the UTF-16BE-only path that ships with `ttf_parser::Name::to_string`
/// returns `None` and the font is silently dropped without this fallback.
fn decode_name_id(face: &ttf_parser::Face, name_id: u16) -> Option<String> {
    let mut fallback: Option<String> = None;
    for name in face.names() {
        if name.name_id != name_id {
            continue;
        }
        if let Some(s) = name.to_string() {
            return Some(s);
        }
        if fallback.is_none()
            && name.platform_id == ttf_parser::PlatformId::Macintosh
            && let Some(s) = decode_mac_roman(name.name)
        {
            fallback = Some(s);
        }
    }
    fallback
}

/// Best-effort decode of a Mac Roman byte string. Returns None if the bytes
/// don't form a recognizable name (empty or all-zero).
fn decode_mac_roman(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() || bytes.iter().all(|&b| b == 0) {
        return None;
    }
    // Mac Roman maps 0x00..0x7F identically to ASCII. Bytes >= 0x80 use a
    // different table than Latin-1, but for the font *family* and *style*
    // strings we care about (Aptos, Calibri, Bold, Light, etc.) the names
    // are pure ASCII in practice. Decode ASCII directly and substitute
    // U+FFFD for any high bytes — good enough for matching, and lossless
    // for everything we've seen in real Office fonts.
    let s: String = bytes
        .iter()
        .map(|&b| if b < 0x80 { b as char } else { '\u{FFFD}' })
        .collect();
    Some(s)
}

#[cfg(all(test, feature = "office", target_os = "macos"))]
mod office_tests {
    use super::*;

    /// Verifies that Office font discovery actually surfaces Aptos on a Mac
    /// with Microsoft Office installed. Skipped (with a warning) if no Office
    /// DFonts directory exists, so it remains useful in CI without Office.
    /// Discovery should keep *all* styles of a family, not just one — otherwise
    /// callers that only register the first match end up with (e.g.) Aptos-Black
    /// in the database while the renderer asks for Aptos at weight 400 and gets
    /// no good match. This test pins the contract.
    #[test]
    fn keeps_all_aptos_styles() {
        if !office_dirs().iter().any(|d| d.is_dir()) {
            eprintln!("skipping keeps_all_aptos_styles: no Office DFonts on this machine");
            return;
        }

        let fonts = discover(&Config::default());

        let aptos: Vec<&Font> = fonts
            .iter()
            .filter(|f| f.family.eq_ignore_ascii_case("Aptos"))
            .collect();

        assert!(
            aptos.len() >= 4,
            "expected several Aptos styles (Regular, Bold, Italic, Light, ...) — \
             got {} entries: {:?}",
            aptos.len(),
            aptos.iter().map(|f| &f.style).collect::<Vec<_>>()
        );

        // Sanity: Regular must be present, otherwise default-weight rendering
        // will silently fall back to a synthesized weight from a heavier face.
        assert!(
            aptos
                .iter()
                .any(|f| f.style.eq_ignore_ascii_case("Regular")),
            "expected an 'Regular' style for Aptos in discovery output; got: {:?}",
            aptos.iter().map(|f| &f.style).collect::<Vec<_>>()
        );
    }

    #[test]
    fn discovers_aptos_from_office() {
        // Sanity-check that *some* Office font directory exists, otherwise
        // skip — there's nothing meaningful to assert.
        let dirs = office_dirs();
        let any_exists = dirs.iter().any(|d| d.is_dir());
        if !any_exists {
            eprintln!(
                "skipping discovers_aptos_from_office: no Office DFonts directories on this machine"
            );
            return;
        }

        // Raw scan of the office dirs only — bypasses the system allowlist
        // so we can see exactly what ttf-parser thinks is in there.
        let mut raw = Vec::new();
        for dir in &dirs {
            scan_directory(dir, &mut raw);
        }
        raw.sort_by(|a, b| a.family.cmp(&b.family).then(a.style.cmp(&b.style)));
        raw.dedup_by(|a, b| a.family == b.family && a.style == b.style);

        let raw_families: Vec<&str> = raw.iter().map(|f| f.family.as_str()).collect();
        eprintln!(
            "raw office families ({}): {:?}",
            raw_families.len(),
            raw_families
        );

        assert!(
            raw.iter().any(|f| f.family.eq_ignore_ascii_case("Aptos")),
            "expected ttf-parser to extract family 'Aptos' from one of the Office DFonts files; \
             got families: {raw_families:?}"
        );

        // Now exercise the full pipeline (allowlist + dedup) and confirm
        // Aptos survives all the filtering.
        let fonts = discover(&Config::default());
        let families = family_names(&fonts);
        eprintln!("discover() families ({}): {:?}", families.len(), families);

        assert!(
            families.iter().any(|f| f.eq_ignore_ascii_case("Aptos")),
            "Aptos was found in the raw scan but filtered out by discover(); families: {families:?}"
        );
    }
}
