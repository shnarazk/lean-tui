mod error;
mod lean_rpc;
mod proxy;
mod tui;
mod tui_ipc;

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
    Serve,
    /// Run TUI viewer (connects to proxy)
    Tui,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Only init tracing for serve (TUI uses terminal)
    if matches!(cli.command, Commands::Serve) {
        if let Ok(log_file) = std::fs::File::create("/tmp/lean-tui.log") {
            let filter = tracing_subscriber::EnvFilter::from_default_env().add_directive(
                "lean_tui=info"
                    .parse()
                    .unwrap_or_else(|_| "info".parse().unwrap()),
            );
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(log_file)
                .with_ansi(true)
                .with_target(false)
                .pretty()
                .init();
        }
        // If log file creation fails, continue without file logging
    }

    let result = match cli.command {
        Commands::Serve => proxy::run().await,
        Commands::Tui => tui::run().await,
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
