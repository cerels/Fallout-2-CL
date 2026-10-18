#!/bin/bash
# Starts the Fallout 2 CE launcher. For Steam, set the game's launch options to:
#   "/path/to/Fallout 2/launcher/launch.sh" # %command%
# The "#" drops Steam's own command. --ui / --no-ui are passed on to the launcher.
dir="$(dirname "$(readlink -f "$0")")"

args=()
for a in "$@"; do
    [[ "$a" == --ui || "$a" == --no-ui ]] && args+=("$a")
done

if command -v python3 >/dev/null; then
    exec python3 -I "$dir/f2launcher.py" "${args[@]}"
fi

# No Python: start the game directly.
cd "$dir/.." || exit 1
exec ./fallout2-ce
