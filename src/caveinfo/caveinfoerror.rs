use std::{
    error::Error,
    fmt::{Debug, Display, Formatter},
    num::{ParseFloatError, ParseIntError},
};

#[derive(Debug, Clone)]
pub enum CaveInfoError {
    InvalidSublevel(String),
    MalformedLine,
    ParseValueError,
    NoSuchTag(String),
    MalformedTagLine(String),
}

impl Error for CaveInfoError {}

impl Display for CaveInfoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Doesn't need to be pretty
        Debug::fmt(self, f)
    }
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
