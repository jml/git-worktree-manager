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
sudo cp target/release/gwm /usr/local/bin/
```

### As a Git Subcommand

Once installed, you can use it as a git subcommand:

```bash
git worktree-manager
# or explicitly:
git worktree-manager list
```

### Shell Completion

Enable tab completion for repository and branch names by generating and sourcing completion scripts:

**Features:**
- Command and flag completion for all gwm commands
- Dynamic repository name completion for `add` and `remove` commands
- Dynamic branch name completion based on selected repository
- Respects `--path` flag and `GWM_REPOS_PATH` environment variable

#### Bash

```bash
# Generate enhanced completion script with dynamic repo/branch completion
gwm completion bash > ~/.gwm_completion

# Add to your .bashrc or .bash_profile
echo "source ~/.gwm_completion" >> ~/.bashrc
source ~/.bashrc
```

#### Zsh

For zsh, you need to place the completion in your fpath. Here are the most common approaches:

**Option 1: Using ~/.zfunc directory (recommended)**
```zsh
# Create completion directory if it doesn't exist
mkdir -p ~/.zfunc

# Generate completion script
gwm completion zsh > ~/.zfunc/_gwm

# Add to your .zshrc (add this line before 'compinit' if you have it)
echo 'fpath=(~/.zfunc $fpath)' >> ~/.zshrc
echo 'autoload -U compinit && compinit' >> ~/.zshrc

# Reload your shell
exec zsh
```

**Option 2: System-wide installation (requires sudo)**
```zsh
# Generate and install system-wide (adjust path for your system)
gwm completion zsh | sudo tee /usr/local/share/zsh/site-functions/_gwm > /dev/null

# Reload completions
compinit
```

**Option 3: Using oh-my-zsh custom completions**
```zsh
# If you use oh-my-zsh
gwm completion zsh > ~/.oh-my-zsh/custom/plugins/gwm/_gwm
# Then add 'gwm' to your plugins list in .zshrc
```

#### Fish

```bash
# Generate completion script and place it in fish completions directory
gwm completion fish > ~/.config/fish/completions/gwm.fish
```

#### PowerShell

```powershell
# Generate completion script
gwm completion powershell > gwm_completion.ps1

# Add to your PowerShell profile
echo ". gwm_completion.ps1" >> $PROFILE
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
