use anyhow::{Result, anyhow};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

/// Trait for abstracting GitHub CLI operations
pub trait GitHubClient {
    fn is_available(&self) -> bool;
    fn list_prs(&self, repo_info: &str) -> Result<String>;
    fn get_pr_status(&self, repo_info: &str) -> Result<String>;
}

/// Default implementation using system gh command
pub struct SystemGitHubClient;

impl GitHubClient for SystemGitHubClient {
    fn is_available(&self) -> bool {
        Command::new("gh")
            .args(["--version"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn list_prs(&self, repo_info: &str) -> Result<String> {
        let output = Command::new("gh")
            .args([
                "pr",
                "list",
                "--repo",
                repo_info,
                "--json",
                "number,state,title,headRefName",
                "--limit",
                "100",
            ])
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!("Failed to list PRs"))
        }
    }

    fn get_pr_status(&self, repo_info: &str) -> Result<String> {
        let output = Command::new("gh")
            .args(["pr", "status", "--repo", repo_info])
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!("Failed to get PR status"))
        }
    }
}

#[derive(Debug, Clone)]
pub enum PrStatus {
    Open(u32, Option<String>), // PR number, approval status
    Merged(u32),
    Closed(u32),
    NoPr,
    NoGitHub,
    NoGhCli,
}

#[derive(Deserialize)]
struct PrDataWithHead {
    number: u32,
    state: String,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
}

pub struct GitHubIntegration<T: GitHubClient> {
    client: T,
}

impl<T: GitHubClient> GitHubIntegration<T> {
    pub fn new(client: T) -> Self {
        Self { client }
    }

    /// Extract GitHub repository info from a remote URL - pure function
    pub fn get_repo_info(remote_url: &str) -> Result<String> {
        // Match GitHub URLs in both formats: git@github.com:owner/repo.git or https://github.com/owner/repo.git
        let re = Regex::new(r"github\.com[:/]([^/]+)/([^/.]+)").unwrap();

        if let Some(captures) = re.captures(remote_url) {
            let owner = &captures[1];
            let repo = &captures[2];
            Ok(format!("{}/{}", owner, repo))
        } else {
            Err(anyhow!("Not a GitHub repository"))
        }
    }

    /// Get PR status for multiple branches using batched GitHub CLI calls
    pub fn get_batch_pr_status(
        &self,
        repo_info: &str,
        branch_names: &[String],
    ) -> Result<HashMap<String, PrStatus>> {
        let mut result = HashMap::new();

        // Check if GitHub CLI is available
        if !self.client.is_available() {
            for branch in branch_names {
                result.insert(branch.clone(), PrStatus::NoGhCli);
            }
            return Ok(result);
        }

        // Get all PRs for this repo in one call
        let pr_data_str = match self.client.list_prs(repo_info) {
            Ok(output) => output,
            Err(_) => {
                // If the call fails, mark all branches as NoPr
                for branch in branch_names {
                    result.insert(branch.clone(), PrStatus::NoPr);
                }
                return Ok(result);
            }
        };

        let pr_data: Vec<PrDataWithHead> = serde_json::from_str(&pr_data_str)?;

        // Create a map of branch -> PR for quick lookup
        let mut branch_to_pr: HashMap<String, &PrDataWithHead> = HashMap::new();
        for pr in &pr_data {
            branch_to_pr.insert(pr.head_ref_name.clone(), pr);
        }

        // Get approval statuses in batch
        let approval_statuses = self.get_batch_pr_approval_status(repo_info)?;

        // Match branches to PRs
        for branch in branch_names {
            let status = if let Some(pr) = branch_to_pr.get(branch) {
                match pr.state.as_str() {
                    "OPEN" => {
                        let approval_status = approval_statuses.get(&pr.number).cloned();
                        PrStatus::Open(pr.number, approval_status)
                    }
                    "MERGED" => PrStatus::Merged(pr.number),
                    "CLOSED" => PrStatus::Closed(pr.number),
                    _ => PrStatus::Open(pr.number, None),
                }
            } else {
                PrStatus::NoPr
            };
            result.insert(branch.clone(), status);
        }

        Ok(result)
    }

    fn get_batch_pr_approval_status(&self, repo_info: &str) -> Result<HashMap<u32, String>> {
        let status_output = match self.client.get_pr_status(repo_info) {
            Ok(output) => output,
            Err(_) => return Ok(HashMap::new()),
        };

        Ok(Self::parse_pr_approval_status(&status_output))
    }

    /// Pure function to parse PR approval status from gh pr status output
    fn parse_pr_approval_status(status_str: &str) -> HashMap<u32, String> {
        let mut result = HashMap::new();

        // Parse all PR statuses from the output
        for line in status_str.lines() {
            // Look for PR numbers in lines like "  #123  Some PR title"
            if let Some(pr_start) = line.find('#') {
                if let Some(pr_end) = line[pr_start + 1..].find(char::is_whitespace) {
                    if let Ok(pr_number) = line[pr_start + 1..pr_start + 1 + pr_end].parse::<u32>()
                    {
                        if line.contains("Approved") {
                            result.insert(pr_number, "Approved".to_string());
                        } else if line.contains("passing") {
                            result.insert(pr_number, "Checks".to_string());
                        }
                    }
                }
            }
        }

        result
    }
}
