#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("XLM only supports Linux x86_64");

mod commands;
mod ui;

use anyhow::Result;
use clap::Parser;
use commands::{install_steam_tool::InstallSteamToolCommand, launch::LaunchCommand};
use std::{env::temp_dir, fs::File};
use tracing::info;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Clone, Parser)]
enum Command {
    Launch(Box<LaunchCommand>),
    InstallSteamTool(InstallSteamToolCommand),
}

#[derive(Debug, Clone, Parser)]
#[command(author, version, about, long_about)]
struct Arguments {
    #[clap(subcommand)]
    command: Command,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = Arguments::parse();
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .compact()
                .with_target(false)
                .with_writer(std::io::stdout)
                .with_filter(
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
                ),
        )
        .with(
            fmt::layer()
                .with_ansi(false)
                .with_writer(File::create(
                    temp_dir().join(format!("{}.log", env!("CARGO_PKG_NAME"))),
                )?)
                .with_filter(EnvFilter::new("debug")),
        )
        .init();
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install default rustls crypto provider");

    info!("TLM v{}", env!("CARGO_PKG_VERSION"));

    // Run the command.
    match args.command {
        Command::Launch(cmd) => cmd.run().await,
        Command::InstallSteamTool(cmd) => cmd.run().await,
    }
}
