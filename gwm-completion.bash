#!/bin/bash
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

# Determine which positional argument we're completing
_gwm_get_positional_arg_index() {
    local cmd="$1"
    local flag_count=0
    local positional_count=0

    local i
    for (( i=0; i < COMP_CWORD; i++ )); do
        local word="${COMP_WORDS[i]}"

        # Skip the command name
        if [[ i -eq 0 ]]; then
            continue
        fi

        # Skip the subcommand
        if [[ "$word" == "$cmd" ]]; then
            continue
        fi

        # Skip flags and their values
        if [[ "$word" =~ ^- ]]; then
            # Skip this flag
            case "$word" in
                --path|-p|--base-branch|-b|--older-than|--newer-than)
                    # These flags take a value, so skip the next word too
                    i=$((i+1))
                    ;;
                *)
                    # Other flags don't take values
                    ;;
            esac
        else
            # This is a positional argument
            positional_count=$((positional_count+1))
        fi
    done

    echo $positional_count
}

_gwm() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="gwm"
                ;;
            gwm,add)
                cmd="gwm__add"
                ;;
            gwm,cleanup)
                cmd="gwm__cleanup"
                ;;
            gwm,complete-branches)
                cmd="gwm__complete__branches"
                ;;
            gwm,complete-repos)
                cmd="gwm__complete__repos"
                ;;
            gwm,completion)
                cmd="gwm__completion"
                ;;
            gwm,help)
                cmd="gwm__help"
                ;;
            gwm,list)
                cmd="gwm__list"
                ;;
            gwm,remove)
                cmd="gwm__remove"
                ;;
            gwm,sync)
                cmd="gwm__sync"
                ;;
            gwm__help,add)
                cmd="gwm__help__add"
                ;;
            gwm__help,cleanup)
                cmd="gwm__help__cleanup"
                ;;
            gwm__help,complete-branches)
                cmd="gwm__help__complete__branches"
                ;;
            gwm__help,complete-repos)
                cmd="gwm__help__complete__repos"
                ;;
            gwm__help,completion)
                cmd="gwm__help__completion"
                ;;
            gwm__help,help)
                cmd="gwm__help__help"
                ;;
            gwm__help,list)
                cmd="gwm__help__list"
                ;;
            gwm__help,remove)
                cmd="gwm__help__remove"
                ;;
            gwm__help,sync)
                cmd="gwm__help__sync"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        gwm)
            opts="-p -h -V --path --no-emoji --prune-candidates --active --needs-attention --stale --dirty --clean --staged --missing --ahead --behind --diverged --not-pushed --not-tracking --up-to-date --older-than --newer-than --help --version list add cleanup remove sync completion complete-repos complete-branches help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --older-than)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --newer-than)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        gwm__add)
            opts="-b -p -h --base-branch --path --dry-run --help"

            # Check if we're completing flags
            if [[ ${cur} == -* ]]; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi

            # Handle flag values
            case "${prev}" in
                --base-branch)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -b)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    # Handle positional arguments (repo and branch)
                    local pos_arg_index=$(_gwm_get_positional_arg_index "add")

                    if [[ $pos_arg_index -eq 1 ]]; then
                        # First positional argument: repository name
                        local repos=$(_gwm_complete_repos)
                        COMPREPLY=( $(compgen -W "${repos}" -- "${cur}") )
                        return 0
                    elif [[ $pos_arg_index -eq 2 ]]; then
                        # Second positional argument: branch name
                        # We need to find the repository name from previous arguments
                        local repo=""
                        local i
                        local positional_found=0
                        for (( i=2; i < COMP_CWORD; i++ )); do
                            local word="${COMP_WORDS[i]}"
                            # Skip flags and their values
                            if [[ "$word" =~ ^- ]]; then
                                case "$word" in
                                    --path|-p|--base-branch|-b)
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

                        if [[ -n "$repo" ]]; then
                            local branches=$(_gwm_complete_branches "$repo")
                            COMPREPLY=( $(compgen -W "${branches}" -- "${cur}") )
                            return 0
                        fi
                    fi
                    ;;
            esac
            return 0
            ;;
        gwm__remove)
            opts="-p -h --path --dry-run --help"

            # Check if we're completing flags
            if [[ ${cur} == -* ]]; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi

            # Handle flag values
            case "${prev}" in
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    # Handle positional arguments (repo and branch)
                    local pos_arg_index=$(_gwm_get_positional_arg_index "remove")

                    if [[ $pos_arg_index -eq 1 ]]; then
                        # First positional argument: repository name
                        local repos=$(_gwm_complete_repos)
                        COMPREPLY=( $(compgen -W "${repos}" -- "${cur}") )
                        return 0
                    elif [[ $pos_arg_index -eq 2 ]]; then
                        # Second positional argument: branch name
                        # Find the repository name from previous arguments
                        local repo=""
                        local i
                        local positional_found=0
                        for (( i=2; i < COMP_CWORD; i++ )); do
                            local word="${COMP_WORDS[i]}"
                            # Skip flags and their values
                            if [[ "$word" =~ ^- ]]; then
                                case "$word" in
                                    --path|-p)
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

                        if [[ -n "$repo" ]]; then
                            local branches=$(_gwm_complete_branches "$repo")
                            COMPREPLY=( $(compgen -W "${branches}" -- "${cur}") )
                            return 0
                        fi
                    fi
                    ;;
            esac
            return 0
            ;;
        # Keep the rest of the completion cases from the original (simplified here for brevity)
        *)
            # Fall back to the original generated completion for other commands
            gwm completion bash | source /dev/stdin
            return $?
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _gwm -o nosort -o bashdefault -o default gwm
else
    complete -F _gwm -o bashdefault -o default gwm
fi
