use anyhow::Result;
use clap::Args;
use futures::future::try_join_all;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::core::{RepoResult, WorktreeAnalyzer, WorktreeFilter, WorktreeResult};
use crate::git::{GitRepository, LocalStatus, RemoteStatus, SystemGitClient};
use crate::output::table;

#[derive(Args)]
pub struct CleanupCommand {
    /// Directory to search for repositories (defaults to current directory)
    /// Can also be set via GWM_REPOS_PATH environment variable
    #[arg(short, long, env = "GWM_REPOS_PATH")]
    path: Option<String>,

    /// Show what would be cleaned up without actually removing anything
    #[arg(long)]
    dry_run: bool,

    /// Skip confirmation prompts (use with caution)
    #[arg(long)]
    force: bool,

    /// Allow pruning branches that haven't been pushed to remote
    #[arg(long)]
    allow_unpushed: bool,

    /// Use custom filter instead of default cleanup candidates
    /// If not specified, uses the prune-candidates preset filter
    #[arg(long)]
    custom_filter: bool,

    // Custom filter options (only used if custom_filter is true)
    /// Show only branches with clean working directories
    #[arg(long)]
    clean: bool,
    /// Show only branches that appear to be merged
    #[arg(long)]
    likely_merged: bool,
    /// Show only branches older than specified time (e.g., 30, 30d, 1w, 2m)
    #[arg(long)]
    older_than: Option<String>,
}

impl CleanupCommand {
    fn build_filter(&self) -> Result<WorktreeFilter> {
        if self.custom_filter {
            let mut filter = WorktreeFilter::new();

            if self.clean {
                filter.clean = Some(true);
            }
            if self.likely_merged {
                filter.likely_merged = Some(true);
            }
            if let Some(age_str) = &self.older_than {
                let days = WorktreeFilter::parse_age_to_days(age_str)
                    .map_err(|e| anyhow::anyhow!("Invalid --older-than value: {}", e))?;
                filter.older_than_days = Some(days);
            }

            Ok(filter)
        } else {
            // Use default prune candidates filter
            Ok(WorktreeFilter::prune_candidates())
        }
    }

    /// Validate command line arguments
    fn validate_args(&self) -> Result<()> {
        Ok(())
    }

    pub async fn execute(&self) -> Result<()> {
        self.validate_args()?;

        let search_path = self.path.as_deref().unwrap_or(".");
        let filter = self.build_filter()?;

        // Find all repositories and get cleanup candidates
        let repo_tasks = self.collect_repositories(search_path).await?;
        let repo_task_results = try_join_all(repo_tasks).await?;

        let mut repo_results = Vec::new();
        for task_result in repo_task_results {
            repo_results.push(task_result?);
        }

        // Apply filtering to get cleanup candidates
        let candidates = WorktreeAnalyzer::filter_results(&repo_results, &filter);

        if candidates.is_empty() {
            println!("No worktrees found matching cleanup criteria.");
            return Ok(());
        }

        // Show candidates table
        println!("Cleanup candidates:");
        let table_output = table::create_table(&candidates, true);
        println!("{}", table_output);
        println!();

        // Perform safety checks
        let unsafe_branches = self.find_unsafe_branches(&candidates).await?;
        if !unsafe_branches.is_empty() && !self.allow_unpushed {
            println!("⚠️  The following branches have safety concerns and won't be cleaned up:");
            for (repo, branch, reason) in &unsafe_branches {
                println!("  {}/{}: {}", repo, branch, reason);
            }
            println!();
            println!("Use --allow-unpushed to override these safety checks.");
            println!();
        }

        // Filter out unsafe branches unless override is specified
        let safe_candidates = if self.allow_unpushed {
            candidates
        } else {
            self.filter_unsafe_branches(candidates, &unsafe_branches)
        };

        if safe_candidates.is_empty() {
            println!("No safe candidates remaining after safety checks.");
            return Ok(());
        }

        let total_count: usize = safe_candidates.iter().map(|r| r.worktrees.len()).sum();

        if self.dry_run {
            println!("🔍 DRY RUN: Would clean up {} worktree(s)", total_count);
            return Ok(());
        }

        // Get confirmation unless force is specified
        if !self.force {
            print!("❓ Remove {} worktree(s)? [y/N]: ", total_count);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            if !input.trim().to_lowercase().starts_with('y') {
                println!("Cancelled.");
                return Ok(());
            }
        }

        // Perform the actual pruning
        self.cleanup_worktrees(&safe_candidates).await?;

        println!("✅ Successfully cleaned up {} worktree(s)", total_count);
        Ok(())
    }

    async fn find_unsafe_branches(
        &self,
        candidates: &[RepoResult],
    ) -> Result<Vec<(String, String, String)>> {
        let mut unsafe_branches = Vec::new();

        for repo_result in candidates {
            for worktree in &repo_result.worktrees {
                // Check if branch has uncommitted changes
                if matches!(
                    worktree.status.local_status,
                    LocalStatus::Dirty | LocalStatus::Staged
                ) {
                    unsafe_branches.push((
                        repo_result.name.clone(),
                        worktree.branch.clone(),
                        "has uncommitted changes".to_string(),
                    ));
                    continue;
                }

                // Check if branch has unpushed commits
                if matches!(
                    worktree.status.remote_status,
                    RemoteStatus::NotPushed | RemoteStatus::Ahead(_)
                ) {
                    unsafe_branches.push((
                        repo_result.name.clone(),
                        worktree.branch.clone(),
                        "has unpushed commits".to_string(),
                    ));
                    continue;
                }

                // Check if worktree directory is missing
                if matches!(worktree.status.local_status, LocalStatus::Missing) {
                    // This is actually safe to clean up, since directory is already gone
                    continue;
                }
            }
        }

        Ok(unsafe_branches)
    }

    fn filter_unsafe_branches(
        &self,
        candidates: Vec<RepoResult>,
        unsafe_branches: &[(String, String, String)],
    ) -> Vec<RepoResult> {
        let mut filtered_results = Vec::new();

        for repo_result in candidates {
            let safe_worktrees: Vec<WorktreeResult> = repo_result
                .worktrees
                .into_iter()
                .filter(|worktree| {
                    !unsafe_branches.iter().any(|(repo, branch, _)| {
                        repo == &repo_result.name && branch == &worktree.branch
                    })
                })
                .collect();

            if !safe_worktrees.is_empty() {
                filtered_results.push(RepoResult {
                    name: repo_result.name,
                    path: repo_result.path,
                    worktrees: safe_worktrees,
                });
            }
        }

        filtered_results
    }

    async fn cleanup_worktrees(&self, candidates: &[RepoResult]) -> Result<()> {
        for repo_result in candidates {
            let repo = GitRepository::new(repo_result.path.to_str().unwrap(), SystemGitClient)?;

            for worktree in &repo_result.worktrees {
                println!("🗑️  Cleaning up {}/{}", repo_result.name, worktree.branch);

                // Remove the worktree
                repo.remove_worktree(&worktree.branch)?;
            }
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

        // Process all worktrees for this repo
        let mut worktree_results = Vec::new();
        for worktree in worktrees {
            // Get all status information
            let local_status = repo.get_local_status(&worktree.path)?;
            let remote_status = repo.get_remote_status(&worktree.path, &worktree.branch)?;
            let commit_timestamp = repo
                .get_last_commit_timestamp(&worktree.path, &worktree.branch)
                .unwrap_or(0);
            let directory_mtime = repo.get_directory_mtime(&worktree.path).unwrap_or(0);
            let merge_status = repo
                .get_merge_status(&worktree.path, &worktree.branch, commit_timestamp)
                .unwrap_or(crate::git::MergeStatus::Unknown);

            worktree_results.push(WorktreeResult {
                branch: worktree.branch.clone(),
                status: crate::core::WorktreeStatus {
                    local_status,
                    remote_status,
                    commit_timestamp,
                    directory_mtime,
                    merge_status,
                },
            });
        }

        Ok(RepoResult {
            name: repo_name,
            path: PathBuf::from(&repo_path),
            worktrees: worktree_results,
        })
    }
}
