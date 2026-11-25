//! Native launchd job registration. launchd, not Shawl, supervises the process.
use crate::{
    cli::CommonOpts,
    native::{self, Result, Scope},
};
use std::{collections::BTreeMap, fs, path::{Path, PathBuf}, process::Command};

fn xml(value: &str) -> Result<String> {
    // Reject control characters (including CR, which XML normalizes) and
    // invalid XML scalars instead of silently changing an argument.
    if value.chars().any(|c| c.is_control() || c == '\u{fffe}' || c == '\u{ffff}') {
        return Err("Control characters or invalid XML characters are not supported in launchd values".into());
    }
    Ok(value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        .replace('"', "&quot;").replace('\'', "&apos;"))
}

fn label(name: &str) -> Result<String> {
    if !native::valid_name(name) || name.ends_with(".plist") {
        return Err("Invalid service name (letters, digits, '.', '_' and '-' only; no .plist suffix)".into());
    }
    Ok(format!("org.shawl.{name}"))
}

fn seconds(ms: u64) -> Result<u64> {
    if ms == 0 {
        return Err("launchd timeout and throttle intervals must be positive".into());
    }
    Ok(ms.div_ceil(1000))
}

fn render_plist(
    name: &str,
    cwd: Option<&str>,
    dependencies: &[String],
    opts: &CommonOpts,
    scope: Scope<'_>,
    log_dir: Option<&Path>,
) -> Result<String> {
    let label = label(name)?;
    if !dependencies.is_empty()
        || opts.pass.is_some()
        || !opts.restart_if.is_empty()
        || !opts.restart_if_not.is_empty()
        || opts.pass_start_args
        || !opts.path.is_empty()
        || !opts.path_prepend.is_empty()
        || opts.priority.is_some()
        || opts.interactive
        || opts.kill_process_tree
        || opts.log_as.is_some()
        || opts.log_cmd_as.is_some()
        || opts.log_rotate.is_some()
        || opts.log_retain.is_some()
    {
        return Err("This option is not supported by the initial launchd backend; see docs/platforms.md".into());
    }
    let program = opts.command.first().ok_or("Missing executable")?;
    if !Path::new(program).is_absolute() || !Path::new(program).is_file() {
        return Err("The service executable must be an existing absolute file path".into());
    }

    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n");
    out.push_str(&format!("  <key>Label</key><string>{label}</string>\n"));
    out.push_str("  <key>ProgramArguments</key><array>\n");
    for arg in &opts.command {
        out.push_str(&format!("    <string>{}</string>\n", xml(arg)?));
    }
    out.push_str("  </array>\n");
    // Files in standard launchd directories load on login/boot. Registration
    // itself does not bootstrap the job into the current domain.
    out.push_str("  <key>RunAtLoad</key><true/>\n");
    out.push_str(if opts.restart {
        "  <key>KeepAlive</key><true/>\n"
    } else if opts.no_restart {
        "  <key>KeepAlive</key><false/>\n"
    } else {
        "  <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>\n"
    });
    out.push_str(&format!("  <key>ExitTimeOut</key><integer>{}</integer>\n", seconds(opts.stop_timeout.unwrap_or(3000))?));
    if let Some(delay) = opts.restart_delay {
        out.push_str(&format!("  <key>ThrottleInterval</key><integer>{}</integer>\n", seconds(delay)?));
    }
    if let Scope::System(user) = scope {
        out.push_str(&format!("  <key>UserName</key><string>{}</string>\n", xml(user)?));
    }
    if let Some(cwd) = cwd {
        if !Path::new(cwd).is_absolute() || !Path::new(cwd).is_dir() {
            return Err("Working directory must be an existing absolute directory".into());
        }
        out.push_str(&format!("  <key>WorkingDirectory</key><string>{}</string>\n", xml(cwd)?));
    }
    let mut env = BTreeMap::new();
    for (key, value) in &opts.env {
        if !native::valid_env_key(key) {
            return Err("Invalid environment variable name".into());
        }
        env.insert(key, value);
    }
    if !env.is_empty() {
        out.push_str("  <key>EnvironmentVariables</key><dict>\n");
        for (key, value) in env {
            out.push_str(&format!("    <key>{}</key><string>{}</string>\n", xml(key)?, xml(value)?));
        }
        out.push_str("  </dict>\n");
    }

    let (stdout, stderr) = if opts.no_log || opts.no_log_cmd {
        if opts.log_dir.is_some() {
            return Err("--log-dir cannot be combined with disabled logging on macOS".into());
        }
        (PathBuf::from("/dev/null"), PathBuf::from("/dev/null"))
    } else {
        if matches!(scope, Scope::System(_)) {
            return Err("System launchd jobs currently require --no-log or --no-log-cmd; configure application logging separately".into());
        }
        let dir = log_dir.ok_or("Missing user log directory")?;
        if !dir.is_absolute() || !dir.is_dir() {
            return Err("Log directory must be an existing absolute directory".into());
        }
        (dir.join(format!("{label}.stdout.log")), dir.join(format!("{label}.stderr.log")))
    };
    for (key, path) in [("StandardOutPath", stdout), ("StandardErrorPath", stderr)] {
        let path = path.to_str().ok_or("Log path must be valid UTF-8")?;
        out.push_str(&format!("  <key>{key}</key><string>{}</string>\n", xml(path)?));
    }
    out.push_str("</dict>\n</plist>\n");
    Ok(out)
}

pub fn add_service(name: &str, cwd: Option<&str>, dependencies: &[String], opts: &CommonOpts) -> Result<()> {
    let scope = native::scope(opts)?;
    let label = label(name)?;
    let (domain, dir, log_dir) = match scope {
        Scope::User => {
            let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?);
            if !home.is_absolute() {
                return Err("HOME must be absolute".into());
            }
            let log_dir = opts.log_dir.as_ref().map(PathBuf::from).unwrap_or_else(|| home.join("Library/Logs"));
            (format!("gui/{}", native::current_uid()?), home.join("Library/LaunchAgents"), Some(log_dir))
        }
        Scope::System(_) => ("system".to_string(), PathBuf::from("/Library/LaunchDaemons"), None),
    };
    let plist = render_plist(name, cwd, dependencies, opts, scope, log_dir.as_deref())?;
    let target = format!("{domain}/{label}");
    if !Command::new("/bin/launchctl").args(["print", &domain]).output()?.status.success() {
        return Err("The launchd domain is unavailable; user registration requires a GUI login session".into());
    }
    if Command::new("/bin/launchctl").args(["print", &target]).output()?.status.success() {
        return Err("A job with this label is already loaded".into());
    }

    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{label}.plist"));
    native::write_new(&path, &plist)?;
    let validation = Command::new("/usr/bin/plutil").arg("-lint").arg("--").arg(&path).output();
    match validation {
        Ok(result) if result.status.success() => (),
        _ => {
            fs::remove_file(&path)?;
            return Err("plutil validation failed; generated plist removed".into());
        }
    }
    println!("Registered {} (loads at next login/boot). To load now, run launchctl bootstrap {domain} with this plist path.", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> CommonOpts {
        CommonOpts {
            command: vec!["/bin/sh".into(), "a b".into(), "a<&>\"'".into(), "".into()],
            ..Default::default()
        }
    }

    #[test]
    fn preserves_arguments_and_escapes_xml() {
        let mut options = opts();
        options.env = vec![("KEY".into(), "first".into()), ("KEY".into(), "<last>&".into())];
        let plist = render_plist("test", Some("/tmp"), &[], &options, Scope::User, Some(Path::new("/tmp"))).unwrap();
        assert!(plist.contains("<string>a b</string>"));
        assert!(plist.contains("<string>a&lt;&amp;&gt;&quot;&apos;</string>"));
        assert!(plist.contains("<string></string>"));
        assert!(plist.contains("<key>KEY</key><string>&lt;last&gt;&amp;</string>"));
        assert!(!plist.contains("first"));
        assert!(plist.contains("<key>SuccessfulExit</key><false/>"));
    }

    #[test]
    fn system_jobs_require_non_root_scope_and_explicit_no_logging() {
        let mut options = opts();
        assert!(render_plist("test", None, &[], &options, Scope::System("app"), None).is_err());
        options.no_log = true;
        options.no_restart = true;
        options.stop_timeout = Some(1500);
        options.restart_delay = Some(500);
        let plist = render_plist("test", None, &[], &options, Scope::System("app"), None).unwrap();
        assert!(plist.contains("<key>UserName</key><string>app</string>"));
        assert!(plist.contains("<key>KeepAlive</key><false/>"));
        assert!(plist.contains("<key>ExitTimeOut</key><integer>2</integer>"));
        assert!(plist.contains("<key>ThrottleInterval</key><integer>1</integer>"));
        assert!(plist.contains("<key>StandardOutPath</key><string>/dev/null</string>"));
    }

    #[test]
    fn rejects_unsafe_or_unsupported_values() {
        let mut options = opts();
        for name in ["../evil", "-bad", "a/b", "foo.plist"] {
            assert!(render_plist(name, None, &[], &options, Scope::User, Some(Path::new("/tmp"))).is_err());
        }
        options.kill_process_tree = true;
        assert!(render_plist("test", None, &[], &options, Scope::User, Some(Path::new("/tmp"))).is_err());
        options.kill_process_tree = false;
        options.command.push("bad\rvalue".into());
        assert!(render_plist("test", None, &[], &options, Scope::User, Some(Path::new("/tmp"))).is_err());
    }
}
