#![feature(slice_as_chunks)]
#![feature(let_else)]
#![feature(map_try_insert)]
#![feature(type_alias_impl_trait)]
#![feature(cell_leak)]
#![allow(stable_features)] // This feature is required to be able to build on NixOS for some reason.
#![feature(let_chains)]

use error_stack::{report, Report, ResultExt};
use errors::CaveripperError;
use rand::random;

// This module style is chosen to keep all related files grouped in the same folder
// without introducing many files named "mod.rs".

#[path = "caveinfo/caveinfo.rs"]
pub mod caveinfo;

#[allow(clippy::bool_to_int_with_if)]
#[path = "layout/layout.rs"]
pub mod layout;

#[path = "query/query.rs"]
pub mod query;

pub mod assets;
pub mod errors;
pub mod pikmin_math;
mod point;
pub mod render;
pub mod sublevel;

pub fn parse_seed(src: &str) -> Result<u32, Report<CaveripperError>> {
    if src.eq_ignore_ascii_case("random") {
        return Ok(random());
    }

    let trimmed = src.strip_prefix("0x").unwrap_or(src);
    if trimmed.len() != 8 {
        Err(report!(CaveripperError::SeedError))
    } else {
        u32::from_str_radix(trimmed, 16).change_context(CaveripperError::SeedError)
    }
}
