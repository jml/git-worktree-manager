use crate::git::{LocalStatus, MergeStatus, RemoteStatus};
use std::path::PathBuf;

/// Pure functional core for worktree status computation
/// This module contains no I/O operations - only data transformations and business logic

#[derive(Debug, Clone)]
pub struct WorktreeStatus {
    pub local_status: LocalStatus,
    pub remote_status: RemoteStatus,
    pub commit_timestamp: i64,
    #[allow(dead_code)]
    pub directory_mtime: i64,
    pub merge_status: MergeStatus,
}

#[derive(Debug, Clone)]
pub struct WorktreeResult {
    pub branch: String,
    pub status: WorktreeStatus,
}

#[derive(Debug, Clone)]
pub struct RepoResult {
    pub name: String,
    pub path: PathBuf,
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

/// Filtering criteria for worktrees
#[derive(Debug, Default)]
pub struct WorktreeFilter {
    // Local status filters
    pub dirty: Option<bool>,
    pub clean: Option<bool>,
    pub staged: Option<bool>,
    pub missing: Option<bool>,

    // Remote status filters
    pub ahead: Option<bool>,
    pub behind: Option<bool>,
    pub diverged: Option<bool>,
    pub not_pushed: Option<bool>,
    pub not_tracking: Option<bool>,
    pub up_to_date: Option<bool>,

    // Merge status filters
    pub likely_merged: Option<bool>,
    pub not_merged: Option<bool>,
    pub unknown_merge: Option<bool>,

    // Age filters
    pub older_than_days: Option<u32>,
    pub newer_than_days: Option<u32>,

    // Preset indicators
    pub is_needs_attention: bool,
}

impl WorktreeFilter {
    pub fn new() -> Self {
        Default::default()
    }

    /// Parse age string like "30", "30d", "1w", "2m" into days
    pub fn parse_age_to_days(age_str: &str) -> Result<u32, String> {
        if age_str.is_empty() {
            return Err("Empty age string".to_string());
        }

        // Handle pure numbers (assume days)
        if let Ok(days) = age_str.parse::<u32>() {
            return Ok(days);
        }

        // Handle suffixed formats
        let (number_part, suffix) = if age_str.ends_with("days") || age_str.ends_with("day") {
            let num_str = age_str.trim_end_matches("days").trim_end_matches("day");
            (num_str, "d")
        } else if age_str.ends_with("weeks") || age_str.ends_with("week") {
            let num_str = age_str.trim_end_matches("weeks").trim_end_matches("week");
            (num_str, "w")
        } else if age_str.ends_with("months") || age_str.ends_with("month") {
            let num_str = age_str.trim_end_matches("months").trim_end_matches("month");
            (num_str, "m")
        } else if age_str.len() > 1 {
            let (num_str, suffix_str) = age_str.split_at(age_str.len() - 1);
            (num_str, suffix_str)
        } else {
            return Err(format!("Invalid age format: {}", age_str));
        };

        let number: u32 = number_part
            .parse()
            .map_err(|_| format!("Invalid number in age: {}", age_str))?;

        match suffix {
            "d" => Ok(number),
            "w" => Ok(number * 7),
            "m" => Ok(number * 30), // Approximate month as 30 days
            _ => Err(format!("Invalid age suffix: {}", suffix)),
        }
    }

    /// Create preset filter for pruning candidates
    pub fn prune_candidates() -> Self {
        Self {
            likely_merged: Some(true),
            clean: Some(true),
            older_than_days: Some(7),
            ..Default::default()
        }
    }

    /// Create preset filter for active work
    pub fn active() -> Self {
        Self {
            not_merged: Some(true),
            newer_than_days: Some(7),
            ..Default::default()
        }
    }

    /// Create preset filter for branches needing attention
    pub fn needs_attention() -> Self {
        Self {
            is_needs_attention: true,
            ..Default::default()
        }
    }

    /// Create preset for stale branches (older than 30 days)
    pub fn stale() -> Self {
        Self {
            older_than_days: Some(30),
            ..Default::default()
        }
    }

    /// Pure function to check if a worktree matches the filter criteria
    pub fn matches(&self, worktree: &WorktreeResult, current_timestamp: i64) -> bool {
        // Handle special preset logic
        if self.is_needs_attention {
            return self.matches_needs_attention(worktree);
        }

        // Check local status filters
        if !self.matches_local_status(&worktree.status.local_status) {
            return false;
        }

        // Check remote status filters
        if !self.matches_remote_status(&worktree.status.remote_status) {
            return false;
        }

        // Check merge status filters
        if !self.matches_merge_status(&worktree.status.merge_status) {
            return false;
        }

        // Check age filters
        if !self.matches_age(worktree.status.commit_timestamp, current_timestamp) {
            return false;
        }

        true
    }

    fn matches_needs_attention(&self, worktree: &WorktreeResult) -> bool {
        matches!(
            worktree.status.remote_status,
            RemoteStatus::Diverged(_, _) | RemoteStatus::Behind(_)
        ) || matches!(worktree.status.local_status, LocalStatus::Missing)
    }

    fn matches_local_status(&self, status: &LocalStatus) -> bool {
        // If no local status filters are specified, pass everything
        if self.dirty.is_none()
            && self.clean.is_none()
            && self.staged.is_none()
            && self.missing.is_none()
        {
            return true;
        }

        // Check if this status matches any of the requested filters
        match status {
            LocalStatus::Dirty => self.dirty.unwrap_or(false),
            LocalStatus::Clean => self.clean.unwrap_or(false),
            LocalStatus::Staged => self.staged.unwrap_or(false),
            LocalStatus::Missing => self.missing.unwrap_or(false),
        }
    }

    fn matches_remote_status(&self, status: &RemoteStatus) -> bool {
        // If no remote status filters are specified, pass everything
        if self.ahead.is_none()
            && self.behind.is_none()
            && self.diverged.is_none()
            && self.not_pushed.is_none()
            && self.not_tracking.is_none()
            && self.up_to_date.is_none()
        {
            return true;
        }

        // Check if this status matches any of the requested filters
        match status {
            RemoteStatus::Ahead(_) => self.ahead.unwrap_or(false),
            RemoteStatus::Behind(_) => self.behind.unwrap_or(false),
            RemoteStatus::Diverged(_, _) => self.diverged.unwrap_or(false),
            RemoteStatus::NotPushed => self.not_pushed.unwrap_or(false),
            RemoteStatus::NotTracking => self.not_tracking.unwrap_or(false),
            RemoteStatus::UpToDate => self.up_to_date.unwrap_or(false),
            RemoteStatus::NoRemote => true, // Always include no remote (assume it's not filtered)
        }
    }

    fn matches_merge_status(&self, status: &MergeStatus) -> bool {
        // If no merge status filters are specified, pass everything
        if self.likely_merged.is_none() && self.not_merged.is_none() && self.unknown_merge.is_none()
        {
            return true;
        }

        // Check if this status matches any of the requested filters
        match status {
            MergeStatus::LikelyMerged => self.likely_merged.unwrap_or(false),
            MergeStatus::NotMerged => self.not_merged.unwrap_or(false),
            MergeStatus::Unknown => self.unknown_merge.unwrap_or(false),
            MergeStatus::Merged => true, // Always include explicitly merged
        }
    }

    fn matches_age(&self, commit_timestamp: i64, current_timestamp: i64) -> bool {
        if commit_timestamp == 0 {
            return true; // Unknown age always passes
        }

        let days_old = (current_timestamp - commit_timestamp) / (24 * 60 * 60);

        if let Some(older_than) = self.older_than_days
            && days_old < older_than as i64
        {
            return false;
        }

        if let Some(newer_than) = self.newer_than_days
            && days_old > newer_than as i64
        {
            return false;
        }

        true
    }
}

/// Analyzer extension for filtering
impl WorktreeAnalyzer {
    /// Filter repository results based on criteria
    pub fn filter_results(repo_results: &[RepoResult], filter: &WorktreeFilter) -> Vec<RepoResult> {
        let current_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut filtered_results = Vec::new();

        for repo_result in repo_results {
            let filtered_worktrees: Vec<WorktreeResult> = repo_result
                .worktrees
                .iter()
                .filter(|worktree| filter.matches(worktree, current_timestamp))
                .cloned()
                .collect();

            if !filtered_worktrees.is_empty() {
                filtered_results.push(RepoResult {
                    name: repo_result.name.clone(),
                    path: repo_result.path.clone(),
                    worktrees: filtered_worktrees,
                });
            }
        }

        filtered_results
    }
}
