use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

pub struct FlakyValkey {
    pub url: String,
    up: watch::Sender<bool>,
    drop_next_reply: Arc<AtomicBool>,
}

impl FlakyValkey {
    pub async fn start() -> FlakyValkey {
        let upstream = kaeso_testkit::services::valkey_url();
        let upstream = upstream.trim_start_matches("redis://").to_owned();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let (up, watcher) = watch::channel(true);
        let drop_next_reply = Arc::new(AtomicBool::new(false));
        let armed = drop_next_reply.clone();
        tokio::spawn(async move {
            loop {
                let Ok((client, _)) = listener.accept().await else {
                    return;
                };
                if !*watcher.borrow() {
                    continue;
                }
                let upstream = upstream.clone();
                let watcher = watcher.clone();
                let armed = armed.clone();
                tokio::spawn(async move {
                    let Ok(server) = TcpStream::connect(upstream).await else {
                        return;
                    };
                    relay(client, server, watcher, armed).await;
                });
            }
        });
        FlakyValkey {
            url: format!("redis://{addr}"),
            up,
            drop_next_reply,
        }
    }

    pub fn cut(&self) {
        self.up.send_replace(false);
    }

    pub fn restore(&self) {
        self.up.send_replace(true);
    }

    pub fn drop_before_next_reply(&self) {
        self.drop_next_reply.store(true, Ordering::SeqCst);
    }
}

async fn relay(
    client: TcpStream,
    server: TcpStream,
    mut watcher: watch::Receiver<bool>,
    drop_next_reply: Arc<AtomicBool>,
) {
    let (mut from_client, mut to_client) = client.into_split();
    let (mut from_server, mut to_server) = server.into_split();
    let upstream = async move {
        let mut buf = [0_u8; 16 * 1024];
        while let Ok(n) = from_client.read(&mut buf).await {
            if n == 0 || to_server.write_all(&buf[..n]).await.is_err() {
                return;
            }
        }
    };
    let downstream = async move {
        let mut buf = [0_u8; 16 * 1024];
        while let Ok(n) = from_server.read(&mut buf).await {
            if n == 0 {
                return;
            }
            if drop_next_reply.swap(false, Ordering::SeqCst) {
                return;
            }
            if to_client.write_all(&buf[..n]).await.is_err() {
                return;
            }
        }
    };
    tokio::select! {
        () = upstream => {}
        () = downstream => {}
        _ = watcher.wait_for(|up| !*up) => {}
    }
}
