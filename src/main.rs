use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod core;
mod git;
mod output;

use commands::add::AddCommand;
use commands::complete_branches::CompleteBranchesCommand;
use commands::complete_repos::CompleteReposCommand;
use commands::completion::CompletionCommand;
use commands::list::ListCommand;
use commands::remove::RemoveCommand;
use commands::switch::SwitchCommand;
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
    /// Add a new worktree branch
    #[command(name = "add")]
    Add(AddCommand),
    /// Remove a specific worktree branch
    #[command(name = "remove")]
    Remove(RemoveCommand),
    /// Switch to a worktree directory
    #[command(name = "switch")]
    Switch(SwitchCommand),
    /// Fetch remotes for all repositories in parallel
    #[command(name = "sync")]
    Sync(SyncCommand),
    /// Generate shell completions
    #[command(name = "completion")]
    Completion(CompletionCommand),
    /// List repository names for completion
    #[command(name = "complete-repos")]
    CompleteRepos(CompleteReposCommand),
    /// List branch names for completion
    #[command(name = "complete-branches")]
    CompleteBranches(CompleteBranchesCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::List(cmd)) => cmd.execute().await,
        Some(Commands::Add(cmd)) => cmd.execute().await,
        Some(Commands::Remove(cmd)) => cmd.execute().await,
        Some(Commands::Switch(cmd)) => cmd.execute().await,
        Some(Commands::Sync(cmd)) => cmd.execute().await,
        Some(Commands::Completion(cmd)) => cmd.execute().await,
        Some(Commands::CompleteRepos(cmd)) => cmd.execute().await,
        Some(Commands::CompleteBranches(cmd)) => cmd.execute().await,
        None => cli.list.execute().await,
    }
}
