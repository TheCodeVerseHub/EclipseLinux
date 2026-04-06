#!/usr/bin/env fish

# XDG base directories
if not set -q XDG_CONFIG_HOME
    set -gx XDG_CONFIG_HOME "$HOME/.config"
end

if not set -q XDG_DATA_HOME
    set -gx XDG_DATA_HOME "$HOME/.local/share"
end

if not set -q XDG_DATA_DIRS
    set -gx XDG_DATA_DIRS "$XDG_DATA_HOME:/usr/local/share:/usr/share"
end

if not set -q XDG_STATE_HOME
    set -gx XDG_STATE_HOME "$HOME/.local/state"
end

if not set -q XDG_CACHE_HOME
    set -gx XDG_CACHE_HOME "$HOME/.cache"
end

if not set -q XDG_DESKTOP_DIR
    set -gx XDG_DESKTOP_DIR "$HOME/Desktop"
end

if not set -q XDG_DOWNLOAD_DIR
    set -gx XDG_DOWNLOAD_DIR "$HOME/Downloads"
end

if not set -q XDG_DOCUMENTS_DIR
    set -gx XDG_DOCUMENTS_DIR "$HOME/Documents"
end

if not set -q XDG_PICTURES_DIR
    set -gx XDG_PICTURES_DIR "$HOME/Pictures"
end

if not set -q XDG_VIDEOS_DIR
    set -gx XDG_VIDEOS_DIR "$HOME/Videos"
end

if not set -q XDG_MUSIC_DIR
    set -gx XDG_MUSIC_DIR "$HOME/Music"
end

if not set -q LESSHISTFILE
    set -gx LESSHISTFILE "/tmp/less-hist"
end

if not contains $HOME/.local/bin $PATH
    set -gx PATH $HOME/.local/bin $PATH
end

if type -q starship
    if not set -q STARSHIP_CACHE
        set -gx STARSHIP_CACHE $XDG_CACHE_HOME/starship
    end
    if not set -q STARSHIP_CONFIG
        set -gx STARSHIP_CONFIG $XDG_CONFIG_HOME/prompt/starship.toml
    end
end

if functions -q bind_M_n_history
    bind_M_n_history
end

if not set -q fish_greeting
    set -g fish_greeting
end
