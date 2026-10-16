use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(&'static str, &'static str)>,
    pub(crate) body: Vec<u8>,
}

pub(crate) struct Served {
    pub(crate) url: String,
    pub(crate) request: oneshot::Receiver<String>,
}

pub(crate) async fn serve_once(reply: Reply) -> Served {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut seen = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = socket.read(&mut buf).await.unwrap();
            seen.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&seen).into_owned();
            if let Some((head, body)) = text.split_once("\r\n\r\n") {
                let length = head
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(str::to_owned)
                    })
                    .map_or(0, |v| v.trim().parse::<usize>().unwrap());
                if body.len() >= length {
                    tx.send(text).unwrap();
                    break;
                }
            }
            assert!(n > 0, "the client closed the connection early");
        }
        let mut head = format!(
            "HTTP/1.1 {} X\r\ncontent-length: {}\r\nconnection: close\r\n",
            reply.status,
            reply.body.len()
        );
        for (name, value) in reply.headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("\r\n");
        socket.write_all(head.as_bytes()).await.unwrap();
        socket.write_all(&reply.body).await.unwrap();
        socket.shutdown().await.unwrap();
    });
    Served { url, request: rx }
}

pub(crate) struct TempDir(PathBuf);

impl TempDir {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "kaeso-xtask-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    pub(crate) fn file(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path
    }

    pub(crate) fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.0.join(relative)).unwrap()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
