use std::fmt;

/// A Google Fonts family entry.
#[derive(Debug, Clone)]
pub struct Family {
    pub name: String,
    pub category: Category,
    pub popularity: u32,
    pub is_noto: bool,
    pub variants: Variants,
}

impl Family {
    /// Variant keys suitable for the Google Fonts CSS2 API.
    ///
    /// For static fonts, returns the available keys (e.g. `["400", "700"]`).
    /// For variable fonts, returns the full weight range (e.g. `["100..900"]`
    /// or `["100..900", "100..900i"]` if the font has an italic axis).
    pub fn variant_keys(&self) -> Vec<String> {
        match &self.variants {
            Variants::Static { keys } => keys.clone(),
            Variants::Variable { axes } => {
                let wght = axes.iter().find(|a| a.tag == "wght");
                let has_ital = axes.iter().any(|a| a.tag == "ital");
                let range = match wght {
                    Some(a) => format!("{}..{}", a.min as u16, a.max as u16),
                    None => "400".into(),
                };
                if has_ital {
                    vec![range.clone(), format!("{range}i")]
                } else {
                    vec![range]
                }
            }
        }
    }
}

/// Whether a family uses variable or static font files.
#[derive(Debug, Clone)]
pub enum Variants {
    /// Variable font — a single file covers weight/italic axes.
    Variable { axes: Vec<Axis> },
    /// Static font — separate file per weight/style combination.
    Static { keys: Vec<String> },
}

/// An axis of a variable font (e.g. weight, width, italic).
#[derive(Debug, Clone)]
pub struct Axis {
    pub tag: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

/// Google Fonts family category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    SansSerif,
    Serif,
    Display,
    Handwriting,
    Monospace,
}

impl Category {
    pub(crate) fn from_metadata(s: &str) -> Self {
        match s {
            "SANS_SERIF" => Self::SansSerif,
            "SERIF" => Self::Serif,
            "DISPLAY" => Self::Display,
            "HANDWRITING" => Self::Handwriting,
            "MONOSPACE" => Self::Monospace,
            _ => Self::SansSerif,
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SansSerif => write!(f, "Sans Serif"),
            Self::Serif => write!(f, "Serif"),
            Self::Display => write!(f, "Display"),
            Self::Handwriting => write!(f, "Handwriting"),
            Self::Monospace => write!(f, "Monospace"),
        }
    }
}
