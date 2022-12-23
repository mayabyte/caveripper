use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum CaveInfoError {
    #[error("Missing item")]
    MissingItem,

    #[error("Error parsing value from string")]
    ParseValue,

    #[error("Error parsing a section")]
    ParseSection,

    #[error("Failed to read file")]
    FileRead,

    #[error("Malformed file")]
    MalformedFile,

    #[error("Failed to parse unit file")]
    CaveUnitDefinition,

    #[error("Failed to parse layout file")]
    LayoutFile,

    #[error("Failed to parse waterbox file")]
    WaterboxFile,

    #[error("Failed to parse route file")]
    RouteFile,
}
