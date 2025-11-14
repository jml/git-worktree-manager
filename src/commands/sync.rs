use anyhow::Result;
use clap::Args;
use futures::future::try_join_all;
use std::fs;
use std::path::Path;

use crate::git::{GitRepository, SystemGitClient};

#[derive(Args)]
pub struct SyncCommand {
    /// Directory to search for repositories (defaults to current directory)
    /// Can also be set via GWM_REPOS_PATH environment variable
    #[arg(short, long, env = "GWM_REPOS_PATH")]
    path: Option<String>,
}

impl SyncCommand {
    pub async fn execute(&self) -> Result<()> {
        let search_path = self.path.as_deref().unwrap_or(".");

        println!("Fetching remotes for all repositories...");

        // Find all repositories and fetch them in parallel
        let fetch_tasks = self.collect_repositories(search_path).await?;

        // Process repositories in parallel
        let results = try_join_all(fetch_tasks).await?;

        // Count successes and failures
        let mut success_count = 0;
        let mut failure_count = 0;
        let mut failed_repos = Vec::new();

        for result in results {
            match result {
                Ok(repo_name) => {
                    success_count += 1;
                    println!("✓ {}", repo_name);
                }
                Err((repo_name, error)) => {
                    failure_count += 1;
                    failed_repos.push((repo_name, error));
                    println!(
                        "✗ {}: {}",
                        failed_repos.last().unwrap().0,
                        failed_repos.last().unwrap().1
                    );
                }
            }
        }

        println!();
        println!(
            "Sync complete: {} successful, {} failed",
            success_count, failure_count
        );

        if failure_count > 0 {
            println!("\nFailed repositories:");
            for (repo_name, error) in failed_repos {
                println!("  {}: {}", repo_name, error);
            }
        }

        Ok(())
    }

    async fn collect_repositories(
        &self,
        search_path: &str,
    ) -> Result<Vec<tokio::task::JoinHandle<Result<String, (String, String)>>>> {
        let mut fetch_tasks = Vec::new();
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
            let repo_name = Path::new(&path_str)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let task =
                tokio::spawn(async move { Self::fetch_repository(path_str, repo_name).await });
            fetch_tasks.push(task);
        }

        Ok(fetch_tasks)
    }

    async fn fetch_repository(
        repo_path: String,
        repo_name: String,
    ) -> Result<String, (String, String)> {
        match GitRepository::new(&repo_path, SystemGitClient) {
            Ok(repo) => {
                // First fetch all remotes
                if let Err(e) = repo.fetch_remotes() {
                    return Err((repo_name, e.to_string()));
                }

                // Then pull main branch if we're in the main worktree
                if let Err(e) = repo.pull_main() {
                    // If pull_main fails (e.g., not on main branch), just log it but don't fail the sync
                    // This allows sync to work for both main worktrees and feature worktrees
                    eprintln!("  Note: Could not pull main for {}: {}", repo_name, e);
                }

                Ok(repo_name)
            }
            Err(e) => Err((repo_name, e.to_string())),
        }
    }
}
