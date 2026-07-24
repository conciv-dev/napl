//! The CLI's single user-facing error type. Its `Display` is the message that
//! is printed to stderr as `napl: {message}`, matching the TypeScript CLI.

/// A user-facing CLI error carrying the exact message text.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct CliError(pub String);

impl CliError {
    /// Build a [`CliError`] from anything displayable.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl From<napl_core::schemas::SchemaError> for CliError {
    fn from(error: napl_core::schemas::SchemaError) -> Self {
        // Surface the bare message (without the variant prefix) to match the
        // TypeScript CLI, whose thrown errors carry no such prefix.
        let message = match error {
            napl_core::schemas::SchemaError::Deserialize(message)
            | napl_core::schemas::SchemaError::Validation(message) => message,
        };
        Self(message)
    }
}

impl From<std::io::Error> for CliError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

/// A convenient result alias for command functions.
pub type CliResult<T> = Result<T, CliError>;
