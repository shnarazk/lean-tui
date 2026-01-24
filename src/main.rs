mod cursor;
mod error;
mod proxy;
mod tui;

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
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_writer(std::io::stderr)
            .init();
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
