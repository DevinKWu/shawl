#!/usr/bin/env bash
# Opt-in integration test. Run as a non-root user in a disposable native VM.
set -eo pipefail
# macOS ships Bash 3.2, which treats empty arrays as unset with nounset.

if [[ $# != 2 || ( $1 != user && $1 != system ) ]]; then
    echo "Usage: bash tests/native-smoke.sh user|system /absolute/path/to/shawl" >&2
    exit 2
fi
scope=$1
shawl=$2
[[ $shawl == /* && -x $shawl ]] || { echo 'Shawl must be an absolute executable path' >&2; exit 2; }
uid=$(id -u)
account=$(id -un)
[[ $uid != 0 ]] || { echo 'Run as a non-root user, even for system scope' >&2; exit 2; }
admin=()
registration=()
if [[ $scope == system ]]; then
    sudo -n true
    admin=(sudo -n)
    registration=(--system --service-user "$account")
fi

name="shawl-smoke-${uid}-$$-$(date +%s)"
case $(uname -s) in
    Linux)
        if [[ $scope == system ]]; then
            manager=(sudo -n systemctl --system)
            unit_dir=/etc/systemd/system
        else
            manager=(systemctl --user)
            unit_dir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
        fi
        target=$name.service
        definition=$unit_dir/$target
        start=("${manager[@]}" start "$target")
        stop=("${manager[@]}" stop "$target")
        inspect=("${manager[@]}" status --no-pager "$target")
        ;;
    Darwin)
        label=org.shawl.$name
        if [[ $scope == system ]]; then
            domain=system
            definition=/Library/LaunchDaemons/$label.plist
        else
            domain=gui/$uid
            definition=$HOME/Library/LaunchAgents/$label.plist
        fi
        target=$domain/$label
        start=("${admin[@]}" /bin/launchctl bootstrap "$domain" "$definition")
        stop=("${admin[@]}" /bin/launchctl bootout "$target")
        inspect=("${admin[@]}" /bin/launchctl print "$target")
        ;;
    *) echo 'Requires Linux/systemd or macOS/launchd' >&2; exit 2 ;;
esac

work=$(mktemp -d "${TMPDIR:-/tmp}/shawl-smoke.XXXXXX")
work=$(cd "$work" && pwd -P)
registered=0
stopped=0
cleanup() {
    local result=$?
    trap - EXIT
    set +e
    if (( result != 0 && registered )); then
        "${inspect[@]}" >&2
    fi
    if (( registered )); then
        # Test never enables a systemd service. Do not delete the fixture
        # when we cannot establish that the native manager stopped the job.
        if (( ! stopped )) && ! "${stop[@]}"; then
            echo "Cleanup failed to stop/unload $target; leaving definition and fixture at $work" >&2
            exit 1
        fi
        if ! "${admin[@]}" rm -- "$definition"; then
            echo "Cleanup failed: $definition; fixture retained at $work" >&2
            exit 1
        fi
        if [[ $(uname -s) == Linux ]] && ! "${manager[@]}" daemon-reload; then
            echo 'Cleanup daemon-reload failed' >&2
            exit 1
        fi
    fi
    rm -rf -- "$work" || result=1
    exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

cat > "$work/fixture.sh" <<'FIXTURE'
#!/bin/sh
set -eu
[ "$#" -eq 3 ]
[ "$1" = 'space & % $HOME' ]
[ "$2" = '' ]
[ "$3" = 'quote " and \backslash' ]
[ "$SHAWL_SMOKE_ENV" = 'literal $HOME 100%' ]
[ "$(id -u)" = "$SHAWL_SMOKE_UID" ]
[ "$(pwd -P)" = "$SHAWL_SMOKE_DIR" ]
trap 'printf "%s\n" "$$" >> "$SHAWL_SMOKE_DIR/stops"; exit 0' INT TERM
printf '%s\n' "$$" >> "$SHAWL_SMOKE_DIR/starts"
while :; do sleep 1; done
FIXTURE

add=("${admin[@]}" "$shawl" add --name "$name" "${registration[@]}"
    --cwd "$work" --restart --restart-delay 1000 --stop-timeout 5000 --no-log
    --env 'SHAWL_SMOKE_ENV=literal $HOME 100%'
    --env "SHAWL_SMOKE_UID=$uid" --env "SHAWL_SMOKE_DIR=$work"
    -- /bin/sh "$work/fixture.sh" 'space & % $HOME' '' 'quote " and \backslash')
"${add[@]}"
registered=1

wait_lines() {
    local file=$1 minimum=$2
    local i count
    for (( i=0; i<60; i++ )); do
        if [[ -f $file ]]; then
            count=$(wc -l < "$file")
            if (( count >= minimum )); then return 0; fi
        fi
        sleep 0.5
    done
    echo "Timed out waiting for $minimum lines in $file" >&2
    return 1
}

"${start[@]}"
wait_lines "$work/starts" 1
first_pid=$(head -n 1 "$work/starts")
[[ $first_pid =~ ^[1-9][0-9]*$ ]]
if "${add[@]}"; then
    echo 'Duplicate registration unexpectedly succeeded' >&2
    exit 1
fi
# The fixture runs as the invoking account, so no privileged kill is needed.
kill -KILL "$first_pid"
wait_lines "$work/starts" 2
second_pid=$(tail -n 1 "$work/starts")
[[ $second_pid =~ ^[1-9][0-9]*$ && $second_pid != "$first_pid" ]]
"${stop[@]}"
stopped=1
wait_lines "$work/stops" 1
grep -Fxq "$second_pid" "$work/stops"
# A manager-initiated stop must not restart the fixture.
sleep 2
[[ $(wc -l < "$work/starts") -eq 2 ]]
echo "PASS: $(uname -s) $scope registration, identity, argv/env/cwd, duplicate, restart and graceful stop"
