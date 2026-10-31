#![forbid(unsafe_code)]

mod error;
mod json;
mod kpmr;
mod record;
mod signed_urls;
#[cfg(test)]
mod testing;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", bin_name = "cargo xtask", about = "Repository tasks for Kaeso", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(
        about = "Refresh the KPMR catalogue in crates/kaeso-core/data after checking its signature"
    )]
    Kpmr {
        #[arg(long, env = "KPMR_URL", default_value = kaeso_core::kpmr::DEFAULT_URL, help = "Where to download KPMR's catalogue")]
        url: String,
        #[arg(
            long,
            env = "KPMR_PUBLIC_KEYS",
            help = "More keys to trust besides the built-in one, as <key id>:<base64 Ed25519 public key>, comma-separated"
        )]
        public_keys: Option<String>,
    },
    #[command(
        about = "Record real provider responses for the request fixtures of one dialect",
        after_help = "--provider names a provider in crates/kaeso-core/src/provider.rs that speaks the dialect. Its key comes from <ID>_API_KEY, the id in capitals with underscores for hyphens (OPENAI_API_KEY, FIREWORKS_AI_API_KEY), and is never written; <ID>_BASE_URL overrides its default base URL. openai and anthropic record into the dialect's folders, every other provider into <dialect>/providers/<id>/. Review every recorded file before committing it."
    )]
    Record(record::Args),
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let root = root();
    let mut out = std::io::stdout();
    let result = match Cli::parse().command {
        Command::Kpmr { url, public_keys } => match kpmr::trusted_keys(public_keys.as_deref()) {
            Ok(keys) => {
                kpmr::refresh(&kpmr::Paths::in_repository(&root), &url, &keys, &mut out).await
            }
            Err(e) => Err(e),
        },
        Command::Record(args) => match args.provider() {
            Ok(provider) => {
                let endpoint = record::Endpoint::from_env(provider);
                record::run(&root, &args, &endpoint, &mut out).await
            }
            Err(e) => Err(e),
        },
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "error: {e}");
            ExitCode::FAILURE
        }
    }
}
