// bin/ivory/src/main.rs
//! Ivory Chain node

use clap::Parser;

#[derive(Parser)]
#[command(name = "ivory")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    Run,
    Init,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    tracing_subscriber::fmt::init();
    
    match cli.command {
        Commands::Run => {
            println!("Starting Ivory Chain...");
        }
        Commands::Init => {
            println!("Initializing...");
        }
    }
    
    Ok(())
}