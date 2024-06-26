use std::fmt::Debug;

use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum CaveripperError {
    #[error("Couldn't construct CaveInfo")]
    CaveinfoError,

    #[error("Unrecognized sublevel")]
    UnrecognizedSublevel,

    #[error("Unrecognized game")]
    UnrecognizedGame,

    #[error("Layout generation failed")]
    LayoutGenerationError,

    #[error("Couldn't parse query string")]
    QueryParseError,

    #[error("Failed to load asset")]
    AssetLoadingError,

    #[error("Asset manager has not been initialized!")]
    AssetMgrUninitialized,

    #[error("Image rendering error")]
    RenderingError,

    #[error("Invalid seed string")]
    SeedError,
}
