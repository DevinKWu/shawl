//! Small shared helpers for native Unix service registration.
use crate::cli::CommonOpts;
use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
pub enum Scope<'a> {
    User,
    System(&'a str),
}

fn valid_user(name: &str) -> bool {
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(b) if b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn uid_for(user: Option<&str>) -> Result<u32> {
    // Do not search PATH when a system service is being registered as root.
    let mut command = Command::new("/usr/bin/id");
    command.arg("-u");
    if let Some(user) = user {
        command.arg(user);
    }
    let result = command.output()?;
    if !result.status.success() {
        return Err("Unable to resolve account UID".into());
    }
    Ok(String::from_utf8(result.stdout)?.trim().parse()?)
}

pub fn current_uid() -> Result<u32> {
    uid_for(None)
}

pub fn scope(opts: &CommonOpts) -> Result<Scope<'_>> {
    let uid = current_uid()?;
    match (opts.system, opts.service_user.as_deref()) {
        (false, None) if uid != 0 => Ok(Scope::User),
        (false, None) => {
            Err("Do not register user services as root; use --system --service-user ACCOUNT for system services".into())
        }
        (true, Some(user)) if uid == 0 => {
            if !valid_user(user) || uid_for(Some(user))? == 0 {
                return Err("System services require an existing non-root account name".into());
            }
            Ok(Scope::System(user))
        }
        (true, Some(_)) => Err("System registration requires root".into()),
        _ => Err("--system and --service-user must be used together".into()),
    }
}

// Stage in a private directory on the same filesystem. Publishing by
// hard_link is atomic and never overwrites an existing destination, even
// when two registrations race or the destination is a symlink.
pub fn write_new_checked(path: &Path, contents: &str, validate: impl FnOnce(&Path) -> Result<()>) -> Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().ok_or("Missing service directory")?;
    let name = path.file_name().ok_or("Missing service filename")?;
    let stage = parent.join(format!(
        ".shawl-stage-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    DirBuilder::new().mode(0o700).create(&stage)?;
    let staged_path = stage.join(name);
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged_path)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        drop(file);
        validate(&staged_path)?;
        fs::hard_link(&staged_path, path)?;
        Ok(())
    })();

    let cleanup = fs::remove_dir_all(&stage);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Ok(()), Err(error)) => {
            eprintln!("Service file published, but staging directory cleanup failed: {error}");
            Ok(())
        }
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(format!("Service publication failed ({error}); staging cleanup failed ({cleanup})").into())
        }
    }
}

#[cfg(any(target_os = "linux", test))]
pub fn write_new(path: &Path, contents: &str) -> Result<()> {
    write_new_checked(path, contents, |_| Ok(()))
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

pub fn valid_env_key(key: &str) -> bool {
    let mut bytes = key.bytes();
    matches!(bytes.next(), Some(b) if b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_user_names() {
        for name in ["app", "_service", "app-2"] {
            assert!(valid_user(name));
        }
        for name in ["", "-u", "0", "a\nUser=root", "a:b", "../root"] {
            assert!(!valid_user(name));
        }
    }

    #[test]
    fn refuses_to_overwrite_existing_files() {
        let dir = std::env::temp_dir().join(format!("shawl-write-new-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("service");
        write_new(&path, "original").unwrap();
        assert!(write_new(&path, "replacement").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn validation_failure_never_publishes_file() {
        let dir = std::env::temp_dir().join(format!("shawl-stage-test-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("service");
        let result = write_new_checked(&path, "invalid", |staged| {
            assert_eq!(fs::read_to_string(staged)?, "invalid");
            assert!(!path.exists());
            Err("validation failed".into())
        });
        assert!(result.is_err());
        assert!(!path.exists());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn refuses_to_replace_symlink() {
        let dir = std::env::temp_dir().join(format!("shawl-symlink-test-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("service");
        let original = dir.join("original");
        fs::write(&original, "original").unwrap();
        std::os::unix::fs::symlink(&original, &path).unwrap();
        assert!(write_new(&path, "replacement").is_err());
        assert!(fs::symlink_metadata(&path).unwrap().is_symlink());
        assert_eq!(fs::read_to_string(&original).unwrap(), "original");
        fs::remove_dir_all(dir).unwrap();
    }
}
