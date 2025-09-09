use anyhow::{Result, anyhow};
use git2::{BranchType, Repository, StatusOptions, WorktreePruneOptions};
use std::fmt::Display;
use std::fs;
use std::path::Path;

/// Trait for abstracting Git command operations
pub trait GitClient {
    fn get_config(&self, repo: &Repository, key: &str) -> Result<String>;
    fn list_worktrees(&self, repo: &Repository) -> Result<String>;
    fn get_status_porcelain(&self, repo: &Repository) -> Result<String>;
    fn get_status_branch(&self, repo: &Repository) -> Result<String>;
    fn check_remote_branch(&self, repo: &Repository, remote: &str, branch: &str) -> Result<bool>;
    fn get_last_commit_timestamp(&self, repo: &Repository, branch: &str) -> Result<i64>;
    fn get_directory_mtime(&self, path: &str) -> Result<i64>;
    fn remove_worktree(&self, repo: &Repository, worktree_path: &str) -> Result<()>;
    fn fetch_remotes(&self, repo: &Repository) -> Result<()>;
}

/// Default implementation using system git command
pub struct SystemGitClient;

impl GitClient for SystemGitClient {
    fn get_config(&self, repo: &Repository, key: &str) -> Result<String> {
        let config = repo
            .config()
            .map_err(|e| anyhow!("Failed to open git config: {}", e))?;
        let value = config
            .get_string(key)
            .map_err(|e| anyhow!("Failed to get config value for '{}': {}", key, e))?;
        Ok(value)
    }

    fn list_worktrees(&self, repo: &Repository) -> Result<String> {
        let worktrees = repo
            .worktrees()
            .map_err(|e| anyhow!("Failed to list worktrees: {}", e))?;
        let mut result = String::new();

        for worktree_name in worktrees.iter().flatten() {
            if let Ok(worktree) = repo.find_worktree(worktree_name) {
                let path = worktree.path();
                if path.exists() {
                    let path_str = path.to_string_lossy();

                    // Try to get the current branch for this worktree
                    if let Ok(wt_repo) = Repository::open(path) {
                        if let Ok(head) = wt_repo.head() {
                            if let Some(branch_name) = head.shorthand() {
                                result.push_str(&format!("{} [{}]\n", path_str, branch_name));
                            } else {
                                result.push_str(&format!("{} [detached]\n", path_str));
                            }
                        } else {
                            result.push_str(&format!("{} [unknown]\n", path_str));
                        }
                    } else {
                        result.push_str(&format!("{} [missing]\n", path_str));
                    }
                }
            }
        }

        Ok(result)
    }

    fn get_status_porcelain(&self, repo: &Repository) -> Result<String> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.include_ignored(false);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| anyhow!("Failed to get repository status: {}", e))?;

        let mut result = String::new();
        for entry in statuses.iter() {
            let flags = entry.status();
            let path = entry.path().unwrap_or("<unknown>");

            let mut status_chars = [' ', ' '];

            // Index status (first character)
            if flags.contains(git2::Status::INDEX_NEW) {
                status_chars[0] = 'A';
            } else if flags.contains(git2::Status::INDEX_MODIFIED) {
                status_chars[0] = 'M';
            } else if flags.contains(git2::Status::INDEX_DELETED) {
                status_chars[0] = 'D';
            } else if flags.contains(git2::Status::INDEX_RENAMED) {
                status_chars[0] = 'R';
            } else if flags.contains(git2::Status::INDEX_TYPECHANGE) {
                status_chars[0] = 'T';
            }

            // Working tree status (second character)
            if flags.contains(git2::Status::WT_NEW) {
                status_chars[1] = '?';
            } else if flags.contains(git2::Status::WT_MODIFIED) {
                status_chars[1] = 'M';
            } else if flags.contains(git2::Status::WT_DELETED) {
                status_chars[1] = 'D';
            } else if flags.contains(git2::Status::WT_RENAMED) {
                status_chars[1] = 'R';
            } else if flags.contains(git2::Status::WT_TYPECHANGE) {
                status_chars[1] = 'T';
            }

            if status_chars[0] != ' ' || status_chars[1] != ' ' {
                result.push_str(&format!(
                    "{}{} {}\n",
                    status_chars[0], status_chars[1], path
                ));
            }
        }

        Ok(result)
    }

    fn get_status_branch(&self, repo: &Repository) -> Result<String> {
        let head = repo
            .head()
            .map_err(|e| anyhow!("Failed to get HEAD: {}", e))?;
        let mut result = String::new();

        if let Some(branch_name) = head.shorthand() {
            // Get the upstream branch if it exists
            if let Ok(branch) = repo.find_branch(branch_name, BranchType::Local) {
                if let Ok(upstream) = branch.upstream() {
                    let upstream_name = upstream.name().unwrap_or(None).unwrap_or("unknown");

                    // Calculate ahead/behind counts
                    if let (Some(local_oid), Some(upstream_oid)) =
                        (head.target(), upstream.get().target())
                    {
                        let (ahead, behind) = repo
                            .graph_ahead_behind(local_oid, upstream_oid)
                            .unwrap_or((0, 0));

                        if ahead > 0 && behind > 0 {
                            result.push_str(&format!(
                                "## {}...{} [ahead {}, behind {}]\n",
                                branch_name, upstream_name, ahead, behind
                            ));
                        } else if ahead > 0 {
                            result.push_str(&format!(
                                "## {}...{} [ahead {}]\n",
                                branch_name, upstream_name, ahead
                            ));
                        } else if behind > 0 {
                            result.push_str(&format!(
                                "## {}...{} [behind {}]\n",
                                branch_name, upstream_name, behind
                            ));
                        } else {
                            result.push_str(&format!("## {}...{}\n", branch_name, upstream_name));
                        }
                    } else {
                        result.push_str(&format!("## {}...{}\n", branch_name, upstream_name));
                    }
                } else {
                    // No upstream branch
                    result.push_str(&format!("## {}\n", branch_name));
                }
            } else {
                result.push_str(&format!("## {}\n", branch_name));
            }
        } else {
            result.push_str("## (no branch)\n");
        }

        // Add the file status entries
        let status_output = self.get_status_porcelain(repo)?;
        result.push_str(&status_output);

        Ok(result)
    }

    fn check_remote_branch(&self, repo: &Repository, remote: &str, branch: &str) -> Result<bool> {
        // Try to find the remote reference
        let remote_ref = format!("refs/remotes/{}/{}", remote, branch);
        match repo.find_reference(&remote_ref) {
            Ok(_) => Ok(true),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(e) => Err(anyhow!("Failed to check remote branch: {}", e)),
        }
    }

    fn get_last_commit_timestamp(&self, repo: &Repository, branch: &str) -> Result<i64> {
        let obj = repo
            .revparse_single(branch)
            .map_err(|e| anyhow!("Failed to find branch '{}': {}", branch, e))?;
        let commit = obj
            .as_commit()
            .ok_or_else(|| anyhow!("Object is not a commit"))?;
        Ok(commit.time().seconds())
    }

    fn get_directory_mtime(&self, path: &str) -> Result<i64> {
        let metadata = fs::metadata(path)?;
        let mtime = metadata.modified()?;
        let timestamp = mtime
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| anyhow!("Failed to get timestamp: {}", e))?;
        Ok(timestamp.as_secs() as i64)
    }

    fn remove_worktree(&self, repo: &Repository, worktree_path: &str) -> Result<()> {
        let worktree_name = std::path::Path::new(worktree_path)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("Invalid worktree path: {}", worktree_path))?;

        if let Ok(worktree) = repo.find_worktree(worktree_name) {
            // Configure prune options equivalent to --force
            let mut prune_opts = WorktreePruneOptions::new();
            prune_opts.valid(true); // Prune even if valid (--force equivalent)
            prune_opts.working_tree(true); // Remove the working tree directory

            worktree
                .prune(Some(&mut prune_opts))
                .map_err(|e| anyhow!("Failed to prune worktree: {}", e))?;
        } else {
            // If worktree not found in git metadata but directory exists, just remove it
            if std::path::Path::new(worktree_path).exists() {
                std::fs::remove_dir_all(worktree_path)
                    .map_err(|e| anyhow!("Failed to remove worktree directory: {}", e))?;
            }
        }

        Ok(())
    }

    fn fetch_remotes(&self, repo: &Repository) -> Result<()> {
        let remotes = repo
            .remotes()
            .map_err(|e| anyhow!("Failed to get remotes: {}", e))?;

        // Set up credentials callback for SSH
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
        });

        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        for remote_name in remotes.iter().flatten() {
            if let Ok(mut remote) = repo.find_remote(remote_name) {
                remote
                    .fetch::<&str>(&[], Some(&mut fetch_options), None)
                    .map_err(|e| anyhow!("Failed to fetch from remote '{}': {}", remote_name, e))?;
            }
        }

        Ok(())
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

#[derive(Debug, Clone)]
pub enum MergeStatus {
    #[allow(dead_code)]
    Merged, // Definitely merged (traditional merge detected)
    LikelyMerged, // Probably squash merged (remote branch deleted + old)
    NotMerged,    // Active branch with remote tracking
    Unknown,      // Cannot determine status
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

impl Display for MergeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            MergeStatus::Merged => "Merged",
            MergeStatus::LikelyMerged => "Likely merged",
            MergeStatus::NotMerged => "Not merged",
            MergeStatus::Unknown => "Unknown",
        };
        write!(f, "{}", text)
    }
}

pub struct GitRepository<T: GitClient> {
    git_client: T,
    repository: Repository,
}

impl<T: GitClient> GitRepository<T> {
    pub fn new(path: &str, git_client: T) -> Result<Self> {
        let repository = Repository::open(path)
            .map_err(|e| anyhow!("Failed to open repository at '{}': {}", path, e))?;
        Ok(Self {
            git_client,
            repository,
        })
    }

    pub fn is_bare(&self) -> Result<bool> {
        match self.git_client.get_config(&self.repository, "core.bare") {
            Ok(config_value) => Ok(config_value.trim() == "true"),
            Err(_) => Ok(false),
        }
    }

    pub fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>> {
        let worktrees_output = match self.git_client.list_worktrees(&self.repository) {
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
            if let Some(branch_start) = line.rfind('[')
                && let Some(branch_end) = line.rfind(']')
            {
                let branch = line[branch_start + 1..branch_end].to_string();
                let path = line.split_whitespace().next().unwrap_or("").to_string();

                // Skip main/master branches for WIP detection
                if branch != "main" && branch != "master" {
                    worktrees.push(WorktreeInfo { path, branch });
                }
            }
        }

        worktrees
    }

    pub fn get_local_status(&self, worktree_path: &str) -> Result<LocalStatus> {
        if !Path::new(worktree_path).exists() {
            return Ok(LocalStatus::Missing);
        }

        let worktree_repo = Repository::open(worktree_path)
            .map_err(|_| anyhow!("Failed to open worktree repository"))?;
        let status_output = match self.git_client.get_status_porcelain(&worktree_repo) {
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

        let worktree_repo = Repository::open(worktree_path)
            .map_err(|_| anyhow!("Failed to open worktree repository"))?;
        let status_output = match self.git_client.get_status_branch(&worktree_repo) {
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
                .check_remote_branch(&worktree_repo, "origin", branch_name)
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

    pub fn get_merge_status(
        &self,
        worktree_path: &str,
        branch_name: &str,
        commit_timestamp: i64,
    ) -> Result<MergeStatus> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let days_old = (now - commit_timestamp) / (24 * 60 * 60);

        // Check if remote branch exists (using local remote-tracking refs)
        let worktree_repo = Repository::open(worktree_path)
            .map_err(|_| anyhow!("Failed to open worktree repository"))?;
        let remote_exists = self
            .git_client
            .check_remote_branch(&worktree_repo, "origin", branch_name)
            .unwrap_or(false);

        match (remote_exists, days_old) {
            // Remote branch deleted and branch is old - likely squash merged
            (false, age) if age > 7 => Ok(MergeStatus::LikelyMerged),
            // Remote exists - probably not merged yet
            (true, _) => Ok(MergeStatus::NotMerged),
            // Recent branch with no remote - unknown status
            _ => Ok(MergeStatus::Unknown),
        }
    }

    pub fn get_last_commit_timestamp(&self, worktree_path: &str, branch_name: &str) -> Result<i64> {
        let worktree_repo = Repository::open(worktree_path)
            .map_err(|_| anyhow!("Failed to open worktree repository"))?;
        self.git_client
            .get_last_commit_timestamp(&worktree_repo, branch_name)
    }

    pub fn get_directory_mtime(&self, worktree_path: &str) -> Result<i64> {
        self.git_client.get_directory_mtime(worktree_path)
    }

    pub fn remove_worktree(&self, branch_name: &str) -> Result<()> {
        // First we need to find the worktree path for this branch
        let worktrees = self.list_worktrees()?;
        let worktree = worktrees
            .iter()
            .find(|wt| wt.branch == branch_name)
            .ok_or_else(|| anyhow!("Worktree for branch '{}' not found", branch_name))?;

        self.git_client
            .remove_worktree(&self.repository, &worktree.path)
    }

    pub fn fetch_remotes(&self) -> Result<()> {
        self.git_client.fetch_remotes(&self.repository)
    }
}
