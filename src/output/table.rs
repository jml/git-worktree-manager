use crate::core::{RepoResult, WorktreeResult};
use crate::git::{LocalStatus, RemoteStatus};
use crate::github::PrStatus;
use std::fmt::Display;
use tabled::settings::Style;
use tabled::{Table, Tabled};

#[derive(Debug, Clone)]
pub struct EmojiStatus<T>(pub T);

impl Display for EmojiStatus<LocalStatus> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let emoji = match self.0 {
            LocalStatus::Clean => "✅",
            LocalStatus::Dirty => "🔧",
            LocalStatus::Staged => "📦",
            LocalStatus::Missing => "❌",
        };
        write!(f, "{} {}", emoji, self.0)
    }
}

impl Display for EmojiStatus<RemoteStatus> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let emoji = match self.0 {
            RemoteStatus::UpToDate => "✅",
            RemoteStatus::Ahead(_) => "⬆️",
            RemoteStatus::Behind(_) => "⬇️",
            RemoteStatus::Diverged(_, _) => "🔀",
            RemoteStatus::NotPushed => "❌",
            RemoteStatus::NotTracking => "🔄",
            RemoteStatus::NoRemote => "❌",
        };
        write!(f, "{} {}", emoji, self.0)
    }
}

impl Display for EmojiStatus<PrStatus> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let emoji = match self.0 {
            PrStatus::Open(_, _) => "📋",
            PrStatus::Merged(_) => "✅",
            PrStatus::Closed(_) => "❌",
            PrStatus::NoPr => "➖",
            PrStatus::NoGitHub => "➖",
            PrStatus::NoGhCli => "➖",
        };
        write!(f, "{} {}", emoji, self.0)
    }
}

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
    pub fn from_worktree(repo_name: &str, worktree: &WorktreeResult, use_emoji: bool) -> Self {
        Self {
            repo: repo_name.to_string(),
            branch: worktree.branch.clone(),
            local_status: if use_emoji {
                EmojiStatus(worktree.status.local_status.clone()).to_string()
            } else {
                worktree.status.local_status.to_string()
            },
            remote_status: if use_emoji {
                EmojiStatus(worktree.status.remote_status.clone()).to_string()
            } else {
                worktree.status.remote_status.to_string()
            },
            pr_status: if use_emoji {
                EmojiStatus(worktree.status.pr_status.clone()).to_string()
            } else {
                worktree.status.pr_status.to_string()
            },
        }
    }
}

pub fn create_table(repo_results: &[RepoResult], use_emoji: bool) -> String {
    let mut rows = Vec::new();

    for repo_result in repo_results {
        for worktree in &repo_result.worktrees {
            rows.push(TableRow::from_worktree(
                &repo_result.name,
                worktree,
                use_emoji,
            ));
        }
    }

    if rows.is_empty() {
        return "No work in progress branches found.".to_string();
    }

    Table::new(rows).with(Style::psql()).to_string()
}
