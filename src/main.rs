use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod core;
mod git;
mod output;

use commands::cleanup::CleanupCommand;
use commands::remove::RemoveCommand;
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
    /// Clean up worktree branches that are candidates for removal
    #[command(name = "cleanup")]
    Cleanup(CleanupCommand),
    /// Remove a specific worktree branch
    #[command(name = "remove")]
    Remove(RemoveCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ShowWip(cmd) => cmd.execute().await,
        Commands::Cleanup(cmd) => cmd.execute().await,
        Commands::Remove(cmd) => cmd.execute().await,
    }
}
