use anyhow::Result;
use clap::Args;
use futures::future::try_join_all;
use std::fs;
use std::path::Path;

use crate::core::{RepoResult, WorktreeAnalyzer, WorktreeResult, WorktreeStatus};
use crate::git::{GitRepository, SystemGitClient};
use crate::output::table;

#[derive(Args)]
pub struct ShowWipCommand {
    /// Directory to search for repositories (defaults to current directory)
    /// Can also be set via GWM_REPOS_PATH environment variable
    #[arg(short, long, env = "GWM_REPOS_PATH")]
    path: Option<String>,
    /// Disable emoji in status output
    #[arg(long)]
    no_emoji: bool,
}

impl ShowWipCommand {
    pub async fn execute(&self) -> Result<()> {
        let search_path = self.path.as_deref().unwrap_or(".");

        // Find all repositories
        let repo_tasks = self.collect_repositories(search_path).await?;

        // Process repositories in parallel
        let repo_task_results = try_join_all(repo_tasks).await?;

        // Unwrap the results from the join handles
        let mut repo_results = Vec::new();
        for task_result in repo_task_results {
            repo_results.push(task_result?);
        }

        // Use pure functional core to analyze results
        let (total_wip, repos_with_wip, _status_counters, _wip_branches) =
            WorktreeAnalyzer::analyze(&repo_results);

        // Display results as table
        let use_emoji = !self.no_emoji;
        let table_output = table::create_table(&repo_results, use_emoji);
        println!("{}", table_output);

        // Simple summary
        if total_wip > 0 {
            println!();
            println!("Total WIP branches: {}", total_wip);
            println!("Repositories with WIP: {}", repos_with_wip);
        }

        Ok(())
    }

    async fn collect_repositories(
        &self,
        search_path: &str,
    ) -> Result<Vec<tokio::task::JoinHandle<Result<RepoResult>>>> {
        let mut repo_tasks = Vec::new();
        let entries = fs::read_dir(search_path)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let git_path = path.join(".git");
            if !git_path.exists() {
                continue;
            }

            let path_str = path.to_str().unwrap().to_string();

            let task = tokio::spawn(async move { Self::process_repository(path_str).await });
            repo_tasks.push(task);
        }

        Ok(repo_tasks)
    }

    async fn process_repository(repo_path: String) -> Result<RepoResult> {
        let repo_name = Path::new(&repo_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let repo = GitRepository::new(&repo_path, SystemGitClient);

        // Check if it's a bare repository
        if !repo.is_bare().unwrap_or(false) {
            return Ok(RepoResult {
                name: repo_name,
                worktrees: Vec::new(),
            });
        }

        // Get worktree list for this repo
        let worktrees = repo.list_worktrees()?;

        if worktrees.is_empty() {
            return Ok(RepoResult {
                name: repo_name,
                worktrees: Vec::new(),
            });
        }

        // Process all worktrees for this repo
        let mut worktree_results = Vec::new();
        for worktree in worktrees {
            // Get local and remote status only
            let local_status = repo.get_local_status(&worktree.path)?;
            let remote_status = repo.get_remote_status(&worktree.path, &worktree.branch)?;

            worktree_results.push(WorktreeResult {
                branch: worktree.branch.clone(),
                status: WorktreeStatus {
                    local_status,
                    remote_status,
                },
            });
        }

        Ok(RepoResult {
            name: repo_name,
            worktrees: worktree_results,
        })
    }
}
