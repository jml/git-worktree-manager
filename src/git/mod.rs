use anyhow::{Result, anyhow};
use std::fmt::Display;
use std::path::Path;
use std::process::Command;

/// Trait for abstracting Git command operations
pub trait GitClient {
    fn get_config(&self, path: &str, key: &str) -> Result<String>;
    fn list_worktrees(&self, path: &str) -> Result<String>;
    fn get_status_porcelain(&self, path: &str) -> Result<String>;
    fn get_status_branch(&self, path: &str) -> Result<String>;
    fn check_remote_branch(&self, path: &str, remote: &str, branch: &str) -> Result<bool>;
}

/// Default implementation using system git command
pub struct SystemGitClient;

impl GitClient for SystemGitClient {
    fn get_config(&self, path: &str, key: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["-C", path, "config", "--get", key])
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow!("Git config command failed"))
        }
    }

    fn list_worktrees(&self, path: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["-C", path, "worktree", "list"])
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!("Git worktree list failed"))
        }
    }

    fn get_status_porcelain(&self, path: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["-C", path, "status", "--porcelain"])
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!("Git status failed"))
        }
    }

    fn get_status_branch(&self, path: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["-C", path, "status", "--porcelain=v1", "--branch"])
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!("Git status branch failed"))
        }
    }

    fn check_remote_branch(&self, path: &str, remote: &str, branch: &str) -> Result<bool> {
        let output = Command::new("git")
            .args([
                "-C",
                path,
                "ls-remote",
                "--exit-code",
                "--heads",
                remote,
                branch,
            ])
            .output()?;

        Ok(output.status.success())
    }
}

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

impl Display for LocalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            LocalStatus::Clean => "Clean",
            LocalStatus::Dirty => "Dirty",
            LocalStatus::Staged => "Staged",
            LocalStatus::Missing => "Missing",
        };
        write!(f, "{}", text)
    }
}

impl Display for RemoteStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            RemoteStatus::UpToDate => "Up to date".to_string(),
            RemoteStatus::Ahead(n) => format!("Ahead {}", n),
            RemoteStatus::Behind(n) => format!("Behind {}", n),
            RemoteStatus::Diverged(ahead, behind) => format!("Diverged +{} -{}", ahead, behind),
            RemoteStatus::NotPushed => "Not pushed".to_string(),
            RemoteStatus::NotTracking => "Not tracking".to_string(),
            RemoteStatus::NoRemote => "No remote".to_string(),
        };
        write!(f, "{}", text)
    }
}

pub struct GitRepository<T: GitClient> {
    pub path: String,
    git_client: T,
}

impl<T: GitClient> GitRepository<T> {
    pub fn new(path: &str, git_client: T) -> Self {
        Self {
            path: path.to_string(),
            git_client,
        }
    }

    pub fn is_bare(&self) -> Result<bool> {
        match self.git_client.get_config(&self.path, "core.bare") {
            Ok(config_value) => Ok(config_value.trim() == "true"),
            Err(_) => Ok(false),
        }
    }

    pub fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>> {
        let worktrees_output = match self.git_client.list_worktrees(&self.path) {
            Ok(output) => output,
            Err(_) => return Ok(vec![]),
        };

        Ok(Self::parse_worktrees(&worktrees_output))
    }

    /// Pure function to parse worktree output
    fn parse_worktrees(worktrees_str: &str) -> Vec<WorktreeInfo> {
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

        worktrees
    }

    pub fn get_local_status(&self, worktree_path: &str) -> Result<LocalStatus> {
        if !Path::new(worktree_path).exists() {
            return Ok(LocalStatus::Missing);
        }

        let status_output = match self.git_client.get_status_porcelain(worktree_path) {
            Ok(output) => output,
            Err(_) => return Ok(LocalStatus::Missing),
        };

        Ok(Self::parse_local_status(&status_output))
    }

    /// Pure function to parse local status from git status --porcelain output
    fn parse_local_status(status_output: &str) -> LocalStatus {
        if status_output.trim().is_empty() {
            LocalStatus::Clean
        } else if status_output.lines().any(|line| {
            line.starts_with('A')
                || line.starts_with('D')
                || line.starts_with('R')
                || line.starts_with('M')
        }) {
            LocalStatus::Staged
        } else {
            LocalStatus::Dirty
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

        let status_output = match self.git_client.get_status_branch(worktree_path) {
            Ok(output) => output,
            Err(_) => return Ok(RemoteStatus::NoRemote),
        };

        let first_line = status_output.lines().next().unwrap_or("");

        if !first_line.starts_with("## ") {
            return Ok(RemoteStatus::NoRemote);
        }

        let branch_info = &first_line[3..]; // Remove "## "

        if !branch_info.contains("...") {
            // No upstream tracking - check if branch exists on remote
            match self
                .git_client
                .check_remote_branch(worktree_path, "origin", branch_name)
            {
                Ok(true) => Ok(RemoteStatus::NotTracking),
                Ok(false) | Err(_) => Ok(RemoteStatus::NotPushed),
            }
        } else {
            Ok(Self::parse_remote_status(branch_info))
        }
    }

    /// Pure function to parse remote status from git status branch line
    fn parse_remote_status(branch_info: &str) -> RemoteStatus {
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
                    (0, 0) => RemoteStatus::UpToDate,
                    (a, 0) if a > 0 => RemoteStatus::Ahead(a),
                    (0, b) if b > 0 => RemoteStatus::Behind(b),
                    (a, b) if a > 0 && b > 0 => RemoteStatus::Diverged(a, b),
                    _ => RemoteStatus::UpToDate,
                }
            } else {
                RemoteStatus::UpToDate
            }
        } else {
            RemoteStatus::UpToDate
        }
    }
}
