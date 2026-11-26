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
os=$(uname -s)
logging=()
case $os in
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
        if [[ $scope == system ]]; then
            logs=(sudo -n journalctl --no-pager --output=cat --unit "$target")
        else
            logs=(journalctl --user --no-pager --output=cat --unit "$target")
        fi
        start=("${manager[@]}" start "$target")
        stop=("${manager[@]}" stop "$target")
        inspect=("${manager[@]}" status --no-pager "$target")
        ;;
    Darwin)
        label=org.shawl.$name
        if [[ $scope == system ]]; then
            domain=system
            definition=/Library/LaunchDaemons/$label.plist
            logging=(--no-log)
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
        if [[ $os == Linux ]] && ! "${manager[@]}" daemon-reload; then
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

cwd="$work/working & % directory"
program="$work/sh & % executable"
mkdir "$cwd"
ln -s /bin/sh "$program"
if [[ $os == Darwin && $scope == user ]]; then
    logging=(--log-dir "$work")
fi

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
if [ "$SHAWL_SMOKE_OS" = Linux ]; then
    sleep 600 &
    printf '%s\n' "$!" >> "$SHAWL_SMOKE_DIR/children"
fi
printf 'shawl-smoke stdout\n'
printf 'shawl-smoke stderr\n' >&2
printf '%s\n' "$$" >> "$SHAWL_SMOKE_DIR/starts"
while :; do sleep 1; done
FIXTURE

add=("${admin[@]}" "$shawl" add --name "$name" "${registration[@]}"
    --cwd "$cwd" --restart --restart-delay 1000 --stop-timeout 5000 "${logging[@]}"
    --env 'SHAWL_SMOKE_ENV=literal $HOME 100%'
    --env "SHAWL_SMOKE_UID=$uid" --env "SHAWL_SMOKE_DIR=$cwd" --env "SHAWL_SMOKE_OS=$os"
    -- "$program" "$work/fixture.sh" 'space & % $HOME' '' 'quote " and \backslash')
if [[ $os == Linux && $scope == user ]]; then
    # Force the client to write outside the active manager's unit search path.
    invalid_config="$work/unloaded-config"
    if XDG_CONFIG_HOME="$invalid_config" "${add[@]}"; then
        echo 'Registration outside the systemd search path unexpectedly succeeded' >&2
        exit 1
    fi
    [[ ! -e $invalid_config/systemd/user/$target ]]
fi
"${add[@]}"
registered=1
if [[ $os == Linux ]]; then
    [[ $(stat -c %a "$definition") == 600 ]]
else
    [[ $(stat -f %Lp "$definition") == 600 ]]
fi

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
wait_lines "$cwd/starts" 1
first_pid=$(head -n 1 "$cwd/starts")
[[ $first_pid =~ ^[1-9][0-9]*$ ]]
if "${add[@]}"; then
    echo 'Duplicate registration unexpectedly succeeded' >&2
    exit 1
fi
# The fixture runs as the invoking account, so no privileged kill is needed.
kill -KILL "$first_pid"
wait_lines "$cwd/starts" 2
second_pid=$(tail -n 1 "$cwd/starts")
[[ $second_pid =~ ^[1-9][0-9]*$ && $second_pid != "$first_pid" ]]
"${stop[@]}"
stopped=1
wait_lines "$cwd/stops" 1
grep -Fxq "$second_pid" "$cwd/stops"
# A manager-initiated stop must not restart the fixture.
sleep 2
[[ $(wc -l < "$cwd/starts") -eq 2 ]]
if [[ $os == Linux ]]; then
    journal=$("${logs[@]}")
    grep -Fxq 'shawl-smoke stdout' <<< "$journal"
    grep -Fxq 'shawl-smoke stderr' <<< "$journal"
    while read -r pid; do
        if kill -0 "$pid" 2>/dev/null; then
            echo "systemd left child $pid running after stop" >&2
            exit 1
        fi
    done < "$cwd/children"
elif [[ $scope == user ]]; then
    grep -Fxq 'shawl-smoke stdout' "$work/$label.stdout.log"
    grep -Fxq 'shawl-smoke stderr' "$work/$label.stderr.log"
fi
echo "PASS: $os $scope registration, identity, argv/env/cwd, duplicate, restart and graceful stop"
