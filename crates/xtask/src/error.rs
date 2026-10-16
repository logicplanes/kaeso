use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("{}: {source}", path.display())]
    File {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot write the report: {0}")]
    Report(#[from] std::io::Error),
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{what} is not valid JSON: {source}")]
    Json {
        what: String,
        source: serde_json::Error,
    },
    #[error("cannot format the time: {0}")]
    Time(#[from] time::error::Format),
    #[error("{0}")]
    Invalid(String),
}

impl Error {
    pub(crate) fn json(what: impl std::fmt::Display, source: serde_json::Error) -> Self {
        Self::Json {
            what: what.to_string(),
            source,
        }
    }
}

pub(crate) fn read(path: &Path) -> Result<Vec<u8>, Error> {
    std::fs::read(path).map_err(|source| Error::File {
        path: path.to_owned(),
        source,
    })
}

pub(crate) fn read_text(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|source| Error::File {
        path: path.to_owned(),
        source,
    })
}

pub(crate) fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::File {
            path: parent.to_owned(),
            source,
        })?;
    }
    std::fs::write(path, contents).map_err(|source| Error::File {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_errors_name_the_path() {
        let missing = Path::new("/nonexistent/kaeso-xtask/file.json");
        let message = read(missing).unwrap_err().to_string();
        assert!(
            message.starts_with("/nonexistent/kaeso-xtask/file.json: "),
            "{message}"
        );
        assert!(read_text(missing).is_err());
        assert!(write(Path::new("/dev/null/kaeso-xtask/file.json"), b"x").is_err());
    }
}
