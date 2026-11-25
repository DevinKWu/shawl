# Native service support (development branch)

Windows behavior remains unchanged. Linux uses systemd, macOS uses launchd.
Shawl registers a native service definition and exits; no extra persistent
supervisor is introduced. Native manager commands control the service.
All Unix backends require an absolute executable path. These implementations
are experimental until native integration and three-platform build tests pass.

## Linux (systemd)

User services (do not use sudo):

```sh
shawl add --name my-app --cwd /home/me/my-app -- /usr/bin/node dist/index.js
systemctl --user start my-app.service
systemctl --user enable my-app.service
systemctl --user status my-app.service
journalctl --user -u my-app.service
systemctl --user disable --now my-app.service
```

System services require root and an explicit existing non-root account:

```sh
sudo /usr/local/bin/shawl add --name my-app --system --service-user appuser --cwd /srv/my-app -- /usr/bin/node dist/index.js
sudo systemctl start my-app.service
sudo systemctl enable my-app.service
sudo systemctl status my-app.service
sudo journalctl -u my-app.service
sudo systemctl disable --now my-app.service
```

Use an active systemd manager. User units are installed under
`$XDG_CONFIG_HOME/systemd/user` (default `~/.config/systemd/user`); system
units under `/etc/systemd/system`. Shawl refuses existing units, reloads
the manager, verifies the loaded fragment and cleans up failed registration.
It does not start or enable the unit. To remove, stop and disable it first,
remove only the generated file, and run `systemctl daemon-reload` (add
`--user` for user units). User services require administrator-configured
lingering if they must start at boot without a login.

## macOS (launchd)

User services use the current GUI login domain. Registration writes a
LaunchAgent plist, without bootstrapping the current session. The label is
`org.shawl.<name>`. Jobs in the standard directory load on subsequent login;
the generated job uses RunAtLoad, so bootstrap starts it immediately.

```sh
shawl add --name my-app --cwd /Users/me/my-app -- /opt/homebrew/bin/node dist/index.js
launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/org.shawl.my-app.plist"
launchctl print "gui/$(id -u)/org.shawl.my-app"
launchctl kickstart -k "gui/$(id -u)/org.shawl.my-app"
launchctl bootout "gui/$(id -u)/org.shawl.my-app"
```

System services require root and a non-root account. The first version
requires disabled captured logs for system jobs to avoid unsafe privileged
log paths. Configure application logging separately.

```sh
sudo /usr/local/bin/shawl add --system --service-user appuser --no-log --name my-app -- /usr/local/bin/my-app
sudo launchctl bootstrap system /Library/LaunchDaemons/org.shawl.my-app.plist
sudo launchctl print system/org.shawl.my-app
sudo launchctl bootout system/org.shawl.my-app
```

Bootout unloads a job; it may load again at login/boot if its plist remains.
To prevent automatic loading, unload and remove only the generated plist,
or explicitly disable the job with the native manager. To start again after
bootout, bootstrap its file. Kickstart -k restarts a loaded job.
Registration checks the domain and loaded label, refuses existing files and
validates the generated plist with plutil. An unloaded conflicting label
in a different file is not discoverable from launchctl print; administrators
must avoid such collisions.

## Supported options and native differences

| Option | Linux | macOS |
| --- | --- | --- |
| name, cwd, command, env | supported | supported |
| --system --service-user | root registration; non-root service | root registration; non-root service |
| --restart / --no-restart | Restart=always/no | KeepAlive=true/false |
| default restart | on-failure, systemd signal classification | KeepAlive SuccessfulExit=false |
| --restart-delay | RestartSec in ms | ThrottleInterval in seconds, rounds up; not exact delay |
| --stop-timeout | positive ms; SIGINT then kill | positive ms rounded up; SIGTERM then kill |
| --kill-process-tree | native cgroup cleanup always | rejected; launchd default process-group cleanup is not full tree tracking |
| --no-log / --no-log-cmd | discard stdout/stderr | discard stdout/stderr |
| --log-dir | rejected; journal | existing user log directory; default ~/Library/Logs |
| custom filenames/rotation | rejected | rejected |

macOS logs are plain stdout/stderr files without Shawl rotation, so configure
retention externally. Restart behavior follows each manager's throttling,
signal handling and rate limits. Programs should stay in the foreground.
Other unsupported options: exit-code lists, dependencies, pass-start-args,
PATH append/prepend, priority and interactive mode. Unsupported options fail
explicitly. No shell is implicitly invoked. Control characters are rejected.
The `shawl run` subcommand is Windows-only; use native commands elsewhere.

Environment values and arguments may be exposed by native managers even
though files are created with mode 0600. Never pass credentials through
--env or command arguments; use application-managed credentials instead.

## Validation

Run `cargo test --locked` on each target. On Linux and macOS, run
`cargo test --locked --bin shawl generated_native_config -- --ignored` to
check the actual generated definition with systemd-analyze or plutil.
These tests do not register or start services; they are not lifecycle tests.
Before release, separately exercise user/system registration, service
identity, exact argv/env/cwd, start and stop, normal/failing exit, restart,
descendant cleanup, duplicate names, permissions, and reboot/login
persistence in disposable native test environments.
