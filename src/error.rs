//! Typed application errors.
//!
//! `main` catches a single `AppError` and prints it in red. Error bodies are
//! carried in the error value rather than printed directly by the client —
//! printing to stdout would corrupt the MCP JSON-RPC stream when running as
//! `jenkins mcp serve`.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    /// No auth configured yet — the user must run `jenkins auth save`.
    #[error(
        "You have to configure auth info to use this command.\nRun \"jenkins auth save\" first."
    )]
    NoAuth,

    /// HTTP 401 from Jenkins.
    #[error("Authorization error (HTTP 401). Check your user and API token.")]
    Unauthorized,

    /// HTTP 403 from Jenkins — usually a missing/invalid CSRF crumb, though an
    /// API token in Basic auth normally bypasses that requirement.
    #[error("Forbidden (HTTP 403). Check your token's permissions, or whether CSRF crumb protection requires a different auth path.")]
    Forbidden,

    /// HTTP 404 on a job-scoped path.
    #[error("Job not found: {0}")]
    JobNotFound(String),

    /// The queue item was cancelled before a build started.
    #[error("Build was cancelled while queued: {0}")]
    QueueCancelled(String),

    /// `--timeout` elapsed before the build finished.
    #[error("Timed out waiting for the build to finish.\n{0}")]
    WaitTimeout(String),

    /// Any other non-2xx status. Carries the status code and response body.
    #[error("An error occurred, status code: {0}\n{1}")]
    Status(u16, String),

    /// Jenkins responded successfully but without an expected field/header.
    #[error("{0}")]
    Api(String),

    /// A required argument was missing or invalid.
    #[error("{0}")]
    Usage(String),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Prompt(#[from] dialoguer::Error),
}
