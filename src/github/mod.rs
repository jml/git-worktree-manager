use anyhow::{Result, anyhow};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone)]
pub enum PrStatus {
    Open(u32, Option<String>), // PR number, approval status
    Merged(u32),
    Closed(u32),
    NoPr,
    NoGitHub,
    NoGhCli,
}

impl PrStatus {
    pub fn emoji(&self) -> &'static str {
        match self {
            PrStatus::Open(_, _) => "📋",
            PrStatus::Merged(_) => "✅",
            PrStatus::Closed(_) => "❌",
            PrStatus::NoPr => "➖",
            PrStatus::NoGitHub => "➖",
            PrStatus::NoGhCli => "➖",
        }
    }

    pub fn description(&self) -> String {
        match self {
            PrStatus::Open(num, Some(approval)) => format!("PR Open (#{}) ✓ {}", num, approval),
            PrStatus::Open(num, None) => format!("PR Open (#{}) ⏳", num),
            PrStatus::Merged(num) => format!("PR Merged (#{}) ✅", num),
            PrStatus::Closed(num) => format!("PR Closed (#{}) ❌", num),
            PrStatus::NoPr => "No PR".to_string(),
            PrStatus::NoGitHub => "No GitHub".to_string(),
            PrStatus::NoGhCli => "No gh CLI".to_string(),
        }
    }
}

#[derive(Deserialize)]
struct PrDataWithHead {
    number: u32,
    state: String,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
}

pub struct GitHubIntegration;

impl GitHubIntegration {
    /// Extract GitHub repository info from a remote URL
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
        repo_info: &str,
        branch_names: &[String],
    ) -> Result<HashMap<String, PrStatus>> {
        let mut result = HashMap::new();

        // Check if GitHub CLI is available
        if !Self::is_gh_cli_available() {
            for branch in branch_names {
                result.insert(branch.clone(), PrStatus::NoGhCli);
            }
            return Ok(result);
        }

        // Get all PRs for this repo in one call
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

        if !output.status.success() {
            // If the call fails, mark all branches as NoPr
            for branch in branch_names {
                result.insert(branch.clone(), PrStatus::NoPr);
            }
            return Ok(result);
        }

        let pr_data_str = String::from_utf8_lossy(&output.stdout);
        let pr_data: Vec<PrDataWithHead> = serde_json::from_str(&pr_data_str)?;

        // Create a map of branch -> PR for quick lookup
        let mut branch_to_pr: HashMap<String, &PrDataWithHead> = HashMap::new();
        for pr in &pr_data {
            branch_to_pr.insert(pr.head_ref_name.clone(), pr);
        }

        // Get approval statuses in batch
        let approval_statuses = Self::get_batch_pr_approval_status(repo_info)?;

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

    fn is_gh_cli_available() -> bool {
        Command::new("gh")
            .args(["--version"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn get_batch_pr_approval_status(repo_info: &str) -> Result<HashMap<u32, String>> {
        let mut result = HashMap::new();

        let output = Command::new("gh")
            .args(["pr", "status", "--repo", repo_info])
            .output()?;

        if !output.status.success() {
            return Ok(result);
        }

        let status_str = String::from_utf8_lossy(&output.stdout);

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

        Ok(result)
    }
}
