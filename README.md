# git-worktree-manager

An opinionated git worktree management tool.

## Features

- **Comprehensive Status Tracking**: Shows local changes and remote sync status
- **Multi-Repository Support**: Scans directories for bare git repositories with worktrees
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
git worktree-manager
# or explicitly:
git worktree-manager list
```

## Usage

### List Work in Progress

Display all work-in-progress (non-main) worktrees across repositories:

```bash
# Default command - just run without subcommand:
git-worktree-manager

# Or explicitly use the list subcommand:
git-worktree-manager list
```

Options:
- `--path <PATH>`: Directory to search for repositories (defaults to current directory)
- `--no-emoji`: Disable emoji in status output

### Example Output

```
+------------+---------------+---------+----------+
| Repository | Branch        | Local   | Remote   |
+------------+---------------+---------+----------+
| my-project | feature-branch| 🔧 Dirty| ⬆️ Ahead 2|
+------------+---------------+---------+----------+

Total WIP branches: 1
Repositories with WIP: 1
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


## Requirements

- Rust 1.70+
- Git

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
# Default behavior:
cargo run

# Or explicitly:
cargo run -- list
```
