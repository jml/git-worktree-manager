# git-worktree-manager

An opinionated git worktree management tool with comprehensive GitHub integration.

## Features

- **Comprehensive Status Tracking**: Shows local changes, remote sync status, and GitHub PR information
- **Multi-Repository Support**: Scans directories for bare git repositories with worktrees
- **GitHub Integration**: Uses `gh` CLI to fetch PR status, approvals, and checks
- **Action Items**: Provides actionable recommendations based on current branch states
- **Colored Output**: Easy-to-read status indicators with emojis and colors

## Installation

### From Source

```bash
git clone <repository-url>
cd git-worktree-manager
cargo build --release
sudo cp target/release/git-worktree-manager /usr/local/bin/
```

### As a Git Subcommand

Once installed, you can use it as a git subcommand:

```bash
git worktree-manager show-wip
```

## Usage

### Show Work in Progress

Display all work-in-progress (non-main) worktrees across repositories:

```bash
git-worktree-manager show-wip
```

Options:
- `--path <PATH>`: Directory to search for repositories (defaults to current directory)

### Example Output

```
📋 Work In Progress - GitHub-Integrated Status Overview
======================================================

📁 my-project
  🔨 feature-branch
    📍 /path/to/my-project/feature-branch
    🔧 Dirty | ⬆️ Ahead 2 | 📋 PR Open (#123) ✓ Approved

📊 Comprehensive Summary
========================
Total WIP branches: 1
Repositories with WIP: 1

🎯 Action Items:
   • Commit changes in 1 dirty branches
   • Push 1 ahead branches
```

## Status Indicators

### Local Status
- ✅ **Clean**: No uncommitted changes
- 🔧 **Dirty**: Uncommitted changes present
- 📦 **Staged**: Changes staged for commit
- ❌ **Missing**: Worktree directory doesn't exist

### Remote Status
- ✅ **Up to date**: In sync with remote
- ⬆️ **Ahead N**: N commits ahead of remote
- ⬇️ **Behind N**: N commits behind remote
- 🔀 **Diverged**: Both ahead and behind remote
- ❌ **Not pushed**: Branch doesn't exist on remote
- 🔄 **Not tracking**: Branch exists but not tracking remote

### PR Status
- 📋 **PR Open**: Pull request is open
- ✅ **PR Merged**: Pull request was merged
- ❌ **PR Closed**: Pull request was closed
- ➖ **No PR**: No pull request found
- ➖ **No GitHub**: Not a GitHub repository

## Requirements

- Rust 1.70+
- Git
- `gh` CLI (for GitHub integration)

## Future Features

This tool is designed to be extensible with additional worktree management features:

- Create new worktrees
- Clean up merged branches
- Sync worktrees with remote changes
- Batch operations across multiple repositories
- Custom status filters and queries

## Development

```bash
cargo build
cargo test
cargo run -- show-wip
```