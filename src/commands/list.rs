use anyhow::Result;
use clap::Args;
use futures::future::try_join_all;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::{RepoResult, WorktreeAnalyzer, WorktreeFilter, WorktreeResult, WorktreeStatus};
use crate::git::{GitRepository, SystemGitClient};
use crate::output::table;

#[derive(Args)]
pub struct ListCommand {
    /// Directory to search for repositories (defaults to current directory)
    /// Can also be set via GWM_REPOS_PATH environment variable
    #[arg(short, long, env = "GWM_REPOS_PATH")]
    path: Option<String>,
    /// Disable emoji in status output
    #[arg(long)]
    no_emoji: bool,

    // Preset filters
    /// Show only branches that are likely candidates for pruning (likely-merged, clean, older than 7 days)
    #[arg(long)]
    prune_candidates: bool,
    /// Show only active branches (not-merged, newer than 7 days)
    #[arg(long)]
    active: bool,
    /// Show only branches needing attention (diverged, behind, or missing)
    #[arg(long)]
    needs_attention: bool,
    /// Show only stale branches (older than 30 days)
    #[arg(long)]
    stale: bool,

    // Local status filters
    /// Show only branches with dirty working directories
    #[arg(long)]
    dirty: bool,
    /// Show only branches with clean working directories
    #[arg(long)]
    clean: bool,
    /// Show only branches with staged changes
    #[arg(long)]
    staged: bool,
    /// Show only branches with missing worktree directories
    #[arg(long)]
    missing: bool,

    // Remote status filters
    /// Show only branches that are ahead of their remote
    #[arg(long)]
    ahead: bool,
    /// Show only branches that are behind their remote
    #[arg(long)]
    behind: bool,
    /// Show only branches that have diverged from their remote
    #[arg(long)]
    diverged: bool,
    /// Show only branches that haven't been pushed to remote
    #[arg(long)]
    not_pushed: bool,
    /// Show only branches that exist on remote but aren't tracking
    #[arg(long)]
    not_tracking: bool,
    /// Show only branches that are up to date with their remote
    #[arg(long)]
    up_to_date: bool,

    // Age filters
    /// Show only branches older than the specified time (e.g., 30, 30d, 1w, 2m)
    #[arg(long)]
    older_than: Option<String>,
    /// Show only branches newer than the specified time (e.g., 30, 30d, 1w, 2m)
    #[arg(long)]
    newer_than: Option<String>,
}

impl ListCommand {
    /// Build a WorktreeFilter from command line arguments
    fn build_filter(&self) -> Result<WorktreeFilter> {
        // Handle preset filters first (they override individual filters)
        if self.prune_candidates {
            return Ok(WorktreeFilter::prune_candidates());
        }
        if self.active {
            return Ok(WorktreeFilter::active());
        }
        if self.needs_attention {
            return Ok(WorktreeFilter::needs_attention());
        }
        if self.stale {
            return Ok(WorktreeFilter::stale());
        }

        // Build custom filter from individual flags
        let mut filter = WorktreeFilter::new();

        // Local status filters
        if self.dirty {
            filter.dirty = Some(true);
        }
        if self.clean {
            filter.clean = Some(true);
        }
        if self.staged {
            filter.staged = Some(true);
        }
        if self.missing {
            filter.missing = Some(true);
        }

        // Remote status filters
        if self.ahead {
            filter.ahead = Some(true);
        }
        if self.behind {
            filter.behind = Some(true);
        }
        if self.diverged {
            filter.diverged = Some(true);
        }
        if self.not_pushed {
            filter.not_pushed = Some(true);
        }
        if self.not_tracking {
            filter.not_tracking = Some(true);
        }
        if self.up_to_date {
            filter.up_to_date = Some(true);
        }

        // Age filters
        if let Some(age_str) = &self.older_than {
            let days = WorktreeFilter::parse_age_to_days(age_str)
                .map_err(|e| anyhow::anyhow!("Invalid --older-than value: {}", e))?;
            filter.older_than_days = Some(days);
        }

        if let Some(age_str) = &self.newer_than {
            let days = WorktreeFilter::parse_age_to_days(age_str)
                .map_err(|e| anyhow::anyhow!("Invalid --newer-than value: {}", e))?;
            filter.newer_than_days = Some(days);
        }

        Ok(filter)
    }

    pub async fn execute(&self) -> Result<()> {
        let search_path = self.path.as_deref().unwrap_or(".");

        // Build filter from command line arguments
        let filter = self.build_filter()?;

        // Find all repositories
        let repo_tasks = self.collect_repositories(search_path).await?;

        // Process repositories in parallel
        let repo_task_results = try_join_all(repo_tasks).await?;

        // Unwrap the results from the join handles
        let mut repo_results = Vec::new();
        for task_result in repo_task_results {
            repo_results.push(task_result?);
        }

        // Apply filtering if any filters are active
        let filtered_results = if self.has_filters() {
            WorktreeAnalyzer::filter_results(&repo_results, &filter)
        } else {
            repo_results
        };

        // Use pure functional core to analyze results
        let (total_wip, repos_with_wip, _status_counters, _wip_branches) =
            WorktreeAnalyzer::analyze(&filtered_results);

        // Display results as table
        let use_emoji = !self.no_emoji;
        let table_output = table::create_table(&filtered_results, use_emoji);
        println!("{}", table_output);

        // Simple summary
        if total_wip > 0 {
            println!();
            println!("Total WIP branches: {}", total_wip);
            println!("Repositories with WIP: {}", repos_with_wip);

            // Show active filters if any
            if self.has_filters() {
                println!("Filters applied: {}", self.describe_filters());
            }
        } else if self.has_filters() {
            println!("No branches match the specified filters.");
        }

        Ok(())
    }

    /// Check if any filters are active
    fn has_filters(&self) -> bool {
        self.prune_candidates
            || self.active
            || self.needs_attention
            || self.stale
            || self.dirty
            || self.clean
            || self.staged
            || self.missing
            || self.ahead
            || self.behind
            || self.diverged
            || self.not_pushed
            || self.not_tracking
            || self.up_to_date
            || self.older_than.is_some()
            || self.newer_than.is_some()
    }

    /// Describe active filters for user feedback
    fn describe_filters(&self) -> String {
        let mut filters = Vec::new();

        // Preset filters
        if self.prune_candidates {
            filters.push("prune-candidates".to_string());
        }
        if self.active {
            filters.push("active".to_string());
        }
        if self.needs_attention {
            filters.push("needs-attention".to_string());
        }
        if self.stale {
            filters.push("stale".to_string());
        }

        // Individual filters
        if self.dirty {
            filters.push("dirty".to_string());
        }
        if self.clean {
            filters.push("clean".to_string());
        }
        if self.staged {
            filters.push("staged".to_string());
        }
        if self.missing {
            filters.push("missing".to_string());
        }
        if self.ahead {
            filters.push("ahead".to_string());
        }
        if self.behind {
            filters.push("behind".to_string());
        }
        if self.diverged {
            filters.push("diverged".to_string());
        }
        if self.not_pushed {
            filters.push("not-pushed".to_string());
        }
        if self.not_tracking {
            filters.push("not-tracking".to_string());
        }
        if self.up_to_date {
            filters.push("up-to-date".to_string());
        }

        if let Some(age) = &self.older_than {
            filters.push(format!("older-than-{}", age));
        }
        if let Some(age) = &self.newer_than {
            filters.push(format!("newer-than-{}", age));
        }

        filters.join(", ")
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
            let commit_summary = repo
                .get_commit_summary(&worktree.path, &worktree.branch)
                .unwrap_or_else(|_| "<no commit>".to_string());

            worktree_results.push(WorktreeResult {
                branch: worktree.branch.clone(),
                status: WorktreeStatus {
                    local_status,
                    remote_status,
                    commit_timestamp,
                    directory_mtime,
                    commit_summary,
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
