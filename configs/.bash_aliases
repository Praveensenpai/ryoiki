# ==============================================================================
#  領域 (Ryoiki) - Bash Aliases
# ==============================================================================

# Environment paths (Cargo for Rust, ~/.local/bin for uv & toss)
case ":$PATH:" in
    *":$HOME/.cargo/bin:"*) ;;
    *) export PATH="$HOME/.cargo/bin:$PATH" ;;
esac
case ":$PATH:" in
    *":$HOME/.local/bin:"*) ;;
    *) export PATH="$HOME/.local/bin:$PATH" ;;
esac

# Replace ls with eza
if command -v eza &>/dev/null; then
    alias ls='eza --group-directories-first'
    alias ll='eza -la --icons --git --group-directories-first'
    alias lt='eza --tree --level=2'
fi

# Replace cat with bat
if command -v bat &>/dev/null; then
    alias cat='bat --paging=never'
elif command -v batcat &>/dev/null; then
    alias cat='batcat --paging=never'
    alias bat='batcat'
fi

# Replace rm with toss (safe FreeDesktop trash manager)
if command -v toss &>/dev/null; then
    alias rm='toss put'
fi

# Editor shortcuts
if command -v nvim &>/dev/null; then
    alias v='nvim'
    alias vi='nvim'
    alias vim='nvim'
fi

# Git shortcuts
alias gs='git status'
alias gp='git push'
alias gl='git pull'

# Zoxide (smarter cd + auto-completion)
if command -v zoxide &>/dev/null; then
    eval "$(zoxide init bash)"
    alias cd='z'
fi
