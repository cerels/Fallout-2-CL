#!/bin/bash
# Starts the Fallout 2 CE launcher. For Steam, set the game's launch options to:
#   "/path/to/Fallout 2/launcher/launch.sh" # %command%
# The "#" drops Steam's own command. --ui / --no-ui / --campaign=<id> are passed on to
# the launcher - the last one is how a different Steam entry (e.g. Fallout 1's) forces
# a specific campaign; see backend/launch_option.py.
dir="$(dirname "$(readlink -f "$0")")"

args=()
for a in "$@"; do
    [[ "$a" == --ui || "$a" == --no-ui || "$a" == --campaign=* ]] && args+=("$a")
done

if command -v python3 >/dev/null; then
    exec python3 -I "$dir/f2launcher.py" "${args[@]}"
fi

# No Python: start the game directly.
cd "$dir/.." || exit 1
exec ./fallout2-ce
