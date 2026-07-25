use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unknown target: {0}")]
    UnknownTarget(String),

    #[error("command failed ({code}): {argv:?}")]
    CommandFailed {
        argv: Vec<String>,
        code: i32,
        stderr: String,
    },

    #[error("command timed out: {argv:?}")]
    CommandTimeout { argv: Vec<String> },

    #[error("failed to spawn {argv:?}: {source}")]
    CommandSpawn {
        argv: Vec<String>,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Message(String),
}
