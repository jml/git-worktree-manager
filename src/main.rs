use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod core;
mod git;
mod output;

use commands::prune::PruneCommand;
use commands::show_wip::ShowWipCommand;

#[derive(Parser)]
#[command(name = "git-worktree-manager")]
#[command(about = "An opinionated git worktree management tool")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show all work-in-progress (non-main) worktrees with comprehensive status
    #[command(name = "show-wip")]
    ShowWip(ShowWipCommand),
    /// Remove worktree branches that are candidates for pruning
    #[command(name = "prune")]
    Prune(PruneCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ShowWip(cmd) => cmd.execute().await,
        Commands::Prune(cmd) => cmd.execute().await,
    }
}
