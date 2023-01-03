#![feature(slice_as_chunks)]
#![feature(option_result_contains)]
#![feature(let_else)]
#![feature(map_try_insert)]
#![feature(type_alias_impl_trait)]

#![allow(stable_features)] // This feature is required to be able to build on NixOS for some reason.
#![feature(let_chains)]

use error_stack::{Report, report, IntoReport, ResultExt};
use errors::CaveripperError;
use rand::random;

pub mod caveinfo;
#[allow(clippy::bool_to_int_with_if)]
pub mod layout;
pub mod render;
pub mod pikmin_math;
pub mod assets;
pub mod sublevel;
pub mod query;
pub mod search;
pub mod errors;
mod pinmap;

pub fn parse_seed(src: &str) -> Result<u32, Report<CaveripperError>> {
    if src.eq_ignore_ascii_case("random") {
        return Ok(random())
    }

    let trimmed = src.strip_prefix("0x").unwrap_or(src);
    if trimmed.len() != 8 {
        Err(report!(CaveripperError::SeedError))
    }
    else {
        u32::from_str_radix(trimmed, 16).into_report().change_context(CaveripperError::SeedError)
    }
}
