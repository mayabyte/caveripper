use crate::errors::SublevelError;
use crate::assets::{ASSETS, CaveConfig};
use regex::Regex;
use once_cell::sync::OnceCell;

/// Uniquely represents a sublevel and handles parsing to and from strings
/// for sublevel specifiers.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Sublevel {
    pub(crate) cfg: CaveConfig,
    pub(crate) floor: usize,
}

impl Sublevel {
    pub fn from_cfg(cfg: &CaveConfig, floor: usize) -> Sublevel {
        Sublevel { cfg: cfg.clone(), floor }
    }

    /// Constructs the normalized name of this sublevel, i.e. one in the
    /// same format that JHawk's CaveGen implementation accepts (i.e. "SCx-3" - proper capitalization
    /// and hyphenated sublevel number.) The first entry in the shortened names list in cave_config.txt 
    /// should always be the normalized cave name.
    pub fn normalized_name(&self) -> String {
        format!("{}-{}", self.cfg.shortened_names.first().unwrap(), self.floor)
    }

    /// Constructs the short cave name of this sublevel, e.g. "SCx3" with no hyphen.
    pub fn short_name(&self) -> String {
        format!("{}{}", self.cfg.shortened_names.first().unwrap(), self.floor)
    }

    /// Constructs the long name of this sublevel, e.g. "Subterranean Complex 3" with the full cave name.
    pub fn long_name(&self) -> String {
        format!("{} {}", self.cfg.full_name, self.floor)
    }
}

static DIGIT: OnceCell<Regex> = OnceCell::new();
static CHAR: OnceCell<Regex> = OnceCell::new();

impl TryFrom<&str> for Sublevel {
    type Error = SublevelError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let value_lower = value.to_ascii_lowercase();
        let cave = CHAR.get_or_init(|| Regex::new(r"([[:alpha:]]+)").unwrap()).find(&value_lower)
            .ok_or_else(|| SublevelError::MissingCaveName)?
            .as_str();
        let floor: usize = DIGIT.get_or_init(|| Regex::new(r"(\d+)").unwrap()).find(&value_lower)
            .ok_or_else(|| SublevelError::MissingFloorNumber)?
            .as_str()
            .parse().unwrap();

        let cfg = ASSETS.find_cave_cfg(cave);
        if let Some(cfg) = cfg {
            Ok(Sublevel {
                cfg: cfg.clone(),
                floor
            })
        }
        else {
            Err(SublevelError::UnrecognizedSublevel(value.to_string()))
        }
    }
}

impl Ord for Sublevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.normalized_name().cmp(&other.normalized_name())
    }
}

impl PartialOrd for Sublevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.normalized_name().partial_cmp(&other.normalized_name())
    }
}
