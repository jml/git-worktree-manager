use crate::core::{RepoResult, WorktreeResult};
use crate::git::{LocalStatus, RemoteStatus};
use crate::github::PrStatus;
use tabled::settings::Style;
use tabled::{Table, Tabled};

#[derive(Tabled)]
pub struct TableRow {
    #[tabled(rename = "Repository")]
    pub repo: String,
    #[tabled(rename = "Branch")]
    pub branch: String,
    #[tabled(rename = "Local")]
    pub local_status: String,
    #[tabled(rename = "Remote")]
    pub remote_status: String,
    #[tabled(rename = "PR")]
    pub pr_status: String,
}

impl TableRow {
    pub fn from_worktree(repo_name: &str, worktree: &WorktreeResult) -> Self {
        Self {
            repo: repo_name.to_string(),
            branch: worktree.branch.clone(),
            local_status: format_local_status(&worktree.status.local_status),
            remote_status: format_remote_status(&worktree.status.remote_status),
            pr_status: format_pr_status(&worktree.status.pr_status),
        }
    }
}

pub fn create_table(repo_results: &[RepoResult]) -> String {
    let mut rows = Vec::new();

    for repo_result in repo_results {
        for worktree in &repo_result.worktrees {
            rows.push(TableRow::from_worktree(&repo_result.name, worktree));
        }
    }

    if rows.is_empty() {
        return "No work in progress branches found.".to_string();
    }

    Table::new(rows).with(Style::psql()).to_string()
}

fn format_local_status(status: &LocalStatus) -> String {
    match status {
        LocalStatus::Clean => "Clean".to_string(),
        LocalStatus::Dirty => "Dirty".to_string(),
        LocalStatus::Staged => "Staged".to_string(),
        LocalStatus::Missing => "Missing".to_string(),
    }
}

fn format_remote_status(status: &RemoteStatus) -> String {
    match status {
        RemoteStatus::UpToDate => "Up to date".to_string(),
        RemoteStatus::Ahead(n) => format!("Ahead {}", n),
        RemoteStatus::Behind(n) => format!("Behind {}", n),
        RemoteStatus::Diverged(ahead, behind) => format!("Diverged +{} -{}", ahead, behind),
        RemoteStatus::NotPushed => "Not pushed".to_string(),
        RemoteStatus::NotTracking => "Not tracking".to_string(),
        RemoteStatus::NoRemote => "No remote".to_string(),
    }
}

fn format_pr_status(status: &PrStatus) -> String {
    match status {
        PrStatus::Open(num, Some(approval)) => format!("Open #{} ({})", num, approval),
        PrStatus::Open(num, None) => format!("Open #{}", num),
        PrStatus::Merged(num) => format!("Merged #{}", num),
        PrStatus::Closed(num) => format!("Closed #{}", num),
        PrStatus::NoPr => "No PR".to_string(),
        PrStatus::NoGitHub => "No GitHub".to_string(),
        PrStatus::NoGhCli => "No gh CLI".to_string(),
    }
}
