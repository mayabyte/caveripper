use crate::caveinfo::CaveInfoError;

#[derive(Debug)]
pub enum SublevelError {
    MissingCaveName,
    MissingFloorNumber,
    UnrecognizedSublevel(String),
}

#[derive(Debug)]
pub enum AssetError {
    SublevelError(SublevelError),
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