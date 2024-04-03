use std::{fmt::Display, sync::OnceLock};

use error_stack::{report, Result, ResultExt};
use itertools::Itertools;
use regex::Regex;
use serde::Serialize;

use crate::{
    assets::{CaveConfig, AssetManager},
    errors::CaveripperError,
};

pub static DIRECT_MODE_TAG: &str = "caveinfo";

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

    pub fn try_from_str(input: &str, mgr: &impl AssetManager) -> Result<Self, CaveripperError> {
        let component_re = SUBLEVEL_COMPONENT.get_or_init(|| Regex::new(r"([.[^-]]+)").unwrap());

        let (game, input) = input
            .split_once(':')
            .map(|(game, rest)| (Some(game.trim().to_ascii_lowercase()), rest.to_ascii_lowercase()))
            .unwrap_or_else(|| (None, input.to_ascii_lowercase()));

        let components = component_re.find_iter(&input).map(|c| c.as_str().trim()).collect_vec();

        match *components.as_slice() {
            // Short sublevel specifier (e.g. "SH6") or full name specifier (e.g. "Snagret Hole 6")
            [c1] => {
                if c1.eq_ignore_ascii_case("colossal") {
                    Ok(Sublevel {
                        cfg: mgr
                            .find_cave_cfg("colossal", game.as_deref(), false)
                            .change_context(CaveripperError::UnrecognizedSublevel)?
                            .clone(),
                        floor: 1,
                    })
                } else {
                    let (name, floor) = from_short_specifier(c1)?;
                    Ok(Sublevel {
                        cfg: mgr
                            .find_cave_cfg(name, game.as_deref(), false)
                            .change_context(CaveripperError::UnrecognizedSublevel)?
                            .clone(),
                        floor,
                    })
                }
            }

            // Short sublevel specifier with explicit Challenge Mode qualifier
            [c1, c2] if c1 == "ch" || c1 == "cm" => {
                let (name, floor) = from_short_specifier(c2)?;
                Ok(Sublevel {
                    cfg: mgr
                        .find_cave_cfg(name, game.as_deref(), true)
                        .change_context(CaveripperError::UnrecognizedSublevel)?
                        .clone(),
                    floor,
                })
            }

            // Long sublevel specifier ("SH-6") Challenge Mode index specifier ("CH24-1"),
            [c1, c2] => {
                let floor = c2.trim().parse::<usize>().change_context(CaveripperError::UnrecognizedSublevel)?;

                Ok(Sublevel {
                    cfg: mgr
                        .find_cave_cfg(c1.trim(), game.as_deref(), false)
                        .change_context(CaveripperError::UnrecognizedSublevel)?
                        .clone(),
                    floor,
                })
            }

            // Direct mode caveinfo+unitfile specifier
            [caveinfo_path, _unitfile_path, floor] if game.is_some_and(|game_name| game_name.eq_ignore_ascii_case(DIRECT_MODE_TAG)) => {
                let floor = floor
                    .trim()
                    .parse::<usize>()
                    .change_context(CaveripperError::UnrecognizedSublevel)?;
                Ok(Sublevel {
                    cfg: CaveConfig {
                        game: DIRECT_MODE_TAG.into(),
                        full_name: format!("[Direct] {caveinfo_path}"),
                        is_challenge_mode: caveinfo_path.starts_with("ch"),
                        shortened_names: vec!["direct".to_string()],
                        caveinfo_filename: caveinfo_path.into(),
                    },
                    floor,
                })
            }

            _ => Err(report!(CaveripperError::UnrecognizedSublevel)),
        }
    }

    /// Constructs the normalized name of this sublevel, i.e. one in the
    /// same format that JHawk's CaveGen implementation accepts (i.e. "SCx-3" - proper capitalization
    /// and hyphenated sublevel number.) The first entry in the shortened names list in cave_config.txt
    /// should always be the normalized cave name.
    pub fn normalized_name(&self) -> String {
        format!("{}-{}", self.cfg.shortened_names.first().unwrap(), self.floor)
    }

    /// Constructs the short cave name of this sublevel, e.g. "SCx3" with no hyphen.
    /// For challenge mode sublevels, this forwards to the normalized_name implementation.
    pub fn short_name(&self) -> String {
        if self.cfg.is_challenge_mode {
            self.normalized_name()
        } else {
            format!("{}{}", self.cfg.shortened_names.first().unwrap(), self.floor)
        }
    }

    /// Constructs the long name of this sublevel, e.g. "Subterranean Complex 3" with the full cave name.
    pub fn long_name(&self) -> String {
        format!("{} {}", self.cfg.full_name, self.floor)
    }

    pub fn is_challenge_mode(&self) -> bool {
        self.cfg.is_challenge_mode
    }
}

static DIGIT: OnceLock<Regex> = OnceLock::new();
static WORDS: OnceLock<Regex> = OnceLock::new();
static SUBLEVEL_COMPONENT: OnceLock<Regex> = OnceLock::new();

impl Serialize for Sublevel {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.short_name())
    }
}

fn from_short_specifier(input: &str) -> Result<(&str, usize), CaveripperError> {
    let words_re = WORDS.get_or_init(|| Regex::new(r"([[[:alpha:]]\s'_]+)").unwrap());
    let number_re = DIGIT.get_or_init(|| Regex::new(r"(\d+)").unwrap());

    let cave_name = words_re.find(input).ok_or(CaveripperError::UnrecognizedSublevel)?.as_str().trim();
    let floor = number_re
        .find(input)
        .ok_or(CaveripperError::UnrecognizedSublevel)?
        .as_str()
        .trim()
        .parse::<usize>()
        .change_context(CaveripperError::UnrecognizedSublevel)?;

    Ok((cave_name, floor))
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

impl Display for Sublevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_name())
    }
}
