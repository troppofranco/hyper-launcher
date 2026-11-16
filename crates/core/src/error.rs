use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("cannot determine the user's data directory")]
    NoDataDir,

    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid TOML in {path}: {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("cannot serialize TOML: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("unsafe path {0:?}")]
    UnsafePath(String),
}

impl CoreError {
    /// Costruttore comodo da usare con `map_err(|e| CoreError::io(path, e))`.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
