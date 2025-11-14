use crate::core::{PrStatus, RepoResult, WorktreeResult};
use crate::git::LocalStatus;
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

#[derive(Tabled)]
pub struct TableRow {
    #[tabled(rename = "Repository")]
    pub repo: String,
    #[tabled(rename = "Branch")]
    pub branch: String,
    #[tabled(rename = "Local")]
    pub local_status: String,
    #[tabled(rename = "PR Status")]
    pub pr_status: String,
    #[tabled(rename = "Age")]
    pub commit_age: String,
    #[tabled(rename = "Last Commit")]
    pub commit_summary: String,
}

#[derive(Tabled)]
pub struct TableRowWithoutPr {
    #[tabled(rename = "Repository")]
    pub repo: String,
    #[tabled(rename = "Branch")]
    pub branch: String,
    #[tabled(rename = "Local")]
    pub local_status: String,
    #[tabled(rename = "Age")]
    pub commit_age: String,
    #[tabled(rename = "Last Commit")]
    pub commit_summary: String,
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
            pr_status: format_pr_status(&worktree.status.pr_status),
            commit_age: format_age(worktree.status.commit_timestamp),
            commit_summary: worktree.status.commit_summary.clone(),
        }
    }
}

impl TableRowWithoutPr {
    pub fn from_worktree(repo_name: &str, worktree: &WorktreeResult, use_emoji: bool) -> Self {
        Self {
            repo: repo_name.to_string(),
            branch: worktree.branch.clone(),
            local_status: if use_emoji {
                EmojiStatus(worktree.status.local_status.clone()).to_string()
            } else {
                worktree.status.local_status.to_string()
            },
            commit_age: format_age(worktree.status.commit_timestamp),
            commit_summary: worktree.status.commit_summary.clone(),
        }
    }
}

fn format_pr_status(pr_status: &Option<PrStatus>) -> String {
    match pr_status {
        Some(status) => status.to_string(),
        None => "-".to_string(),
    }
}

fn format_age(timestamp: i64) -> String {
    if timestamp == 0 {
        return "Unknown".to_string();
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let days_ago = (now - timestamp) / (24 * 60 * 60);

    match days_ago {
        0 => "Today".to_string(),
        1 => "1 day".to_string(),
        n if n < 7 => format!("{} days", n),
        n if n < 30 => format!("{} weeks", n / 7),
        n if n < 365 => format!("{} months", n / 30),
        n => format!("{} years", n / 365),
    }
}

pub fn create_table(repo_results: &[RepoResult], use_emoji: bool, show_pr_status: bool) -> String {
    if show_pr_status {
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
    } else {
        let mut rows = Vec::new();

        for repo_result in repo_results {
            for worktree in &repo_result.worktrees {
                rows.push(TableRowWithoutPr::from_worktree(
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
}
