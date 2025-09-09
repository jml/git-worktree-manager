use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod core;
mod git;
mod output;

use commands::cleanup::CleanupCommand;
use commands::list::ListCommand;
use commands::remove::RemoveCommand;
use commands::sync::SyncCommand;

#[derive(Parser)]
#[command(name = "git-worktree-manager")]
#[command(about = "An opinionated git worktree management tool")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[command(flatten)]
    pub list: ListCommand,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show all work-in-progress (non-main) worktrees with comprehensive status
    #[command(name = "list")]
    List(ListCommand),
    /// Clean up worktree branches that are candidates for removal
    #[command(name = "cleanup")]
    Cleanup(CleanupCommand),
    /// Remove a specific worktree branch
    #[command(name = "remove")]
    Remove(RemoveCommand),
    /// Fetch remotes for all repositories in parallel
    #[command(name = "sync")]
    Sync(SyncCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::List(cmd)) => cmd.execute().await,
        Some(Commands::Cleanup(cmd)) => cmd.execute().await,
        Some(Commands::Remove(cmd)) => cmd.execute().await,
        Some(Commands::Sync(cmd)) => cmd.execute().await,
        None => cli.list.execute().await,
    }
}
