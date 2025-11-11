use anyhow::Result;
use clap::Args;
use futures::future::try_join_all;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::core::{RepoResult, WorktreeResult};
use crate::git::{GitRepository, SystemGitClient};
use crate::output::table;

#[derive(Args)]
pub struct RemoveCommand {
    /// Repository name
    repo: String,

    /// Branch name to remove
    branch: String,

    /// Directory to search for repositories (defaults to current directory)
    /// Can also be set via GWM_REPOS_PATH environment variable
    #[arg(short, long, env = "GWM_REPOS_PATH")]
    path: Option<String>,

    /// Show what would be removed without actually removing anything
    #[arg(long)]
    dry_run: bool,
}

impl RemoveCommand {
    pub async fn execute(&self) -> Result<()> {
        let search_path = self.path.as_deref().unwrap_or(".");

        // Find all repositories
        let repo_tasks = self.collect_repositories(search_path).await?;
        let repo_task_results = try_join_all(repo_tasks).await?;

        let mut repo_results = Vec::new();
        for task_result in repo_task_results {
            repo_results.push(task_result?);
        }

        // Find the specific target
        let target = self.find_target_worktree(&repo_results)?;

        if target.is_none() {
            println!("No worktree found for {}/{}", self.repo, self.branch);
            return Ok(());
        }

        let (repo_result, worktree_result) = target.unwrap();

        // Show what we found
        println!("Target worktree:");
        let target_repo = RepoResult {
            name: repo_result.name.clone(),
            path: repo_result.path.clone(),
            worktrees: vec![worktree_result.clone()],
        };
        let table_output = table::create_table(&[target_repo], true);
        println!("{}", table_output);
        println!();

        if self.dry_run {
            println!(
                "🔍 DRY RUN: Would remove worktree {}/{}",
                self.repo, self.branch
            );
            return Ok(());
        }

        // Ask for confirmation
        print!("❓ Remove worktree {}/{}? [y/N]: ", self.repo, self.branch);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().to_lowercase().starts_with('y') {
            println!("Cancelled.");
            return Ok(());
        }

        // Perform the removal
        let repo = GitRepository::new(repo_result.path.to_str().unwrap(), SystemGitClient)?;
        println!(
            "🗑️  Removing {}/{}",
            repo_result.name, worktree_result.branch
        );
        repo.remove_worktree(&worktree_result.branch)?;

        println!(
            "✅ Successfully removed worktree {}/{}",
            self.repo, self.branch
        );
        Ok(())
    }

    /// Find the specific worktree target
    fn find_target_worktree<'a>(
        &self,
        repo_results: &'a [RepoResult],
    ) -> Result<Option<(&'a RepoResult, &'a WorktreeResult)>> {
        for repo_result in repo_results {
            if repo_result.name == self.repo {
                for worktree in &repo_result.worktrees {
                    if worktree.branch == self.branch {
                        return Ok(Some((repo_result, worktree)));
                    }
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
                path: PathBuf::from(&repo_path),
                worktrees: Vec::new(),
            });
        }

        // Get worktree list for this repo
        let worktrees = repo.list_worktrees()?;

        if worktrees.is_empty() {
            return Ok(RepoResult {
                name: repo_name,
                path: PathBuf::from(&repo_path),
                worktrees: Vec::new(),
            });
        }

        // For removal, we only need basic worktree info - skip expensive status checks
        let mut worktree_results = Vec::new();
        for worktree in worktrees {
            worktree_results.push(WorktreeResult {
                branch: worktree.branch.clone(),
                status: crate::core::WorktreeStatus {
                    local_status: crate::git::LocalStatus::Clean, // Placeholder
                    commit_timestamp: 0,                          // Placeholder
                    directory_mtime: 0,                           // Placeholder
                    commit_summary: "<placeholder>".to_string(),  // Placeholder
                },
            });
        }

        Ok(RepoResult {
            name: repo_name,
            path: PathBuf::from(&repo_path),
            worktrees: worktree_results,
        })
    }
}
