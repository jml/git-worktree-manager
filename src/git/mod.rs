use anyhow::{Result, anyhow};
use git2::build::CheckoutBuilder;
use git2::{BranchType, Repository, StatusOptions, WorktreeAddOptions, WorktreePruneOptions};
use std::fmt::Display;
use std::fs;
use std::path::Path;

/// Trait for abstracting Git command operations
pub trait GitClient {
    fn get_config(&self, repo: &Repository, key: &str) -> Result<String>;
    fn list_worktrees(&self, repo: &Repository) -> Result<String>;
    fn get_status_porcelain(&self, repo: &Repository) -> Result<String>;
    fn get_last_commit_timestamp(&self, repo: &Repository, branch: &str) -> Result<i64>;
    fn get_commit_summary(&self, repo: &Repository, branch: &str) -> Result<String>;
    fn get_directory_mtime(&self, path: &str) -> Result<i64>;
    fn remove_worktree(&self, repo: &Repository, worktree_path: &str) -> Result<()>;
    fn add_worktree(
        &self,
        repo: &Repository,
        branch: &str,
        path: &str,
        base_branch: Option<&str>,
        reuse_existing_branch: bool,
    ) -> Result<()>;
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

    fn get_last_commit_timestamp(&self, repo: &Repository, branch: &str) -> Result<i64> {
        let obj = repo
            .revparse_single(branch)
            .map_err(|e| anyhow!("Failed to find branch '{}': {}", branch, e))?;
        let commit = obj
            .as_commit()
            .ok_or_else(|| anyhow!("Object is not a commit"))?;
        Ok(commit.time().seconds())
    }

    fn get_commit_summary(&self, repo: &Repository, branch: &str) -> Result<String> {
        let obj = repo
            .revparse_single(branch)
            .map_err(|e| anyhow!("Failed to find branch '{}': {}", branch, e))?;
        let commit = obj
            .as_commit()
            .ok_or_else(|| anyhow!("Object is not a commit"))?;

        // Get the commit summary (first line of the message)
        let message = commit.summary().unwrap_or("<no message>").to_string();

        Ok(message)
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

    fn add_worktree(
        &self,
        repo: &Repository,
        branch: &str,
        path: &str,
        base_branch: Option<&str>,
        reuse_existing_branch: bool,
    ) -> Result<()> {
        // Check if the target path already exists
        if std::path::Path::new(path).exists() {
            return Err(anyhow!("Target path '{}' already exists", path));
        }

        // Determine the source branch
        let source_branch = base_branch.unwrap_or("main");

        // Check if the branch already exists locally
        let branch_exists = repo.find_branch(branch, BranchType::Local).is_ok();

        if branch_exists {
            // If branch exists but reuse is not enabled, fail with helpful message
            if !reuse_existing_branch {
                return Err(anyhow!(
                    "Branch '{}' already exists. Use --reuse to reuse the existing branch, or choose a different branch name.",
                    branch
                ));
            }
            // If branch exists, create worktree and check it out to existing branch
            let worktree_name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(branch);

            // Find the existing branch reference to use it for the worktree
            let branch_ref = repo
                .find_branch(branch, BranchType::Local)
                .map_err(|e| anyhow!("Failed to find existing branch '{}': {}", branch, e))?;

            // Create worktree using the existing branch's commit
            let mut worktree_opts = WorktreeAddOptions::new();
            worktree_opts.reference(Some(branch_ref.get()));

            repo.worktree(worktree_name, Path::new(path), Some(&worktree_opts))
                .map_err(|e| anyhow!("Failed to create worktree: {}", e))?;

            // Open the worktree repository and verify checkout
            let worktree_repo = Repository::open(path)
                .map_err(|e| anyhow!("Failed to open worktree repository: {}", e))?;

            // Checkout the branch (worktree should already be on correct branch)
            worktree_repo
                .checkout_head(Some(CheckoutBuilder::new().force()))
                .map_err(|e| anyhow!("Failed to checkout existing branch: {}", e))?;
        } else {
            // Check if source branch exists before creating worktree
            if repo.find_branch(source_branch, BranchType::Local).is_err()
                && repo
                    .find_branch(&format!("origin/{}", source_branch), BranchType::Remote)
                    .is_err()
            {
                return Err(anyhow!(
                    "Source branch '{}' not found locally or on remote",
                    source_branch
                ));
            }

            // Create worktree first (this creates it at the default branch/commit)
            repo.worktree(branch, Path::new(path), Some(&WorktreeAddOptions::new()))
                .map_err(|e| anyhow!("Failed to create worktree: {}", e))?;

            // From this point on, if we fail, we should clean up the worktree
            let cleanup_worktree = || {
                if let Ok(worktree) = repo.find_worktree(branch) {
                    let mut prune_opts = WorktreePruneOptions::new();
                    prune_opts.valid(true);
                    prune_opts.working_tree(true);
                    let _ = worktree.prune(Some(&mut prune_opts));
                }
                if std::path::Path::new(path).exists() {
                    let _ = std::fs::remove_dir_all(path);
                }
            };

            // Open the worktree repository and create/checkout the new branch
            let worktree_repo = match Repository::open(path) {
                Ok(repo) => repo,
                Err(e) => {
                    cleanup_worktree();
                    return Err(anyhow!("Failed to open worktree repository: {}", e));
                }
            };

            // Now resolve the source commit in the worktree repository context
            let source_branch_ref = if worktree_repo
                .find_branch(source_branch, BranchType::Local)
                .is_ok()
            {
                source_branch.to_string()
            } else if worktree_repo
                .find_branch(&format!("origin/{}", source_branch), BranchType::Remote)
                .is_ok()
            {
                format!("origin/{}", source_branch)
            } else {
                cleanup_worktree();
                return Err(anyhow!(
                    "Source branch '{}' not found in worktree repository",
                    source_branch
                ));
            };

            // Resolve the commit in the worktree repository
            let source_obj = match worktree_repo.revparse_single(&source_branch_ref) {
                Ok(obj) => obj,
                Err(e) => {
                    cleanup_worktree();
                    return Err(anyhow!(
                        "Failed to resolve source branch '{}': {}",
                        source_branch_ref,
                        e
                    ));
                }
            };
            let source_commit = match source_obj.as_commit() {
                Some(commit) => commit,
                None => {
                    cleanup_worktree();
                    return Err(anyhow!("Source reference is not a commit"));
                }
            };

            // Create new branch pointing to source commit
            if let Err(e) = worktree_repo.branch(branch, source_commit, false) {
                cleanup_worktree();
                return Err(anyhow!("Failed to create branch '{}': {}", branch, e));
            }

            // Checkout the new branch
            let branch_ref = match worktree_repo.find_branch(branch, BranchType::Local) {
                Ok(branch_ref) => branch_ref,
                Err(e) => {
                    cleanup_worktree();
                    return Err(anyhow!("Failed to find created branch: {}", e));
                }
            };

            if let Err(e) = worktree_repo.checkout_tree(
                source_commit.as_object(),
                Some(CheckoutBuilder::new().force()),
            ) {
                cleanup_worktree();
                return Err(anyhow!("Failed to checkout tree: {}", e));
            }

            if let Err(e) = worktree_repo.set_head(
                branch_ref
                    .get()
                    .name()
                    .unwrap_or(&format!("refs/heads/{}", branch)),
            ) {
                cleanup_worktree();
                return Err(anyhow!("Failed to set HEAD: {}", e));
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

    pub fn get_last_commit_timestamp(&self, worktree_path: &str, branch_name: &str) -> Result<i64> {
        let worktree_repo = Repository::open(worktree_path)
            .map_err(|_| anyhow!("Failed to open worktree repository"))?;
        self.git_client
            .get_last_commit_timestamp(&worktree_repo, branch_name)
    }

    pub fn get_commit_summary(&self, worktree_path: &str, branch_name: &str) -> Result<String> {
        let worktree_repo = Repository::open(worktree_path)
            .map_err(|_| anyhow!("Failed to open worktree repository"))?;
        self.git_client
            .get_commit_summary(&worktree_repo, branch_name)
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

    pub fn add_worktree(
        &self,
        branch: &str,
        path: &str,
        base_branch: Option<&str>,
        reuse_existing_branch: bool,
    ) -> Result<()> {
        self.git_client.add_worktree(
            &self.repository,
            branch,
            path,
            base_branch,
            reuse_existing_branch,
        )
    }

    pub fn fetch_remotes(&self) -> Result<()> {
        self.git_client.fetch_remotes(&self.repository)
    }
}
