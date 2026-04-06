#!/bin/bash

# Updates checker script for waybar (Void Linux / xbps)

check_updates() {
    UPDATES=$(xbps-install -Mun 2>/dev/null | wc -l)

    if [ "$UPDATES" -eq 0 ]; then
        echo "{\"text\":\"\",\"class\":\"up-to-date\",\"tooltip\":\"System is up to date\"}"
    else
        echo "{\"text\":\"󰏗 $UPDATES\",\"class\":\"has-updates\",\"tooltip\":\"$UPDATES update(s) available\"}"
    fi
}

check_updates
