use crate::git::{LocalStatus, RemoteStatus};
use crate::github::PrStatus;
use colored::{Color, Colorize};

pub struct ColoredOutput;

impl ColoredOutput {
    pub fn log_header(text: &str) {
        println!("{}", text.color(Color::Blue));
    }

    pub fn log_repo(text: &str) {
        println!("{}", text.color(Color::Green));
    }

    pub fn log_branch(text: &str) {
        println!("  {}", text.color(Color::Cyan));
    }

    pub fn log_path(text: &str) {
        println!("    {}", text.color(Color::BrightBlack));
    }

    pub fn log_status(text: &str) {
        println!("    {}", text.color(Color::BrightBlack));
    }

    pub fn log_summary(text: &str) {
        println!("{}", text.color(Color::Yellow));
    }
}

/// Trait for status formatting - separates presentation from domain logic
pub trait StatusFormatter {
    fn emoji(&self) -> &'static str;
    fn description(&self) -> String;
}

impl StatusFormatter for LocalStatus {
    fn emoji(&self) -> &'static str {
        match self {
            LocalStatus::Clean => "✅",
            LocalStatus::Dirty => "🔧",
            LocalStatus::Staged => "📦",
            LocalStatus::Missing => "❌",
        }
    }

    fn description(&self) -> String {
        match self {
            LocalStatus::Clean => "Clean",
            LocalStatus::Dirty => "Dirty",
            LocalStatus::Staged => "Staged",
            LocalStatus::Missing => "Missing",
        }
        .to_string()
    }
}

impl StatusFormatter for RemoteStatus {
    fn emoji(&self) -> &'static str {
        match self {
            RemoteStatus::UpToDate => "✅",
            RemoteStatus::Ahead(_) => "⬆️",
            RemoteStatus::Behind(_) => "⬇️",
            RemoteStatus::Diverged(_, _) => "🔀",
            RemoteStatus::NotPushed => "❌",
            RemoteStatus::NotTracking => "🔄",
            RemoteStatus::NoRemote => "❌",
        }
    }

    fn description(&self) -> String {
        match self {
            RemoteStatus::UpToDate => "Up to date".to_string(),
            RemoteStatus::Ahead(n) => format!("Ahead {}", n),
            RemoteStatus::Behind(n) => format!("Behind {}", n),
            RemoteStatus::Diverged(ahead, behind) => format!("Diverged (+{}/−{})", ahead, behind),
            RemoteStatus::NotPushed => "Not pushed".to_string(),
            RemoteStatus::NotTracking => "Not tracking".to_string(),
            RemoteStatus::NoRemote => "No remote".to_string(),
        }
    }
}

impl StatusFormatter for PrStatus {
    fn emoji(&self) -> &'static str {
        match self {
            PrStatus::Open(_, _) => "📋",
            PrStatus::Merged(_) => "✅",
            PrStatus::Closed(_) => "❌",
            PrStatus::NoPr => "➖",
            PrStatus::NoGitHub => "➖",
            PrStatus::NoGhCli => "➖",
        }
    }

    fn description(&self) -> String {
        match self {
            PrStatus::Open(num, Some(approval)) => format!("PR Open (#{}) ✓ {}", num, approval),
            PrStatus::Open(num, None) => format!("PR Open (#{}) ⏳", num),
            PrStatus::Merged(num) => format!("PR Merged (#{}) ✅", num),
            PrStatus::Closed(num) => format!("PR Closed (#{}) ❌", num),
            PrStatus::NoPr => "No PR".to_string(),
            PrStatus::NoGitHub => "No GitHub".to_string(),
            PrStatus::NoGhCli => "No gh CLI".to_string(),
        }
    }
}
