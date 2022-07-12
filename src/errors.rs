use std::{error::Error, fmt::Display};
use crate::caveinfo::CaveInfoError;

#[derive(Debug)]
pub enum SublevelError {
    MissingCaveName,
    MissingFloorNumber,
    UnrecognizedSublevel(String),
}

impl Error for SublevelError {}

impl Display for SublevelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SublevelError::MissingCaveName => write!(f, "Couldn't find cave name in input string!"),
            SublevelError::MissingFloorNumber => write!(f, "Couldn't find sublevel number in input string!"),
            SublevelError::UnrecognizedSublevel(sl) => write!(f, "Unrecognized sublevel \"{}\"", sl),
        }
    }
}

#[derive(Debug)]
pub enum AssetError {
    SublevelError(SublevelError),
}

impl Error for AssetError {}

impl Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetError::SublevelError(sle) => write!(f, "{}", sle),
        }
    }
}

impl From<CaveInfoError> for AssetError {
    fn from(_: CaveInfoError) -> Self {
        todo!()
    }
}

impl From<SublevelError> for AssetError {
    fn from(e: SublevelError) -> Self {
        AssetError::SublevelError(e)
    }
}

#[derive(Debug)]
pub enum SeedError {
    InvalidLength,
    InvalidHexDigits
}

impl Error for SeedError {}

impl Display for SeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeedError::InvalidLength => write!(f, "Seed must be 8 digits long, excluding the optional '0x' at the beginning."),
            SeedError::InvalidHexDigits => write!(f, "Seed contained invalid hex digits! You can only use 0-9 and A-F."),
        }
    }
}