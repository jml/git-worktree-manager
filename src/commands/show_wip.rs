use anyhow::Result;
use clap::Args;
use futures::future::try_join_all;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::core::{RepoResult, WorktreeAnalyzer, WorktreeResult, WorktreeStatus};
use crate::git::{GitRepository, SystemGitClient};
use crate::github::{GitHubIntegration, PrStatus, SystemGitHubClient};
use crate::output::table;

#[derive(Args)]
pub struct ShowWipCommand {
    /// Directory to search for repositories (defaults to current directory)
    #[arg(short, long)]
    path: Option<String>,
    /// Skip GitHub integration for faster execution
    #[arg(long)]
    fast: bool,
    /// Disable emoji in status output
    #[arg(long)]
    no_emoji: bool,
}

impl ShowWipCommand {
    pub async fn execute(&self) -> Result<()> {
        // Check if we're in the expected directory structure
        let search_path = self.path.as_deref().unwrap_or(".");

        if !Path::new(&format!("{}/convert-to-worktree.sh", search_path)).exists() {
            println!("⚠️  Warning: convert-to-worktree.sh not found in current directory");
            println!("   This tool is optimized for the chainguard directory structure.");
            println!();
        }

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
            let fast_mode = self.fast;

            let task =
                tokio::spawn(async move { Self::process_repository(path_str, fast_mode).await });
            repo_tasks.push(task);
        }

        Ok(repo_tasks)
    }

    async fn process_repository(repo_path: String, fast_mode: bool) -> Result<RepoResult> {
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

        // Get GitHub repo info once for this repository (cached within this execution)
        let github_repo = if !fast_mode {
            let main_path = Path::new(&repo_path).join("main");
            if main_path.exists() {
                repo.get_remote_url(main_path.to_str().unwrap())
                    .and_then(|url| GitHubIntegration::<SystemGitHubClient>::get_repo_info(&url))
                    .unwrap_or_default()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Get PR statuses in batch if we have GitHub integration
        let pr_statuses = if !fast_mode && !github_repo.is_empty() {
            let branch_names: Vec<String> = worktrees.iter().map(|w| w.branch.clone()).collect();
            let github_integration = GitHubIntegration::new(SystemGitHubClient);
            github_integration
                .get_batch_pr_status(&github_repo, &branch_names)
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        // Process all worktrees for this repo
        let mut worktree_results = Vec::new();
        for worktree in worktrees {
            // Get comprehensive status
            let local_status = repo.get_local_status(&worktree.path)?;
            let remote_status = repo.get_remote_status(&worktree.path, &worktree.branch)?;
            let pr_status = if fast_mode || github_repo.is_empty() {
                PrStatus::NoGitHub
            } else {
                pr_statuses
                    .get(&worktree.branch)
                    .cloned()
                    .unwrap_or(PrStatus::NoPr)
            };

            worktree_results.push(WorktreeResult {
                branch: worktree.branch.clone(),
                status: WorktreeStatus {
                    local_status,
                    remote_status,
                    pr_status,
                },
            });
        }

        Ok(RepoResult {
            name: repo_name,
            worktrees: worktree_results,
        })
    }
}
