use anyhow::{Result, anyhow};
use clap::Args;
use futures::future::try_join_all;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::{
    PrStatus, RepoResult, WorktreeAnalyzer, WorktreeFilter, WorktreeResult, WorktreeStatus,
};
use crate::git::{GitRepository, SystemGitClient};
use crate::github;
use crate::output::table;

#[derive(Args)]
pub struct GcCommand {
    /// Directory to search for repositories (defaults to current directory)
    /// Can also be set via GWM_REPOS_PATH environment variable
    #[arg(short, long, env = "GWM_REPOS_PATH")]
    path: Option<String>,

    /// Show what would be removed without actually removing anything
    #[arg(long)]
    dry_run: bool,

    /// Disable emoji in status output
    #[arg(long)]
    no_emoji: bool,
}

impl GcCommand {
    pub async fn execute(&self) -> Result<()> {
        // Validate GITHUB_TOKEN early
        std::env::var("GITHUB_TOKEN").map_err(|_| {
            anyhow!(
                "GITHUB_TOKEN environment variable not set. This is required to check PR merge status for garbage collection.\n\nSet it with: export GITHUB_TOKEN=your_token_here"
            )
        })?;

        let search_path = self.path.as_deref().unwrap_or(".");

        // Collect repositories with PR status
        let repo_tasks = self.collect_repositories(search_path).await?;
        let repo_task_results = try_join_all(repo_tasks).await?;

        let mut repo_results = Vec::new();
        for task_result in repo_task_results {
            repo_results.push(task_result?);
        }

        // Filter for GC candidates
        let filter = WorktreeFilter::gc_candidates();
        let candidates = WorktreeAnalyzer::filter_results(&repo_results, &filter);

        // Check if any candidates found
        if candidates.is_empty() {
            println!("No worktrees eligible for garbage collection.");
            println!("(Looking for worktrees that are clean or missing AND have merged PRs)");
            return Ok(());
        }

        // Display candidates
        let use_emoji = !self.no_emoji;
        println!("Garbage collection candidates:");
        let table_output = table::create_table(&candidates, use_emoji, true);
        println!("{}", table_output);
        println!();

        let total_count: usize = candidates.iter().map(|r| r.worktrees.len()).sum();

        // Dry run check
        if self.dry_run {
            if use_emoji {
                println!("🔍 DRY RUN: Would remove {} worktree(s)", total_count);
            } else {
                println!("DRY RUN: Would remove {} worktree(s)", total_count);
            }
            return Ok(());
        }

        // Perform removal (no confirmation - user intent is clear)
        for repo_result in &candidates {
            let repo = GitRepository::new(repo_result.path.to_str().unwrap(), SystemGitClient)?;

            for worktree in &repo_result.worktrees {
                let emoji = if use_emoji { "🗑️  " } else { "" };
                println!("{}Removing {}/{}", emoji, repo_result.name, worktree.branch);

                repo.remove_worktree(&worktree.branch)?;
            }
        }

        let emoji = if use_emoji { "✅ " } else { "" };
        println!("{}Successfully removed {} worktree(s)", emoji, total_count);

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

        let repo = GitRepository::new(&repo_path, SystemGitClient)?;

        // Check if it's a bare repository
        if !repo.is_bare().unwrap_or(false) {
            return Ok(RepoResult {
                name: repo_name,
                path: PathBuf::from(&repo_path),
                worktrees: Vec::new(),
            });
        }

        // Get worktree list for this repo
        let worktrees = repo.list_worktrees()?;

        if worktrees.is_empty() {
            return Ok(RepoResult {
                name: repo_name,
                path: PathBuf::from(&repo_path),
                worktrees: Vec::new(),
            });
        }

        // Fetch PR data
        let pr_matches: HashMap<String, PrStatus> =
            Self::fetch_pr_data_for_repo(&repo_path, &worktrees).await?;

        // Process all worktrees for this repo
        let mut worktree_results = Vec::new();
        for worktree in worktrees {
            // Get all status information
            let local_status = repo.get_local_status(&worktree.path)?;
            let commit_timestamp = repo
                .get_last_commit_timestamp(&worktree.path, &worktree.branch)
                .unwrap_or(0);
            let directory_mtime = repo.get_directory_mtime(&worktree.path).unwrap_or(0);
            let commit_summary = repo
                .get_commit_summary(&worktree.path, &worktree.branch)
                .unwrap_or_else(|_| "<no commit>".to_string());

            // Get PR status for this branch
            let pr_status = pr_matches.get(&worktree.branch).cloned();

            worktree_results.push(WorktreeResult {
                branch: worktree.branch.clone(),
                status: WorktreeStatus {
                    local_status,
                    commit_timestamp,
                    directory_mtime,
                    commit_summary,
                    pr_status,
                },
            });
        }

        Ok(RepoResult {
            name: repo_name,
            path: PathBuf::from(&repo_path),
            worktrees: worktree_results,
        })
    }

    async fn fetch_pr_data_for_repo(
        repo_path: &str,
        worktrees: &[crate::git::WorktreeInfo],
    ) -> Result<HashMap<String, PrStatus>> {
        // Validate GITHUB_TOKEN is present
        std::env::var("GITHUB_TOKEN")
            .map_err(|_| anyhow!("GITHUB_TOKEN environment variable not set"))?;

        // Create a new repo instance for this async context
        let repo = GitRepository::new(repo_path, SystemGitClient)?;

        // Get upstream remote URL
        let remote_url = repo
            .get_upstream_remote_url()?
            .ok_or_else(|| anyhow!("No upstream or origin remote found"))?;

        // Parse GitHub repo from URL
        let github_repo = github::parse_github_url(&remote_url)?;

        // Determine the earliest worktree creation time
        let since_timestamp = Self::get_earliest_worktree_time(repo_path, worktrees).await?;

        // Create GitHub client
        let github_client = octocrab::Octocrab::builder()
            .personal_token(std::env::var("GITHUB_TOKEN")?)
            .build()?;

        // Fetch PRs for this repository
        let prs = github::fetch_prs_for_repo(&github_client, &github_repo, since_timestamp).await?;

        // Extract branch names from worktrees
        let branch_names: Vec<String> = worktrees.iter().map(|wt| wt.branch.clone()).collect();

        // Match worktrees to PRs
        let matches = github::match_worktrees_to_prs(&branch_names, &prs);

        Ok(matches)
    }

    async fn get_earliest_worktree_time(
        repo_path: &str,
        worktrees: &[crate::git::WorktreeInfo],
    ) -> Result<i64> {
        let repo = GitRepository::new(repo_path, SystemGitClient)?;
        let mut earliest_time: Option<i64> = None;

        for worktree in worktrees {
            if let Ok(Some(birth_time)) = repo.get_worktree_birth_time(&worktree.path) {
                earliest_time = Some(match earliest_time {
                    None => birth_time,
                    Some(current) => current.min(birth_time),
                });
            }
        }

        // If we have a birth time, use it; otherwise fall back to 1 week ago
        Ok(earliest_time.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                - (7 * 24 * 60 * 60)
        }))
    }
}
