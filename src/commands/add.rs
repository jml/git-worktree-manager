use anyhow::Result;
use clap::Args;
use futures::future::try_join_all;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::RepoResult;
use crate::git::{GitRepository, SystemGitClient};

#[derive(Args)]
pub struct AddCommand {
    /// Repository name
    repo: String,

    /// Branch name to create
    branch: String,

    /// Base branch to create from (defaults to main)
    #[arg(short, long)]
    base_branch: Option<String>,

    /// Directory to search for repositories (defaults to current directory)
    /// Can also be set via GWM_REPOS_PATH environment variable
    #[arg(short, long, env = "GWM_REPOS_PATH")]
    path: Option<String>,

    /// Show what would be created without actually creating anything
    #[arg(long)]
    dry_run: bool,

    /// Reuse existing branch instead of failing when branch already exists
    #[arg(long)]
    reuse: bool,
}

impl AddCommand {
    pub async fn execute(&self) -> Result<()> {
        let search_path = self.path.as_deref().unwrap_or(".");

        // Find all repositories
        let repo_tasks = self.collect_repositories(search_path).await?;
        let repo_task_results = try_join_all(repo_tasks).await?;

        let mut repo_results = Vec::new();
        for task_result in repo_task_results {
            repo_results.push(task_result?);
        }

        // Find the target repository
        let target_repo = self.find_target_repository(&repo_results)?;

        if target_repo.is_none() {
            println!("No repository found with name '{}'", self.repo);
            return Ok(());
        }

        let repo_result = target_repo.unwrap();

        // Check if branch already exists in this repo
        if self.branch_exists_in_repo(repo_result)? {
            println!(
                "Branch '{}' already exists as a worktree in repository '{}'",
                self.branch, self.repo
            );
            return Ok(());
        }

        // Determine worktree path (sibling directory to repo)
        let worktree_path = self.determine_worktree_path(&repo_result.path)?;

        if worktree_path.exists() {
            println!(
                "Target directory '{}' already exists",
                worktree_path.display()
            );
            return Ok(());
        }

        println!("Target worktree:");
        println!("  Repository: {}", repo_result.name);
        println!("  Branch: {}", self.branch);
        println!(
            "  Base branch: {}",
            self.base_branch.as_deref().unwrap_or("main")
        );
        println!("  Path: {}", worktree_path.display());
        println!();

        if self.dry_run {
            println!(
                "🔍 DRY RUN: Would create worktree {}/{}",
                self.repo, self.branch
            );
            return Ok(());
        }

        // Perform the creation
        let repo = GitRepository::new(repo_result.path.to_str().unwrap(), SystemGitClient)?;
        println!("🌟 Creating worktree {}/{}", repo_result.name, self.branch);

        repo.add_worktree(
            &self.branch,
            worktree_path.to_str().unwrap(),
            self.base_branch.as_deref(),
            self.reuse,
        )?;

        println!(
            "✅ Successfully created worktree {}/{}",
            self.repo, self.branch
        );

        Ok(())
    }

    /// Find the target repository by name
    fn find_target_repository<'a>(
        &self,
        repo_results: &'a [RepoResult],
    ) -> Result<Option<&'a RepoResult>> {
        for repo_result in repo_results {
            if repo_result.name == self.repo {
                return Ok(Some(repo_result));
            }
        }
        Ok(None)
    }

    /// Check if branch already exists as a worktree in this repo
    fn branch_exists_in_repo(&self, repo_result: &RepoResult) -> Result<bool> {
        for worktree in &repo_result.worktrees {
            if worktree.branch == self.branch {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Determine the path for the new worktree (inside the repo directory)
    fn determine_worktree_path(&self, repo_path: &Path) -> Result<PathBuf> {
        Ok(repo_path.join(&self.branch))
    }

    async fn collect_repositories(
        &self,
        search_path: &str,
    ) -> Result<Vec<tokio::task::JoinHandle<Result<RepoResult>>>> {
        let mut repo_tasks = Vec::new();
        let entries = fs::read_dir(search_path)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let git_path = path.join(".git");
            if !git_path.exists() {
                continue;
            }

            let path_str = path.to_str().unwrap().to_string();

            let task = tokio::spawn(async move { Self::process_repository(path_str).await });
            repo_tasks.push(task);
        }

        Ok(repo_tasks)
    }

    async fn process_repository(repo_path: String) -> Result<RepoResult> {
        let repo_name = Path::new(&repo_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let repo = GitRepository::new(&repo_path, SystemGitClient)?;

        // Check if it's a bare repository
        if !repo.is_bare().unwrap_or(false) {
            return Ok(RepoResult {
                name: repo_name,
                path: PathBuf::from(&repo_path),
                worktrees: Vec::new(),
            });
        }

        // Get worktree list for this repo - we only need basic info for adding
        let worktrees = repo.list_worktrees()?;

        let worktree_results = worktrees
            .into_iter()
            .map(|worktree| {
                crate::core::WorktreeResult {
                    branch: worktree.branch.clone(),
                    status: crate::core::WorktreeStatus {
                        local_status: crate::git::LocalStatus::Clean, // Placeholder
                        commit_timestamp: 0,                          // Placeholder
                        directory_mtime: 0,                           // Placeholder
                        commit_summary: "<placeholder>".to_string(),  // Placeholder
                        pr_status: None, // No PR status for add command
                    },
                }
            })
            .collect();

        Ok(RepoResult {
            name: repo_name,
            path: PathBuf::from(&repo_path),
            worktrees: worktree_results,
        })
    }
}
