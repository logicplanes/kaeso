use std::io::Write;
use std::path::{Path, PathBuf};

use kaeso_update::Version;
use kaeso_update::manifest::Service;

pub const MAX_BYTES: usize = 256 << 10;

#[derive(Debug, thiserror::Error)]
pub enum ComposeError {
    #[error("compose.yaml cannot be read: {0}")]
    Read(String),
    #[error("compose.yaml was replaced since the updater started; restart the updater")]
    Replaced,
    #[error("compose.yaml cannot be written: {0}")]
    Write(String),
    #[error("compose.yaml does not read back as written")]
    Verify,
    #[error("compose.yaml is larger than 256 KiB")]
    TooLarge,
}

fn boundary(b: Option<&u8>) -> bool {
    b.is_none_or(|b| !(b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+' | b'_')))
}

pub fn retag(text: &[u8], from: &Version, to: &Version) -> Vec<u8> {
    let mut out = text.to_vec();
    for service in Service::ALL {
        let old = format!("{}:{from}", service.image()).into_bytes();
        let new = format!("{}:{to}", service.image()).into_bytes();
        let mut result = Vec::with_capacity(out.len());
        let mut i = 0;
        while i < out.len() {
            let rest = out.get(i..).unwrap_or_default();
            if rest.starts_with(&old) && boundary(out.get(i + old.len())) {
                result.extend_from_slice(&new);
                i += old.len();
            } else {
                result.extend(out.get(i).copied());
                i += 1;
            }
        }
        out = result;
    }
    out
}

#[derive(Clone, Debug)]
pub struct ComposeFile {
    path: PathBuf,
}

#[cfg(unix)]
fn still_linked(path: &Path) -> std::io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    Ok(std::fs::metadata(path)?.nlink() > 0)
}

#[cfg(not(unix))]
fn still_linked(_: &Path) -> std::io::Result<bool> {
    Ok(true)
}

impl ComposeFile {
    pub fn new(path: impl Into<PathBuf>) -> ComposeFile {
        ComposeFile { path: path.into() }
    }

    pub fn read(&self) -> Result<Vec<u8>, ComposeError> {
        if !still_linked(&self.path).map_err(|e| ComposeError::Read(e.to_string()))? {
            return Err(ComposeError::Replaced);
        }
        let bytes = std::fs::read(&self.path).map_err(|e| ComposeError::Read(e.to_string()))?;
        if bytes.len() > MAX_BYTES {
            return Err(ComposeError::TooLarge);
        }
        Ok(bytes)
    }

    pub fn writable(&self) -> Result<(), ComposeError> {
        std::fs::OpenOptions::new()
            .append(true)
            .open(&self.path)
            .map(|_| ())
            .map_err(|e| ComposeError::Write(e.to_string()))
    }

    pub fn write(&self, bytes: &[u8]) -> Result<(), ComposeError> {
        if !still_linked(&self.path).map_err(|e| ComposeError::Write(e.to_string()))? {
            return Err(ComposeError::Replaced);
        }
        let write = || -> std::io::Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&self.path)?;
            file.write_all(bytes)?;
            file.sync_all()
        };
        write().map_err(|e| ComposeError::Write(e.to_string()))?;
        if self.read()? != bytes {
            return Err(ComposeError::Verify);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn retag_changes_exactly_the_kaeso_image_tags() {
        let text = b"services:\n  gateway:\n    image: ghcr.io/logicplanes/kaeso-gateway:40.2.0\n  console:\n    image: \"ghcr.io/logicplanes/kaeso-console:40.2.0\"\n  updater:\n    image: ghcr.io/logicplanes/kaeso-updater:40.2.0\n  other:\n    image: ghcr.io/logicplanes/kaeso-gateway:40.2.0-rc.1\n    labels: [ghcr.io/logicplanes/kaeso-gateway:40.2.01]\n  caddy:\n    image: caddy:40.2.0\n";
        let out = String::from_utf8(retag(text, &v("40.2.0"), &v("40.2.1"))).unwrap();
        assert!(out.contains("image: ghcr.io/logicplanes/kaeso-gateway:40.2.1\n"));
        assert!(out.contains("\"ghcr.io/logicplanes/kaeso-console:40.2.1\""));
        assert!(out.contains("kaeso-updater:40.2.1\n"));
        assert!(out.contains("kaeso-gateway:40.2.0-rc.1"));
        assert!(out.contains("kaeso-gateway:40.2.01]"));
        assert!(out.contains("caddy:40.2.0"));
    }

    #[test]
    fn the_file_is_written_in_place_and_read_back() {
        let dir = std::env::temp_dir().join(format!("kaeso-compose-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("compose.yaml");
        std::fs::write(&path, b"old").unwrap();
        let file = ComposeFile::new(&path);
        assert_eq!(file.read().unwrap(), b"old");
        file.writable().unwrap();
        file.write(b"new content").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new content");
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(matches!(file.read(), Err(ComposeError::Read(_))));
        assert!(matches!(file.writable(), Err(ComposeError::Write(_))));
    }

    #[test]
    fn oversized_files_are_refused() {
        let dir = std::env::temp_dir().join(format!("kaeso-compose-big-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("compose.yaml");
        std::fs::write(&path, vec![b'#'; MAX_BYTES + 1]).unwrap();
        assert!(matches!(
            ComposeFile::new(&path).read(),
            Err(ComposeError::TooLarge)
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
