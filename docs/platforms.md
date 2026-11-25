# Native service support (development branch)

Windows behavior remains unchanged. Linux has an initial systemd **user-service** backend; macOS and system-wide registration are not implemented yet.

```sh
shawl add --name my-app --cwd /home/me/my-app -- /usr/bin/node dist/index.js
systemctl --user start my-app.service
systemctl --user enable my-app.service
systemctl --user status my-app.service
journalctl --user -u my-app.service
systemctl --user disable --now my-app.service
```

Use an active systemd user manager. Registration creates a new unit under
`$XDG_CONFIG_HOME/systemd/user` (default `~/.config/systemd/user`) and reloads
the manager. It verifies that the loaded fragment matches the generated file.
It does not start or enable the unit. To remove it after
disabling/stopping, remove the generated file and run `systemctl --user daemon-reload`.
User services start with the user manager; boot without login requires appropriate
administrator-configured lingering. Do not use sudo for user registration.

Supported: absolute executable path, arguments, working directory, --env,
--restart, --no-restart, --restart-delay, --stop-timeout, --no-log and
--no-log-cmd. systemd always manages the process cgroup (including descendants).
The stop signal is SIGINT; restart defaults to on-failure, subject to systemd's
own signal classification and start-rate limits. Logs go to the journal unless
disabled. The `shawl run` subcommand is not a Linux entry point. Use systemctl to run the service.

Not supported yet: exit-code lists, --pass-start-args, dependencies, PATH options,
priority, interactive mode, Shawl-specific log files/rotation, system-wide
registration and macOS. Unsupported options fail explicitly. Control characters
in unit values are rejected. No implicit shell invocation is performed.
Environment values are stored in the unit file (mode 0600 on creation) and can
be exposed by systemd; do not use --env for credentials.
This is an initial implementation; native service integration tests remain pending.
