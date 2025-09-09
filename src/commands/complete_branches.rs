use crate::git::{GitRepository, SystemGitClient};
use anyhow::Result;
use clap::Args;
use std::path::Path;

#[derive(Args)]
#[command(hide = true)] // Hidden from help since it's for completion only
pub struct CompleteBranchesCommand {
    /// Repository name to get branches for
    repo: String,

    /// Directory to search for repositories (defaults to current directory)
    /// Can also be set via GWM_REPOS_PATH environment variable
    #[arg(short, long, env = "GWM_REPOS_PATH")]
    path: Option<String>,
}

impl CompleteBranchesCommand {
    pub async fn execute(&self) -> Result<()> {
        let search_path = self.path.as_deref().unwrap_or(".");

        match self.get_branches(search_path, &self.repo) {
            Ok(branches) => {
                for branch in branches {
                    println!("{}", branch);
                }
            }
            Err(_) => {
                // Silently fail for completion - return empty list
            }
        }

        Ok(())
    }

    fn get_branches(&self, search_path: &str, repo_name: &str) -> Result<Vec<String>> {
        let repo_path = Path::new(search_path).join(repo_name);

        if !repo_path.exists() {
            return Ok(vec![]);
        }

        let git_path = repo_path.join(".git");
        if !git_path.exists() {
            return Ok(vec![]);
        }

        let repo = GitRepository::new(repo_path.to_str().unwrap(), SystemGitClient)?;

        // Only get branches from bare repos with worktrees
        if !repo.is_bare().unwrap_or(false) {
            return Ok(vec![]);
        }

        let worktrees = repo.list_worktrees()?;
        let mut branch_names: Vec<String> = worktrees.into_iter().map(|w| w.branch).collect();

        branch_names.sort();
        Ok(branch_names)
    }
}
