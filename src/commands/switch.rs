use anyhow::Result;
use clap::Args;
use futures::future::try_join_all;
use std::fs;
use std::path::Path;

use crate::core::RepoResult;
use crate::git::{GitRepository, SystemGitClient};

#[derive(Args)]
pub struct SwitchCommand {
    /// Repository name
    repo: String,

    /// Branch name to switch to
    branch: String,

    /// Directory to search for repositories (defaults to current directory)
    /// Can also be set via GWM_REPOS_PATH environment variable
    #[arg(short, long, env = "GWM_REPOS_PATH")]
    path: Option<String>,
}

impl SwitchCommand {
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

        let repo_result = match target_repo {
            Some(repo) => repo,
            None => {
                eprintln!("No repository found with name '{}'", self.repo);
                std::process::exit(1);
            }
        };

        // Find the target worktree
        let worktree_path = self.find_worktree_path(repo_result)?;

        match worktree_path {
            Some(path) => {
                // Change to the worktree directory
                std::env::set_current_dir(&path)?;
                println!("📁 Changed to {}", path.display());
            }
            None => {
                eprintln!(
                    "Worktree '{}' not found in repository '{}'",
                    self.branch, self.repo
                );
                std::process::exit(1);
            }
        }

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

    /// Find the path to the worktree for the given branch
    fn find_worktree_path(&self, repo_result: &RepoResult) -> Result<Option<std::path::PathBuf>> {
        // Check if this branch exists as a worktree
        for worktree in &repo_result.worktrees {
            if worktree.branch == self.branch {
                // The worktree path is the branch directory inside the repo
                let worktree_path = repo_result.path.join(&self.branch);
                if worktree_path.exists() {
                    return Ok(Some(worktree_path));
                }
            }
        }
        Ok(None)
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
                path: std::path::PathBuf::from(&repo_path),
                worktrees: Vec::new(),
            });
        }

        // Get worktree list for this repo
        let worktrees = repo.list_worktrees()?;

        let worktree_results = worktrees
            .into_iter()
            .map(|worktree| crate::core::WorktreeResult {
                branch: worktree.branch.clone(),
                status: crate::core::WorktreeStatus {
                    local_status: crate::git::LocalStatus::Clean,
                    commit_timestamp: 0,
                    directory_mtime: 0,
                    commit_summary: "<placeholder>".to_string(),
                },
            })
            .collect();

        Ok(RepoResult {
            name: repo_name,
            path: std::path::PathBuf::from(&repo_path),
            worktrees: worktree_results,
        })
    }
}
