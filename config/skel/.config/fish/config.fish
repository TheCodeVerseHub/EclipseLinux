if status is-interactive
    # ┌─────────────────────────────────────────────────────────────────┐
    # │  Eclipse Linux - Fish Shell Configuration                       │
    # │  Adapted from Niri minimal dots for Void Linux                  │
    # └─────────────────────────────────────────────────────────────────┘

    # ─────────────────────────────────────────────────────────────────
    # ENVIRONMENT VARIABLES - Core Setup
    # ─────────────────────────────────────────────────────────────────

    set -gx LC_ALL en_US.UTF-8
    set -gx LANG en_US.UTF-8

    set -gx PATH $PATH $HOME/bin $HOME/.local/bin

    set -gx TERMINAL alacritty

    set -gx XCURSOR_THEME Bibata-Modern-Classic
    set -gx XCURSOR_SIZE 24

    set -U fish_greeting

    # ─────────────────────────────────────────────────────────────────
    # STARSHIP PROMPT
    # ─────────────────────────────────────────────────────────────────

    set -gx STARSHIP_CONFIG $HOME/.config/prompt/starship.toml

    if command -v starship >/dev/null
        starship init fish | source
    end

    # ─────────────────────────────────────────────────────────────────
    # COLORS & SYNTAX HIGHLIGHTING - Catppuccin Mocha
    # ─────────────────────────────────────────────────────────────────

    set fish_color_normal           cdd6f4
    set fish_color_command          74c7ec --bold
    set fish_color_keyword          cba6f7 --bold
    set fish_color_quote            a6e3a1
    set fish_color_redirection      fab387 --bold
    set fish_color_end              89b4fa --bold
    set fish_color_error            f38ba8 --bold
    set fish_color_param            f9e2af
    set fish_color_option           94e2d5
    set fish_color_comment          6c7086 --bold
    set fish_color_valid_path       --underline
    set fish_color_autosuggestion   6c7086
    set fish_color_user             f5c2e7
    set fish_color_host             89dceb
    set fish_color_cancel           f38ba8 --reverse
    set fish_color_search_match     --background=45475a
    set fish_color_selection        --background=585b70
    set fish_color_history_current  --bold
    set fish_color_operator         fab387 --bold
    set fish_color_escape           89dceb --bold
    set fish_color_cwd              89b4fa
    set fish_color_cwd_root         f38ba8
    set fish_color_match            --background=45475a

    set fish_pager_color_prefix             74c7ec --bold
    set fish_pager_color_completion         cdd6f4
    set fish_pager_color_description        6c7086
    set fish_pager_color_progress           1e1e2e --background=74c7ec
    set fish_pager_color_secondary_prefix   45475a
    set fish_pager_color_selected_prefix    1e1e2e --background=cba6f7
    set fish_pager_color_selected_completion cdd6f4 --background=45475a
    set fish_pager_color_selected_description f9e2af --background=45475a

    # ─────────────────────────────────────────────────────────────────
    # HISTORY CONFIGURATION
    # ─────────────────────────────────────────────────────────────────

    set -g fish_history_size 10000
    set -U fish_history_max_entries 10000

    # ─────────────────────────────────────────────────────────────────
    # KEY BINDINGS
    # ─────────────────────────────────────────────────────────────────

    function fish_user_key_bindings
        bind \cu backward-kill-line
        bind \e\[1\;5C forward-word
        bind \e\[1\;5D backward-word
        bind \e\[H beginning-of-line
        bind \e\[F end-of-line
        bind \e\[5~ history-search-backward
        bind \e\[6~ history-search-forward
        bind \e\[3~ delete-char
        bind \cf accept-autosuggestion
        bind \e\r 'commandline -i \n'
    end

    # ─────────────────────────────────────────────────────────────────
    # ALIASES
    # ─────────────────────────────────────────────────────────────────

    alias imlazy='sudo xbps-install -Syu'

    function ls
        if command -v lsd >/dev/null
            lsd --color=auto $argv
        else
            command ls --color=auto $argv
        end
    end

    function ll
        if command -v lsd >/dev/null
            lsd -l $argv
        else
            command ls -l --color=auto $argv
        end
    end

    function la
        if command -v lsd >/dev/null
            lsd -A $argv
        else
            command ls -A --color=auto $argv
        end
    end

    function lah
        if command -v lsd >/dev/null
            lsd -lah $argv
        else
            command ls -lah --color=auto $argv
        end
    end

    function l
        if command -v lsd >/dev/null
            lsd -CF $argv
        else
            command ls -CF --color=auto $argv
        end
    end

    alias ..='cd ..'
    alias ...='cd ../..'
    alias ....='cd ../../..'
    alias .....='cd ../../../..'

    alias dl='cd ~/Downloads'
    alias doc='cd ~/Documents'
    alias dt='cd ~/Desktop'

    alias g='git'

    alias grep='grep --color=auto'
    alias fgrep='fgrep --color=auto'
    alias egrep='egrep --color=auto'
    alias diff='diff --color=auto'
    alias ip='ip --color=auto'

    # ─────────────────────────────────────────────────────────────────
    # FUNCTIONS
    # ─────────────────────────────────────────────────────────────────

    function reload
        source ~/.config/fish/config.fish
        echo "Fish configuration reloaded!"
    end

    function mkcd
        if test (count $argv) -eq 0
            echo "Usage: mkcd <dir>"
            return 1
        end
        mkdir -p $argv[1]; and cd $argv[1]
    end

    function cd
        builtin cd $argv
        and ls -A
    end

    function extract --description "Extract archives"
        if test (count $argv) -eq 0
            echo "Usage: extract <archive> [archive2 ...]"
            return 1
        end
        for f in $argv
            switch $f
                case '*.tar.gz' '*.tgz'
                    tar xzf $f
                case '*.tar.bz2' '*.tbz2'
                    tar xjf $f
                case '*.tar.xz' '*.txz'
                    tar xJf $f
                case '*.zip'
                    unzip $f
                case '*.rar'
                    if type -q unrar
                        unrar x $f
                    else
                        echo "[!] unrar not installed"
                    end
                case '*'
                    echo "[x] Don't know how to extract: $f"
            end
        end
    end

    function serve --description "Start HTTP server on port (default 8000)"
        set port 8000
        if test (count $argv) -ge 1
            set port $argv[1]
        end
        echo "[*] Serving on http://localhost:$port"
        python3 -m http.server $port
    end

    function gitlog --description "Pretty git log"
        git log --oneline --graph --decorate --all
    end

    function sysinfo --description "Show system info"
        echo "─────────────────────────────────────────────"
        uname -a
        echo "─────────────────────────────────────────────"
    end

    function killp --description "Kill process by name"
        if test (count $argv) -eq 0
            echo "Usage: killp <process_name>"
            return 1
        end
        pkill -f $argv[1]
        echo "[+] Killed processes matching: $argv[1]"
    end

    function memtop --description "Show top memory consumers"
        echo "─────────────────────────────────────────────"
        echo "Top 10 Memory Consumers:"
        echo "─────────────────────────────────────────────"
        ps aux --sort=-%mem | head -11
        echo ""
        free -h | grep Mem
    end

    function diskuse --description "Show largest directories"
        echo "Largest directories in home:"
        du -sh ~/* 2>/dev/null | sort -hr | head -10
    end

    # ─────────────────────────────────────────────────────────────────
    # ABBREVIATIONS
    # ─────────────────────────────────────────────────────────────────

    abbr -e -- c 2>/dev/null; abbr -a c clear
    abbr -e -- cls 2>/dev/null; abbr -a cls clear
    abbr -e -- ..  cd ..
    abbr -e -- ... cd ../..
    abbr -e -- .... cd ../../..
    abbr -e -- update 'sudo xbps-install -Syu'
    abbr -e -- ls 'ls --color=auto -A'
    abbr -e -- ll 'ls -lh --color=auto -A'
    abbr -e -- la 'ls -lah --color=auto -A'
    abbr -e -- grep 'grep --color=auto'
    abbr -e -- diff 'diff --color=auto'
    abbr -e -- sudo 'sudo '
    abbr -e -- g git
    abbr -e -- ga 'git add'
    abbr -e -- gc 'git commit'
    abbr -e -- gp 'git push'
    abbr -e -- gs 'git status'
    abbr -e -- gl 'git log --oneline'

    # ─────────────────────────────────────────────────────────────────
    # SAFE PATH ADJUSTMENTS
    # ─────────────────────────────────────────────────────────────────

    if not contains /usr/local/bin $fish_user_paths
        set -U fish_user_paths /usr/local/bin $fish_user_paths
    end

    # ─────────────────────────────────────────────────────────────────
    # ENVIRONMENT VARIABLES
    # ─────────────────────────────────────────────────────────────────

    if not set -q EDITOR
        set -x EDITOR vi
    else
        set -x EDITOR $EDITOR
    end
    set -x VISUAL $EDITOR

    if not set -q PAGER
        set -x PAGER less
    end

    set -x LESS '-R --use-color -Dd+r -Du+b'
    umask 022

    # ─────────────────────────────────────────────────────────────────
    # EXTERNAL TOOLS INTEGRATION
    # ─────────────────────────────────────────────────────────────────

    if command -v zoxide >/dev/null
        zoxide init fish | source
    end

    # ─────────────────────────────────────────────────────────────────
    # COMPLETION ENHANCEMENTS
    # ─────────────────────────────────────────────────────────────────

    set -g fish_complete_case_insensitive 1
    set -g fish_complete_path_ambiguous_dirs false

    set -g __fish_git_prompt_show_informative_status 1
    set -g __fish_git_prompt_showdirtystate 1
    set -g __fish_git_prompt_showuntrackedfiles 1
    set -g __fish_git_prompt_showupstream auto

end

set -gx LC_ALL en_US.UTF-8
