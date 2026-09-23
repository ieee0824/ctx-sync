//! Error type shared by all ctx-sync operations.
//!
//! Every error maps to a stable process exit code so that AI agents can
//! react to failures without parsing messages.

/// Errors returned by ctx-sync.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    General(String),
    #[error("Context conflict detected:\n\n{}", files.join("\n"))]
    ContextConflict { files: Vec<String> },
    #[error("authentication error: {0}")]
    Auth(String),
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("remote error: {0}")]
    Remote(String),
    #[error("sync failed: retry limit exceeded")]
    SyncRetryExceeded,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Stable process exit code for this error.
    ///
    /// | code | meaning |
    /// |---|---|
    /// | 1 | general error |
    /// | 2 | context conflict |
    /// | 3 | authentication error |
    /// | 4 | invalid configuration |
    /// | 5 | remote error |
    pub fn exit_code(&self) -> u8 {
        match self {
            Error::General(_) | Error::Io(_) => 1,
            Error::ContextConflict { .. } => 2,
            Error::Auth(_) => 3,
            Error::InvalidConfig(_) => 4,
            Error::Remote(_) | Error::SyncRetryExceeded => 5,
        }
    }
}

/// Result type used throughout ctx-sync.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_are_stable() {
        let cases = [
            (Error::General("x".into()), 1),
            (Error::Io(std::io::Error::other("x")), 1),
            (Error::ContextConflict { files: vec![] }, 2),
            (Error::Auth("x".into()), 3),
            (Error::InvalidConfig("x".into()), 4),
            (Error::Remote("x".into()), 5),
            (Error::SyncRetryExceeded, 5),
        ];
        for (err, code) in cases {
            assert_eq!(err.exit_code(), code, "{err:?}");
        }
    }

    #[test]
    fn context_conflict_message_lists_files() {
        let err = Error::ContextConflict {
            files: vec!["20-architecture.md".into()],
        };
        assert_eq!(
            err.to_string(),
            "Context conflict detected:\n\n20-architecture.md"
        );
    }

    #[test]
    fn context_conflict_message_lists_multiple_files() {
        let err = Error::ContextConflict {
            files: vec!["10-project.md".into(), "20-architecture.md".into()],
        };
        assert_eq!(
            err.to_string(),
            "Context conflict detected:\n\n10-project.md\n20-architecture.md"
        );
    }
}
