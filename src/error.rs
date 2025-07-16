//! Error types for ejlv_builder.

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// IDF Process failed
    #[error("Error spawning IDF process {0}")]
    IDFError(String),

    /// I/O operation failed.
    #[error(transparent)]
    IO(#[from] std::io::Error),

    /// Error inside BuilderSDK
    #[error(transparent)]
    BuilderSDK(#[from] ej_builder_sdk::error::Error),
}
