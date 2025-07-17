//! Error types for ejlv_builder.

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// I/O operation failed.
    #[error(transparent)]
    IO(#[from] std::io::Error),

    /// Error inside BuilderSDK
    #[error(transparent)]
    BuilderSDK(#[from] ej_builder_sdk::error::Error),

    /// Serial Port
    #[error(transparent)]
    SerialPort(#[from] tokio_serial::Error),

    /// Serial Port Timeout
    #[error("Timeout Waiting For Benchmark To End - Output: {0}")]
    TimeoutWaitingForBenchmarkToEnd(String),
}
