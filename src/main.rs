mod error;
mod lean_rpc;
mod proxy;
mod tui;
mod tui_ipc;

use std::{fs, path::PathBuf, process};

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "lean-tui")]
#[command(about = "Standalone TUI infoview for Lean 4")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as LSP proxy between editor and lake serve
    Proxy,
    /// Run TUI viewer (connects to proxy)
    View,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Init tracing to log file (both commands write to same file)
    let log_path = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("lean-tui/proxy.log");

    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(log_file) = fs::File::create(&log_path) {
        let filter = tracing_subscriber::EnvFilter::from_default_env().add_directive(
            "lean_tui=debug"
                .parse()
                .unwrap_or_else(|_| "debug".parse().unwrap()),
        );
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(log_file)
            .with_ansi(true)
            .with_target(false)
            .pretty()
            .init();
    }

    let result = match cli.command {
        Commands::Proxy => proxy::run().await,
        Commands::View => tui::run().await,
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
