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

    #[error("Invalid sublevel string {0}")]
    InvalidSublevelString(String),

    #[error("Parsing error: {0}")]
    ParseError(String),

    #[error(transparent)]
    AssetError(#[from] Box<AssetError>),
}

#[derive(Debug, Error, Clone)]
pub enum AssetError {
    #[error("Asset manager has not been initialized!")]
    Uninitialized,

    #[error(transparent)]
    SublevelError(#[from] Box<SublevelError>),

    #[error("Error during file IO for '{0}': {1}")]
    IoError(String, io::ErrorKind),

    #[error("Asset cache was missing key '{0}'")]
    CacheError(String),

    #[error("Failed to decode file '{0}'")]
    DecodingError(String),

    #[error("Failed to parse CaveInfo file {0}: '{1}'")]
    CaveInfoError(String, Box<CaveInfoError>),

    #[error("Files for game '{0}' have not been extracted!")]
    MissingGameError(String),

    #[error("CaveConfig failed to parse: {0}")]
    CaveConfigError(String),
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
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Unrecognized unit name: \"{0}\"")]
    UnrecognizedUnitName(String),

    #[error("Unrecognized entity name: \"{0}\"")]
    UnrecognizedEntityName(String),

    #[error("Unrecognized name: \"{0}\"")]
    UnrecognizedName(String),

    #[error(transparent)]
    AssetError(#[from] AssetError),

    #[error(transparent)]
    SublevelError(#[from] SublevelError),
}

impl From<ParseIntError> for SearchConditionError {
    fn from(e: ParseIntError) -> SearchConditionError {
        SearchConditionError::ParseError(e.to_string())
    }
}

impl From<ParseFloatError> for SearchConditionError {
    fn from(e: ParseFloatError) -> SearchConditionError {
        SearchConditionError::ParseError(e.to_string())
    }
}

#[derive(Debug, Clone, Error)]
pub enum CaveInfoError {
    #[error("Invalid sublevel '{0}'")]
    InvalidSublevel(String),

    #[error("Malformed Section: {0}")]
    MalformedSection(String),

    #[error("Malformed line in caveinfo file: '{0}'")]
    MalformedInfoLine(String),

    #[error("Error parsing value into the appropriate type: {0}")]
    ParseValueError(String),

    #[error("No tag '{0}' in file")]
    NoSuchTag(String),

    #[error("Malformed tag line '{0}'")]
    MalformedTagLine(String),

    #[error("Couldn't read file '{0}'")]
    FileReadError(String),

    #[error("Couldn't find file '{0}'")]
    MissingFileError(String),

    #[error("Error loading asset during parsing: {0}")]
    AssetError(Box<AssetError>),

    #[error("Nom Error: {0}")]
    NomError(String),
}

impl From<ParseIntError> for CaveInfoError {
    fn from(e: ParseIntError) -> CaveInfoError {
        CaveInfoError::ParseValueError(e.to_string())
    }
}

impl From<ParseFloatError> for CaveInfoError {
    fn from(e: ParseFloatError) -> CaveInfoError {
        CaveInfoError::ParseValueError(e.to_string())
    }
}

impl From<AssetError> for CaveInfoError {
    fn from(e: AssetError) -> Self {
        CaveInfoError::AssetError(Box::new(e))
    }
}

// impl From<nom::Err<(&str, nom::error::ErrorKind)>> for CaveInfoError {
//     fn from(e: nom::Err<(&str, nom::error::ErrorKind)>) -> Self {
//         CaveInfoError::NomError(e.to_string())
//     }
// }

impl<'a> From<nom::Err<nom::error::Error<&'a str>>> for CaveInfoError {
    fn from(e: nom::Err<nom::error::Error<&'a str>>) -> Self {
        CaveInfoError::NomError(e.to_string())
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
