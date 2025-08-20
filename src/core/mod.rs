use crate::git::{LocalStatus, RemoteStatus};

/// Pure functional core for worktree status computation
/// This module contains no I/O operations - only data transformations and business logic

#[derive(Debug, Clone)]
pub struct WorktreeStatus {
    pub local_status: LocalStatus,
    pub remote_status: RemoteStatus,
}

#[derive(Debug, Clone)]
pub struct WorktreeResult {
    pub branch: String,
    pub status: WorktreeStatus,
}

#[derive(Debug, Clone)]
pub struct RepoResult {
    pub name: String,
    pub worktrees: Vec<WorktreeResult>,
}

/// Status counters for generating summaries
#[derive(Debug, Default)]
pub struct StatusCounters {
    // Local status counters
    pub clean: u32,
    pub dirty: u32,
    pub staged: u32,

    // Remote status counters
    pub up_to_date: u32,
    pub ahead: u32,
    pub behind: u32,
    pub diverged: u32,
    pub not_pushed: u32,
    pub not_tracking: u32,
}

impl StatusCounters {
    pub fn new() -> Self {
        Default::default()
    }

    /// Pure function to update counters based on status - no side effects
    pub fn update(&mut self, status: &WorktreeStatus) {
        // Update local status counters
        match status.local_status {
            LocalStatus::Clean => self.clean += 1,
            LocalStatus::Dirty => self.dirty += 1,
            LocalStatus::Staged => self.staged += 1,
            LocalStatus::Missing => {}
        }

        // Update remote status counters
        match status.remote_status {
            RemoteStatus::UpToDate => self.up_to_date += 1,
            RemoteStatus::Ahead(_) => self.ahead += 1,
            RemoteStatus::Behind(_) => self.behind += 1,
            RemoteStatus::Diverged(_, _) => self.diverged += 1,
            RemoteStatus::NotPushed => self.not_pushed += 1,
            RemoteStatus::NotTracking => self.not_tracking += 1,
            RemoteStatus::NoRemote => {}
        }
    }
}

/// Core business logic for analyzing worktree results
pub struct WorktreeAnalyzer;

impl WorktreeAnalyzer {
    /// Analyze results and produce summary statistics - pure function
    pub fn analyze(repo_results: &[RepoResult]) -> (u32, u32, StatusCounters, Vec<String>) {
        let mut total_wip = 0u32;
        let mut repos_with_wip = 0u32;
        let mut wip_branches = Vec::new();
        let mut status_counters = StatusCounters::new();

        for repo_result in repo_results {
            if !repo_result.worktrees.is_empty() {
                repos_with_wip += 1;

                for worktree in &repo_result.worktrees {
                    total_wip += 1;
                    wip_branches.push(format!("{}/{}", repo_result.name, worktree.branch));
                    status_counters.update(&worktree.status);
                }
            }
        }

        (total_wip, repos_with_wip, status_counters, wip_branches)
    }
}
