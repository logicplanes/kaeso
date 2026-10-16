#![forbid(unsafe_code)]

mod error;
mod json;
mod models_dev;
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
        about = "Refresh the models.dev snapshot in crates/kaeso-core/data and the fetch time that ships with it"
    )]
    ModelsDev {
        #[arg(long, env = "MODELS_DEV_URL", default_value = kaeso_core::models_dev::DEFAULT_URL, help = "Where to download models.dev's api.json")]
        url: String,
    },
    #[command(
        about = "Record real provider responses for the request fixtures of one dialect",
        after_help = "API keys come from ANTHROPIC_API_KEY or OPENAI_API_KEY and are never written; ANTHROPIC_BASE_URL and OPENAI_BASE_URL override the API base URLs. Review every recorded file before committing it."
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
        Command::ModelsDev { url } => {
            models_dev::refresh(&models_dev::Paths::in_repository(&root), &url, &mut out).await
        }
        Command::Record(args) => {
            let endpoint = record::Endpoint::from_env(args.dialect());
            record::run(&root, &args, &endpoint, &mut out).await
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "error: {e}");
            ExitCode::FAILURE
        }
    }
}
