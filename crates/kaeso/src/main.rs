#![forbid(unsafe_code)]

use std::io::Write;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use kaeso_server::config::{self, ProcessSource};
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
        Command::Healthcheck => runtime()?.block_on(healthcheck()),
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

async fn run(command: Command) -> Result<()> {
    let config = config::load(&ProcessSource).map_err(|e| anyhow::anyhow!("configuration: {e}"))?;
    kaeso_server::telemetry::init_logging(&config.log).map_err(anyhow::Error::msg)?;
    match command {
        Command::Serve => {
            let metrics = kaeso_server::telemetry::init_metrics().map_err(anyhow::Error::msg)?;
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
            let user = kaeso_store::pg::users::get_by_email(&state.pg, &email)
                .await?
                .ok_or_else(|| anyhow::anyhow!("no user has the address {email}"))?;
            let id = kaeso_core::id::UserId::from_uuid(user.id);
            let mut tx = state.pg.begin().await?;
            let had = kaeso_server::services::users::reset_2fa(
                &state,
                &mut tx,
                id,
                kaeso_mail::Actor::Operator,
            )
            .await?;
            kaeso_server::platform::audit::system(
                &mut tx,
                None,
                "user.2fa_reset",
                Some(("user", id.to_string())),
                None,
                state.clock.now(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
            tx.commit().await?;
            if had {
                say!(
                    "Two-factor authentication of {email} was reset and every session ended. They set it up again at their next sign-in."
                );
            } else {
                say!("{email} had no two-factor authentication; every session ended.");
            }
            state.mailer.flush().await;
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

async fn healthcheck() -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listen = std::env::var("KAESO_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let port = listen
        .rsplit(':')
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .context("KAESO_LISTEN has no port")?;
    let check = async {
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
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
