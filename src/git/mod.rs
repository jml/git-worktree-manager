use anyhow::{Result, anyhow};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: String,
}

#[derive(Debug, Clone)]
pub enum LocalStatus {
    Clean,
    Dirty,
    Staged,
    Missing,
}

impl LocalStatus {
    pub fn emoji(&self) -> &'static str {
        match self {
            LocalStatus::Clean => "✅",
            LocalStatus::Dirty => "🔧",
            LocalStatus::Staged => "📦",
            LocalStatus::Missing => "❌",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            LocalStatus::Clean => "Clean",
            LocalStatus::Dirty => "Dirty",
            LocalStatus::Staged => "Staged",
            LocalStatus::Missing => "Missing",
        }
    }
}

#[derive(Debug, Clone)]
pub enum RemoteStatus {
    UpToDate,
    Ahead(u32),
    Behind(u32),
    Diverged(u32, u32), // ahead, behind
    NotPushed,
    NotTracking,
    NoRemote,
}

impl RemoteStatus {
    pub fn emoji(&self) -> &'static str {
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

    pub fn description(&self) -> String {
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

pub struct GitRepository {
    pub path: String,
}

impl GitRepository {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }

    pub fn is_bare(&self) -> Result<bool> {
        let output = Command::new("git")
            .args(["-C", &self.path, "config", "--get", "core.bare"])
            .output()?;

        if output.status.success() {
            let bare_str = String::from_utf8_lossy(&output.stdout);
            Ok(bare_str.trim() == "true")
        } else {
            Ok(false)
        }
    }

    pub fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>> {
        let output = Command::new("git")
            .args(["-C", &self.path, "worktree", "list"])
            .output()?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let worktrees_str = String::from_utf8_lossy(&output.stdout);
        let mut worktrees = Vec::new();

        for line in worktrees_str.lines() {
            // Skip bare repository lines
            if line.contains("(bare)") {
                continue;
            }

            // Parse format: /path/to/worktree [commit] [branch]
            if let Some(branch_start) = line.rfind('[') {
                if let Some(branch_end) = line.rfind(']') {
                    let branch = line[branch_start + 1..branch_end].to_string();
                    let path = line.split_whitespace().next().unwrap_or("").to_string();

                    // Skip main/master branches for WIP detection
                    if branch != "main" && branch != "master" {
                        worktrees.push(WorktreeInfo { path, branch });
                    }
                }
            }
        }

        Ok(worktrees)
    }

    pub fn get_local_status(&self, worktree_path: &str) -> Result<LocalStatus> {
        if !Path::new(worktree_path).exists() {
            return Ok(LocalStatus::Missing);
        }

        let output = Command::new("git")
            .args(["-C", worktree_path, "status", "--porcelain"])
            .output()?;

        if !output.status.success() {
            return Ok(LocalStatus::Missing);
        }

        let status_output = String::from_utf8_lossy(&output.stdout);

        if status_output.trim().is_empty() {
            Ok(LocalStatus::Clean)
        } else if status_output.lines().any(|line| {
            line.starts_with('A')
                || line.starts_with('D')
                || line.starts_with('R')
                || line.starts_with('M')
        }) {
            Ok(LocalStatus::Staged)
        } else {
            Ok(LocalStatus::Dirty)
        }
    }

    pub fn get_remote_status(
        &self,
        worktree_path: &str,
        branch_name: &str,
    ) -> Result<RemoteStatus> {
        if !Path::new(worktree_path).exists() {
            return Ok(RemoteStatus::NoRemote);
        }

        // Get upstream tracking info and ahead/behind counts in one call
        let status_output = Command::new("git")
            .args(["-C", worktree_path, "status", "--porcelain=v1", "--branch"])
            .output()?;

        if !status_output.status.success() {
            return Ok(RemoteStatus::NoRemote);
        }

        let status_str = String::from_utf8_lossy(&status_output.stdout);
        let first_line = status_str.lines().next().unwrap_or("");

        // Parse the branch line: ## branch_name...origin/branch_name [ahead N, behind M]
        if !first_line.starts_with("## ") {
            return Ok(RemoteStatus::NoRemote);
        }

        let branch_info = &first_line[3..]; // Remove "## "

        if !branch_info.contains("...") {
            // No upstream tracking
            // Quick check if branch exists on remote using git ls-remote
            let remote_check = Command::new("git")
                .args([
                    "-C",
                    worktree_path,
                    "ls-remote",
                    "--exit-code",
                    "--heads",
                    "origin",
                    branch_name,
                ])
                .output()?;

            if remote_check.status.success() {
                return Ok(RemoteStatus::NotTracking);
            } else {
                return Ok(RemoteStatus::NotPushed);
            }
        }

        // Parse ahead/behind from status output
        if let Some(bracket_start) = branch_info.find('[') {
            let bracket_content = &branch_info[bracket_start + 1..];
            if let Some(bracket_end) = bracket_content.find(']') {
                let tracking_info = &bracket_content[..bracket_end];

                let mut ahead = 0u32;
                let mut behind = 0u32;

                for part in tracking_info.split(", ") {
                    if let Some(ahead_str) = part.strip_prefix("ahead ") {
                        ahead = ahead_str.parse().unwrap_or(0);
                    } else if let Some(behind_str) = part.strip_prefix("behind ") {
                        behind = behind_str.parse().unwrap_or(0);
                    }
                }

                match (ahead, behind) {
                    (0, 0) => Ok(RemoteStatus::UpToDate),
                    (a, 0) if a > 0 => Ok(RemoteStatus::Ahead(a)),
                    (0, b) if b > 0 => Ok(RemoteStatus::Behind(b)),
                    (a, b) if a > 0 && b > 0 => Ok(RemoteStatus::Diverged(a, b)),
                    _ => Ok(RemoteStatus::UpToDate),
                }
            } else {
                Ok(RemoteStatus::UpToDate)
            }
        } else {
            Ok(RemoteStatus::UpToDate)
        }
    }

    pub fn get_remote_url(&self, worktree_path: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["-C", worktree_path, "remote", "get-url", "origin"])
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow!("Failed to get remote URL"))
        }
    }
}
