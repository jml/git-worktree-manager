use anyhow::Result;
use clap::{Args, CommandFactory};
use clap_complete::{Shell, generate};
use std::io;

#[derive(Args)]
pub struct CompletionCommand {
    /// The shell to generate completions for
    #[arg(value_enum)]
    shell: Shell,
}

impl CompletionCommand {
    pub async fn execute(&self) -> Result<()> {
        match self.shell {
            Shell::Bash => self.generate_enhanced_bash_completion().await,
            Shell::Zsh => self.generate_enhanced_zsh_completion().await,
            _ => {
                // For other shells, use the default completion
                let mut cmd = crate::Cli::command();
                generate(self.shell, &mut cmd, "gwm", &mut io::stdout());
                Ok(())
            }
        }
    }

    async fn generate_enhanced_bash_completion(&self) -> Result<()> {
        // First generate the base completion
        let mut cmd = crate::Cli::command();
        let mut output = Vec::new();
        generate(Shell::Bash, &mut cmd, "gwm", &mut output);

        let base_completion = String::from_utf8(output)?;

        // Add our custom completion functions
        let enhanced_completion = self.enhance_bash_completion(&base_completion);

        print!("{}", enhanced_completion);
        Ok(())
    }

    async fn generate_enhanced_zsh_completion(&self) -> Result<()> {
        // Generate the base completion
        let mut cmd = crate::Cli::command();
        let mut output = Vec::new();
        generate(Shell::Zsh, &mut cmd, "gwm", &mut output);

        let base_completion = String::from_utf8(output)?;

        // Add our custom completion functions for zsh
        let enhanced_completion = self.enhance_zsh_completion(&base_completion);

        print!("{}", enhanced_completion);
        Ok(())
    }

    fn enhance_bash_completion(&self, base: &str) -> String {
        let custom_functions = r#"
# Enhanced gwm completion with dynamic repository and branch name completion

# Helper functions for dynamic completion
_gwm_complete_repos() {
    local path_arg=""

    # Extract path argument if present
    local i
    for (( i=0; i < ${#COMP_WORDS[@]}; i++ )); do
        if [[ "${COMP_WORDS[i]}" == "--path" ]] && [[ $((i+1)) -lt ${#COMP_WORDS[@]} ]]; then
            path_arg="--path ${COMP_WORDS[i+1]}"
            break
        elif [[ "${COMP_WORDS[i]}" == "-p" ]] && [[ $((i+1)) -lt ${#COMP_WORDS[@]} ]]; then
            path_arg="--path ${COMP_WORDS[i+1]}"
            break
        fi
    done

    # Call gwm to get repository names
    gwm complete-repos $path_arg 2>/dev/null
}

_gwm_complete_branches() {
    local repo="$1"
    local path_arg=""

    # Extract path argument if present
    local i
    for (( i=0; i < ${#COMP_WORDS[@]}; i++ )); do
        if [[ "${COMP_WORDS[i]}" == "--path" ]] && [[ $((i+1)) -lt ${#COMP_WORDS[@]} ]]; then
            path_arg="--path ${COMP_WORDS[i+1]}"
            break
        elif [[ "${COMP_WORDS[i]}" == "-p" ]] && [[ $((i+1)) -lt ${#COMP_WORDS[@]} ]]; then
            path_arg="--path ${COMP_WORDS[i+1]}"
            break
        fi
    done

    # Call gwm to get branch names for the specified repository
    gwm complete-branches "$repo" $path_arg 2>/dev/null
}

# Get positional argument index for dynamic completion
_gwm_get_positional_index() {
    local current_cmd="$1"
    local positional_count=0

    local i
    for (( i=2; i < COMP_CWORD; i++ )); do
        local word="${COMP_WORDS[i]}"

        # Skip the subcommand itself
        if [[ "$word" == "$current_cmd" ]]; then
            continue
        fi

        # Skip flags and their values
        if [[ "$word" =~ ^- ]]; then
            case "$word" in
                --path|-p|--base-branch|-b|--older-than|--newer-than)
                    # These flags take a value, skip next word too
                    i=$((i+1))
                    ;;
            esac
        else
            # This is a positional argument
            positional_count=$((positional_count+1))
        fi
    done

    echo $positional_count
}

# Find repository name from previous arguments
_gwm_find_repo_arg() {
    local current_cmd="$1"
    local repo=""
    local positional_found=0

    local i
    for (( i=2; i < COMP_CWORD; i++ )); do
        local word="${COMP_WORDS[i]}"

        # Skip the subcommand itself
        if [[ "$word" == "$current_cmd" ]]; then
            continue
        fi

        # Skip flags and their values
        if [[ "$word" =~ ^- ]]; then
            case "$word" in
                --path|-p|--base-branch|-b|--older-than|--newer-than)
                    i=$((i+1))  # Skip the flag value
                    ;;
            esac
        else
            # This is a positional argument
            positional_found=$((positional_found+1))
            if [[ $positional_found -eq 1 ]]; then
                repo="$word"
                break
            fi
        fi
    done

    echo "$repo"
}

"#;

        // Replace the gwm__add and gwm__remove sections with enhanced versions
        let enhanced = base
            .replace(
                "        gwm__add)\n            opts=\"-b -p -h --base-branch --path --dry-run --help <REPO> <BRANCH>\"\n            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then\n                COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n                return 0\n            fi\n            case \"${prev}\" in\n                --base-branch)\n                    COMPREPLY=($(compgen -f \"${cur}\"))\n                    return 0\n                    ;;\n                -b)\n                    COMPREPLY=($(compgen -f \"${cur}\"))\n                    return 0\n                    ;;\n                --path)\n                    COMPREPLY=($(compgen -f \"${cur}\"))\n                    return 0\n                    ;;\n                -p)\n                    COMPREPLY=($(compgen -f \"${cur}\"))\n                    return 0\n                    ;;\n                *)\n                    COMPREPLY=()\n                    ;;\n            esac\n            COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n            return 0\n            ;;",
                r#"        gwm__add)
            opts="-b -p -h --base-branch --path --dry-run --help"

            # Check if we're completing flags
            if [[ ${cur} == -* ]]; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi

            # Handle flag values
            case "${prev}" in
                --base-branch|-b|--path|-p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    # Handle positional arguments (repo and branch)
                    local pos_index=$(_gwm_get_positional_index "add")

                    if [[ $pos_index -eq 1 ]]; then
                        # First positional: repository name
                        local repos=$(_gwm_complete_repos)
                        COMPREPLY=( $(compgen -W "${repos}" -- "${cur}") )
                        return 0
                    elif [[ $pos_index -eq 2 ]]; then
                        # Second positional: branch name (new branch, so no completion)
                        # Let user type freely since they're creating a new branch
                        COMPREPLY=()
                        return 0
                    fi
                    ;;
            esac
            return 0
            ;;"#
            )
            .replace(
                "        gwm__remove)\n            opts=\"-p -h --path --dry-run --help <REPO> <BRANCH>\"\n            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then\n                COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n                return 0\n            fi\n            case \"${prev}\" in\n                --path)\n                    COMPREPLY=($(compgen -f \"${cur}\"))\n                    return 0\n                    ;;\n                -p)\n                    COMPREPLY=($(compgen -f \"${cur}\"))\n                    return 0\n                    ;;\n                *)\n                    COMPREPLY=()\n                    ;;\n            esac\n            COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n            return 0\n            ;;",
                r#"        gwm__remove)
            opts="-p -h --path --dry-run --help"

            # Check if we're completing flags
            if [[ ${cur} == -* ]]; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi

            # Handle flag values
            case "${prev}" in
                --path|-p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    # Handle positional arguments (repo and branch)
                    local pos_index=$(_gwm_get_positional_index "remove")

                    if [[ $pos_index -eq 1 ]]; then
                        # First positional: repository name
                        local repos=$(_gwm_complete_repos)
                        COMPREPLY=( $(compgen -W "${repos}" -- "${cur}") )
                        return 0
                    elif [[ $pos_index -eq 2 ]]; then
                        # Second positional: branch name
                        local repo=$(_gwm_find_repo_arg "remove")
                        if [[ -n "$repo" ]]; then
                            local branches=$(_gwm_complete_branches "$repo")
                            COMPREPLY=( $(compgen -W "${branches}" -- "${cur}") )
                            return 0
                        fi
                    fi
                    ;;
            esac
            return 0
            ;;"#
            );

        format!("{}{}", custom_functions, enhanced)
    }

    fn enhance_zsh_completion(&self, base: &str) -> String {
        let custom_functions = r#"
# Enhanced gwm completion with dynamic repository and branch name completion

# Helper functions for dynamic completion in zsh
_gwm_complete_repos() {
    local path_arg=""

    # Extract path argument if present
    local i
    for (( i=1; i <= ${#words[@]}; i++ )); do
        if [[ "${words[i]}" == "--path" ]] && [[ $((i+1)) -le ${#words[@]} ]]; then
            path_arg="--path ${words[i+1]}"
            break
        elif [[ "${words[i]}" == "-p" ]] && [[ $((i+1)) -le ${#words[@]} ]]; then
            path_arg="--path ${words[i+1]}"
            break
        fi
    done

    # Call gwm to get repository names and return as array
    local repos
    repos=($(gwm complete-repos $path_arg 2>/dev/null))
    _describe 'repositories' repos
}

_gwm_complete_branches() {
    local repo="$1"
    local path_arg=""

    # Extract path argument if present
    local i
    for (( i=1; i <= ${#words[@]}; i++ )); do
        if [[ "${words[i]}" == "--path" ]] && [[ $((i+1)) -le ${#words[@]} ]]; then
            path_arg="--path ${words[i+1]}"
            break
        elif [[ "${words[i]}" == "-p" ]] && [[ $((i+1)) -le ${#words[@]} ]]; then
            path_arg="--path ${words[i+1]}"
            break
        fi
    done

    # Call gwm to get branch names and return as array
    local branches
    branches=($(gwm complete-branches "$repo" $path_arg 2>/dev/null))
    _describe 'branches' branches
}

# Get positional argument index for dynamic completion
_gwm_get_positional_index_zsh() {
    local current_cmd="$1"
    local positional_count=0

    local i
    for (( i=2; i <= ${#words[@]}; i++ )); do
        local word="${words[i]}"

        # Skip the subcommand itself
        if [[ "$word" == "$current_cmd" ]]; then
            continue
        fi

        # Skip flags and their values
        if [[ "$word" =~ ^- ]]; then
            case "$word" in
                --path|-p|--base-branch|-b|--older-than|--newer-than)
                    # These flags take a value, skip next word too
                    i=$((i+1))
                    ;;
            esac
        else
            # This is a positional argument
            positional_count=$((positional_count+1))
        fi
    done

    echo $positional_count
}

# Find repository name from previous arguments
_gwm_find_repo_arg_zsh() {
    local current_cmd="$1"
    local repo=""
    local positional_found=0

    local i
    for (( i=2; i <= ${#words[@]}; i++ )); do
        local word="${words[i]}"

        # Skip the subcommand itself
        if [[ "$word" == "$current_cmd" ]]; then
            continue
        fi

        # Skip flags and their values
        if [[ "$word" =~ ^- ]]; then
            case "$word" in
                --path|-p|--base-branch|-b|--older-than|--newer-than)
                    i=$((i+1))  # Skip the flag value
                    ;;
            esac
        else
            # This is a positional argument
            positional_found=$((positional_found+1))
            if [[ $positional_found -eq 1 ]]; then
                repo="$word"
                break
            fi
        fi
    done

    echo "$repo"
}

"#;

        // For zsh, we need to add custom completion functions
        // But only apply branch completion to remove command, not add command
        let mut enhanced = base.to_string();

        // Replace repository completion for both add and remove commands
        enhanced = enhanced.replace(
            ":repo -- Repository name:_default",
            ":repo -- Repository name:_gwm_complete_repos",
        );

        // Only apply branch completion to remove command
        // Find the remove command section and replace branch completion there
        if let Some(remove_start) = enhanced.find("(remove)")
            && let Some(remove_end) = enhanced[remove_start..].find(";;")
        {
            let remove_section_end = remove_start + remove_end + 2;
            let remove_section = &enhanced[remove_start..remove_section_end];

            // Replace branch completion only in the remove section
            let updated_remove = remove_section.replace(
                ":branch -- Branch name to remove:_default",
                ":branch -- Branch name to remove:_gwm_complete_branches",
            );

            enhanced = format!(
                "{}{}{}",
                &enhanced[..remove_start],
                updated_remove,
                &enhanced[remove_section_end..]
            );
        }

        // For add command, we want to keep the default (no completion) for branch name
        // The add command should already have ":branch -- Branch name to create:_default"
        // and we leave that as _default (no custom completion)

        format!("{}{}", custom_functions, enhanced)
    }
}
