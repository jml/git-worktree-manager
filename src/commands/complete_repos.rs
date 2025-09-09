use anyhow::Result;
use clap::Args;
use std::fs;

#[derive(Args)]
#[command(hide = true)] // Hidden from help since it's for completion only
pub struct CompleteReposCommand {
    /// Directory to search for repositories (defaults to current directory)
    /// Can also be set via GWM_REPOS_PATH environment variable
    #[arg(short, long, env = "GWM_REPOS_PATH")]
    path: Option<String>,
}

impl CompleteReposCommand {
    pub async fn execute(&self) -> Result<()> {
        let search_path = self.path.as_deref().unwrap_or(".");

        match self.scan_repositories(search_path) {
            Ok(repos) => {
                for repo in repos {
                    println!("{}", repo);
                }
            }
            Err(_) => {
                // Silently fail for completion - return empty list
            }
        }

        Ok(())
    }

    fn scan_repositories(&self, search_path: &str) -> Result<Vec<String>> {
        let mut repo_names = Vec::new();
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

            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                repo_names.push(name.to_string());
            }
        }

        repo_names.sort();
        Ok(repo_names)
    }
}
