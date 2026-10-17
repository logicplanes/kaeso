#![forbid(unsafe_code)]

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use kaeso_server::config::{self, Config, ProcessSource, Source};
use kaeso_server::services::users::{OperatorReset, OperatorSignIn};
use secrecy::ExposeSecret;

#[derive(Parser)]
#[command(name = "kaeso", version, about = "The Kaeso gateway", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Run the gateway (the default)")]
    Serve,
    #[command(about = "Run pending PostgreSQL and ClickHouse migrations, then exit")]
    Migrate,
    #[command(subcommand, about = "Create and rotate master keys")]
    MasterKey(MasterKeyCommand),
    #[command(about = "Print a first-start setup link (only while no user exists)")]
    SetupLink,
    #[command(subcommand, about = "Operator actions on user accounts")]
    User(UserCommand),
    #[command(
        hide = true,
        about = "Exit 0 when the local gateway answers `GET /healthz` (for container health checks)"
    )]
    Healthcheck,
    #[command(
        hide = true,
        about = "Print the Platform API's OpenAPI document (`kaeso openapi > openapi.json`)"
    )]
    Openapi,
}

#[derive(Subcommand)]
enum UserCommand {
    #[command(
        name = "reset-2fa",
        about = "Remove a user's TOTP and recovery codes and end their sessions, as a last resort when they lost their device"
    )]
    Reset2fa {
        #[arg(help = "The user's email address")]
        email: String,
    },
    #[command(
        name = "sign-in-link",
        about = "Print a one-time sign-in link for a user, for when no other way to sign in works, such as before single sign-on is set up"
    )]
    SignInLink {
        #[arg(help = "The user's email address")]
        email: String,
    },
}

#[derive(Subcommand)]
enum MasterKeyCommand {
    #[command(about = "Print a new master key as `id:base64` for KAESO_MASTER_KEYS")]
    Generate,
    #[command(
        about = "Re-wrap every stored data key with the active (first) master key and report which key ids are still in use"
    )]
    Rewrap,
}

macro_rules! say {
    ($($arg:tt)*) => {
        writeln!(std::io::stdout(), $($arg)*).context("cannot write to standard output")?
    };
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve) {
        Command::MasterKey(MasterKeyCommand::Generate) => {
            say!("{}", kaeso_crypto::generate_master_key().expose_secret());
            Ok(())
        }
        Command::Healthcheck => runtime()?.block_on(healthcheck(&ProcessSource)),
        Command::Openapi => {
            let doc = kaeso_server::platform::openapi();
            say!(
                "{}",
                doc.to_pretty_json()
                    .context("cannot serialize the OpenAPI document")?
            );
            Ok(())
        }
        other => runtime()?.block_on(run(other)),
    }
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("cannot start the async runtime")
}

fn load_config(source: &dyn Source) -> Result<Config> {
    config::load(source).map_err(|e| anyhow::anyhow!("configuration: {e}"))
}

async fn run(command: Command) -> Result<()> {
    let config = load_config(&ProcessSource)?;
    kaeso_server::telemetry::init_logging(&config.log)?;
    match command {
        Command::Serve => {
            let metrics = kaeso_server::telemetry::init_metrics()?;
            let state = kaeso_server::serve::prepare(config).await?;
            kaeso_server::serve::run(state, metrics, shutdown_signal()).await?;
        }
        Command::Migrate => {
            let state = kaeso_server::serve::prepare(config).await?;
            state.clickhouse.migrate(&state.pg).await?;
            say!("Migrations are up to date.");
        }
        Command::SetupLink => {
            let state = kaeso_server::serve::prepare(config).await?;
            match kaeso_server::setup::issue_link(&state).await? {
                Some(url) => say!(
                    "{url}\n\nThe link works once, for 24 hours. Older setup links no longer work."
                ),
                None => say!("Kaeso is already set up: a user exists. Sign in instead."),
            }
        }
        Command::User(UserCommand::Reset2fa { email }) => {
            let state = kaeso_server::serve::prepare(config).await?;
            match kaeso_server::services::users::operator_reset_2fa(&state, &email).await? {
                OperatorReset::UnknownEmail => anyhow::bail!("no user has the address {email}"),
                OperatorReset::Removed => say!(
                    "Two-factor authentication of {email} was reset and every session ended. They set it up again at their next sign-in."
                ),
                OperatorReset::NothingToRemove => {
                    say!("{email} had no two-factor authentication; every session ended.")
                }
            }
            state.mailer.flush().await;
        }
        Command::User(UserCommand::SignInLink { email }) => {
            let state = kaeso_server::serve::prepare(config).await?;
            match kaeso_server::services::users::operator_sign_in_link(&state, &email).await? {
                OperatorSignIn::UnknownEmail => anyhow::bail!("no user has the address {email}"),
                OperatorSignIn::Deactivated => {
                    anyhow::bail!("the account of {email} is deactivated; reactivate it first")
                }
                OperatorSignIn::Link(url) => say!(
                    "{}\n\nThe link signs {email} in once, within 15 minutes, even while sign-in links by email are off. Anyone who has it can sign in as {email}: open it yourself and don't share it.",
                    url.expose_secret()
                ),
            }
        }
        Command::MasterKey(MasterKeyCommand::Rewrap) => {
            let state = kaeso_server::serve::prepare(config).await?;
            let report =
                kaeso_server::services::master_key::rewrap(&state.pg, &state.keyring).await?;
            let active = state.keyring.active().id().to_owned();
            say!(
                "Re-wrapped {} data keys with the active master key `{active}`.",
                report.rewrapped
            );
            for (id, n) in &report.usage {
                say!("  `{id}` wraps {n} stored secrets");
            }
            let unused: Vec<&str> = state
                .keyring
                .ids()
                .filter(|id| *id != active && !report.usage.contains_key(*id))
                .collect();
            if unused.is_empty() {
                say!("No older master key can be removed yet.");
            } else {
                say!(
                    "Nothing uses {}; remove {} from KAESO_MASTER_KEYS once every instance runs with the new list.",
                    unused
                        .iter()
                        .map(|i| format!("`{i}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    if unused.len() == 1 { "it" } else { "them" }
                );
            }
        }
        Command::MasterKey(MasterKeyCommand::Generate)
        | Command::Healthcheck
        | Command::Openapi => {}
    }
    Ok(())
}

fn probe_address(config: &Config) -> SocketAddr {
    let ip = match config.listen.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        ip => ip,
    };
    SocketAddr::new(ip, config.listen.port())
}

async fn healthcheck(source: &dyn Source) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let probe = probe_address(&load_config(source)?);
    let check = async {
        let mut stream = tokio::net::TcpStream::connect(probe).await?;
        stream
            .write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")
            .await?;
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await?;
        anyhow::Ok(response.starts_with(b"HTTP/1.0 200") || response.starts_with(b"HTTP/1.1 200"))
    };
    match tokio::time::timeout(std::time::Duration::from_secs(3), check).await {
        Ok(Ok(true)) => Ok(()),
        _ => anyhow::bail!("the gateway is not healthy"),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

#[cfg(test)]
mod tests {
    use kaeso_server::config::MapSource;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    fn source(listen: &str) -> MapSource {
        MapSource::new()
            .set("KAESO_PUBLIC_URL", "https://kaeso.example.com")
            .set("KAESO_POSTGRES_URL", "postgres://kaeso@db/kaeso")
            .set("KAESO_CLICKHOUSE_URL", "http://kaeso:pw@clickhouse:8123")
            .set("KAESO_VALKEY_URL", "redis://valkey:6379")
            .set("KAESO_MASTER_KEYS", "k1:AAAA")
            .set("KAESO_MAIL_TRANSPORT", "log")
            .set("KAESO_CONFIG", "/etc/kaeso.toml")
            .file("/etc/kaeso.toml", format!("listen = \"{listen}\"\n"))
    }

    fn probe(listen: &str) -> SocketAddr {
        probe_address(&load_config(&source(listen)).unwrap())
    }

    #[test]
    fn the_probe_uses_the_loopback_of_the_listen_family() {
        assert_eq!(probe("0.0.0.0:9000"), "127.0.0.1:9000".parse().unwrap());
        assert_eq!(probe("[::]:9000"), "[::1]:9000".parse().unwrap());
        assert_eq!(probe("10.0.0.5:9000"), "10.0.0.5:9000".parse().unwrap());
    }

    #[tokio::test]
    async fn the_healthcheck_reaches_the_port_of_the_config_file() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let gateway = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 64];
            let n = stream.read(&mut request).await.unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();
            String::from_utf8_lossy(&request[..n]).into_owned()
        });
        healthcheck(&source(&format!("0.0.0.0:{port}")))
            .await
            .unwrap();
        assert!(gateway.await.unwrap().starts_with("GET /healthz "));
    }

    #[tokio::test]
    async fn the_healthcheck_fails_when_nothing_answers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let err = healthcheck(&source(&format!("0.0.0.0:{port}")))
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "the gateway is not healthy");
    }
}
