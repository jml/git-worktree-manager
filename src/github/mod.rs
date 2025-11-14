use anyhow::{Result, anyhow};
use octocrab::Octocrab;
use regex::Regex;
use std::collections::HashMap;

use crate::core::PrStatus;

/// Represents a GitHub repository (owner and name)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GitHubRepo {
    pub owner: String,
    pub repo: String,
}

/// Represents PR information for matching with worktrees
#[derive(Debug, Clone)]
pub struct PrInfo {
    #[allow(dead_code)]
    pub number: u64,
    pub head_branch: String,
    pub status: PrStatus,
}

/// Parse a GitHub remote URL to extract owner and repo
/// Handles both SSH (git@github.com:owner/repo.git) and HTTPS (https://github.com/owner/repo.git) formats
pub fn parse_github_url(url: &str) -> Result<GitHubRepo> {
    // SSH format: git@github.com:owner/repo.git
    let ssh_regex = Regex::new(r"git@github\.com:([^/]+)/(.+?)(?:\.git)?$")?;
    if let Some(captures) = ssh_regex.captures(url) {
        return Ok(GitHubRepo {
            owner: captures[1].to_string(),
            repo: captures[2].to_string(),
        });
    }

    // HTTPS format: https://github.com/owner/repo.git or https://github.com/owner/repo
    let https_regex = Regex::new(r"https://github\.com/([^/]+)/(.+?)(?:\.git)?$")?;
    if let Some(captures) = https_regex.captures(url) {
        return Ok(GitHubRepo {
            owner: captures[1].to_string(),
            repo: captures[2].to_string(),
        });
    }

    Err(anyhow!("Failed to parse GitHub URL: {}", url))
}

/// Fetch PRs for a repository created by the authenticated user
/// Filters by creation date (PRs created after `since_timestamp`)
/// Uses GitHub Search API for efficient server-side filtering
pub async fn fetch_prs_for_repo(
    github_client: &Octocrab,
    repo: &GitHubRepo,
    since_timestamp: i64,
) -> Result<Vec<PrInfo>> {
    let start_time = std::time::Instant::now();

    // Convert timestamp to date string for search query
    let since_date = chrono::DateTime::from_timestamp(since_timestamp, 0)
        .ok_or_else(|| anyhow!("Invalid timestamp: {}", since_timestamp))?;
    let date_string = since_date.format("%Y-%m-%d").to_string();

    // Build search query: repo:owner/repo is:pr author:@me created:>=date
    let query = format!(
        "repo:{}/{} is:pr author:@me created:>={}",
        repo.owner, repo.repo, date_string
    );

    eprintln!("[GitHub API] Searching PRs with query: {}", query);

    let mut page = 1u32;
    let mut all_prs = Vec::new();

    loop {
        eprintln!(
            "[GitHub API] GET /search/issues?q={}&per_page=100&page={}",
            urlencoding::encode(&query),
            page
        );

        let results = github_client
            .search()
            .issues_and_pull_requests(&query)
            .per_page(100)
            .page(page)
            .send()
            .await?;

        let page_size = results.items.len();

        if page_size == 0 {
            break;
        }

        eprintln!("[GitHub API] Page {} returned {} results", page, page_size);

        let has_more_pages = page_size >= 100;

        for issue in results.items {
            // The search API returns issues, but we filtered for is:pr
            // We need to extract PR-specific information
            if issue.pull_request.is_some() {
                // Fetch the full PR to get head branch and other details
                // Note: issue.pull_request only has url/html_url, not the full PR data
                // We need to extract PR number from the issue and fetch it

                // Issue number is the same as PR number
                let pr_number = issue.number;

                // Fetch full PR details
                let pr = github_client
                    .pulls(&repo.owner, &repo.repo)
                    .get(pr_number)
                    .await?;

                // Determine PR status
                let status = if pr.merged_at.is_some() {
                    PrStatus::Merged
                } else if pr.draft.unwrap_or(false) {
                    PrStatus::Draft
                } else if pr.state == Some(octocrab::models::IssueState::Open) {
                    PrStatus::Open
                } else {
                    PrStatus::Closed
                };

                all_prs.push(PrInfo {
                    number: pr_number,
                    head_branch: pr.head.ref_field,
                    status,
                });
            }
        }

        if !has_more_pages {
            break;
        }

        page += 1;
    }

    let elapsed = start_time.elapsed();
    eprintln!(
        "[GitHub API] Search completed in {:?}, found {} PRs for {}/{}",
        elapsed,
        all_prs.len(),
        repo.owner,
        repo.repo
    );

    Ok(all_prs)
}

/// Match worktree branches to PRs using exact branch name matching
pub fn match_worktrees_to_prs(
    worktree_branches: &[String],
    prs: &[PrInfo],
) -> HashMap<String, PrStatus> {
    let mut matches = HashMap::new();

    for branch in worktree_branches {
        for pr in prs {
            if branch == &pr.head_branch {
                matches.insert(branch.clone(), pr.status.clone());
                break;
            }
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_github_url() {
        let url = "git@github.com:jml/git-worktree-manager.git";
        let repo = parse_github_url(url).unwrap();
        assert_eq!(repo.owner, "jml");
        assert_eq!(repo.repo, "git-worktree-manager");
    }

    #[test]
    fn parses_ssh_github_url_without_git_extension() {
        let url = "git@github.com:jml/git-worktree-manager";
        let repo = parse_github_url(url).unwrap();
        assert_eq!(repo.owner, "jml");
        assert_eq!(repo.repo, "git-worktree-manager");
    }

    #[test]
    fn parses_https_github_url() {
        let url = "https://github.com/jml/git-worktree-manager.git";
        let repo = parse_github_url(url).unwrap();
        assert_eq!(repo.owner, "jml");
        assert_eq!(repo.repo, "git-worktree-manager");
    }

    #[test]
    fn parses_https_github_url_without_git_extension() {
        let url = "https://github.com/jml/git-worktree-manager";
        let repo = parse_github_url(url).unwrap();
        assert_eq!(repo.owner, "jml");
        assert_eq!(repo.repo, "git-worktree-manager");
    }

    #[test]
    fn matches_worktrees_to_prs_exact_match() {
        let branches = vec!["feature-1".to_string(), "feature-2".to_string()];
        let prs = vec![
            PrInfo {
                number: 1,
                head_branch: "feature-1".to_string(),
                status: PrStatus::Open,
            },
            PrInfo {
                number: 2,
                head_branch: "feature-3".to_string(),
                status: PrStatus::Draft,
            },
        ];

        let matches = match_worktrees_to_prs(&branches, &prs);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches.get("feature-1"), Some(&PrStatus::Open));
        assert_eq!(matches.get("feature-2"), None);
    }
}
