use thiserror::Error;
use std::{fmt::Debug, num::{ParseIntError, ParseFloatError}, io};

#[derive(Debug, Error, Clone)]
pub enum SublevelError {
    #[error("Couldn't find cave name in input string")]
    MissingCaveName,

    #[error("Couldn't find sublevel number in input string")]
    MissingFloorNumber,

    #[error("Unrecognized sublevel {0}")]
    UnrecognizedSublevel(String),
}

#[derive(Debug, Error, Clone)]
pub enum AssetError {
    #[error("Failed to load an asset for {0}")]
    SublevelError(SublevelError),

    #[error("Error during file IO for '{0}': {1}")]
    IoError(String, io::ErrorKind),

    #[error("Asset cache was missing key '{0}'")]
    CacheError(String),

    #[error("Failed to decode file '{0}'")]
    DecodingError(String),
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

#[derive(Debug, Error, Clone)]
pub enum SeedError {
    #[error("Seed must be 8 digits long, excluding the optional '0x' at the beginning.")]
    InvalidLength,

    #[error("Seed contained invalid hex digits! You can only use 0-9 and A-F (case insensitive).")]
    InvalidHexDigits
}

#[derive(Debug, Error, Clone)]
pub enum SearchConditionError {
    #[error("Error parsing search condition")]
    ParseError,

    #[error("Invalid argument passed to search clause: {0}")]
    InvalidArgument(String),
}

#[derive(Debug, Clone, Error)]
pub enum CaveInfoError {
    #[error("Invalid sublevel '{0}'")]
    InvalidSublevel(String),

    #[error("Malformed line in caveinfo file")]
    MalformedLine,

    #[error("Error parsing value into the appropriate type")]
    ParseValueError,

    #[error("No tag '{0}' in file")]
    NoSuchTag(String),

    #[error("Malformed tag line '{0}'")]
    MalformedTagLine(String),

    #[error("Couldn't read file '{0}'")]
    FileReadError(String),

    #[error("Couldn't find file '{0}'")]
    MissingFileError(String),

    #[error("Failed to parse file '{0}'")]
    ParseFileError(String),

    #[error("Error loading asset during parsing: {0}")]
    AssetError(AssetError),
}

impl From<ParseIntError> for CaveInfoError {
    fn from(_: ParseIntError) -> CaveInfoError {
        CaveInfoError::ParseValueError
    }
}

impl From<ParseFloatError> for CaveInfoError {
    fn from(_: ParseFloatError) -> CaveInfoError {
        CaveInfoError::ParseValueError
    }
}

impl From<AssetError> for CaveInfoError {
    fn from(e: AssetError) -> Self {
        CaveInfoError::AssetError(e)
    }
}

#[derive(Debug, Error, Clone)]
pub enum RenderError {
    #[error("Generated layout '{0} {1}' was invalid")]
    InvalidLayout(String, u32),

    #[error("Issue with file '{0}'")]
    IoError(String),

    #[error("Error loading asset for rendering: {0}")]
    AssetError(AssetError),
}

impl From<AssetError> for RenderError {
    fn from(e: AssetError) -> Self {
        RenderError::AssetError(e)
    }
}
