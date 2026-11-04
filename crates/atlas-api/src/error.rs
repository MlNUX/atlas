// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one error type of the API. Messages are German and meant for the UI.

/// Error of every fallible call.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[cfg_attr(feature = "ffi", derive(uniffi::Error))]
pub enum AtlasError {
    /// A file, folder, entry or id does not exist (any more).
    #[error("{message}")]
    NotFound { message: String },
    /// The input was rejected (name, pattern, value).
    #[error("{message}")]
    Invalid { message: String },
    /// Something exists already or changed meanwhile (name clash, edit conflict).
    #[error("{message}")]
    Conflict { message: String },
    /// Not possible in this build or on this system (e.g. PDF engine on macOS).
    #[error("{message}")]
    Unavailable { message: String },
    /// Login or password needed or rejected.
    #[error("{message}")]
    Auth { message: String },
    /// Network, disk and everything else.
    #[error("{message}")]
    Failed { message: String },
}

pub type Result<T> = std::result::Result<T, AtlasError>;

impl From<anyhow::Error> for AtlasError {
    fn from(err: anyhow::Error) -> Self {
        if let Some(io) = err.downcast_ref::<std::io::Error>()
            && io.kind() == std::io::ErrorKind::NotFound
        {
            return AtlasError::NotFound { message: format!("{err:#}") };
        }
        AtlasError::Failed { message: format!("{err:#}") }
    }
}

impl From<std::io::Error> for AtlasError {
    fn from(err: std::io::Error) -> Self {
        anyhow::Error::from(err).into()
    }
}

/// Placeholder body of the signatures until the area's task fills it in.
#[macro_export]
#[doc(hidden)]
macro_rules! todo_task {
    ($task:literal) => {
        unimplemented!(concat!("atlas-api: implemented in task ", $task))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_files_become_not_found() {
        let err = std::fs::read("/definitely/missing").unwrap_err();
        assert!(matches!(AtlasError::from(err), AtlasError::NotFound { .. }));
        let other = anyhow::anyhow!("kaputt");
        assert_eq!(AtlasError::from(other), AtlasError::Failed { message: "kaputt".into() });
    }
}
