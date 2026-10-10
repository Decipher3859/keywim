use nix::unistd::{Gid, User, getgrouplist, setgid, setgroups, setuid};
use std::{
    ffi::CString,
    io::Error,
    os::unix::process::CommandExt,
    process::{Child, Command},
};

pub fn run(command: &str, user: &User) -> Result<Child, String> {
    let groups: Vec<Gid> = resolve_groups(user)?
        .into_iter()
        .map(Gid::from_raw)
        .collect();

    let uid = user.uid;
    let gid = user.gid;

    let mut cmd = Command::new(command);

    unsafe {
        cmd.pre_exec(move || {
            setgroups(&groups).map_err(Error::from)?;

            setgid(gid).map_err(Error::from)?;
            setuid(uid).map_err(Error::from)?;

            Ok(())
        });
    }
    cmd.current_dir(&user.dir)
        .env("HOME", &user.dir)
        .env("USER", &user.name)
        .env("LOGNAME", &user.name)
        .env("XDG_RUNTIME_DIR", format!("/run/user/{}", user.uid))
        .spawn()
        .map_err(|e| format!("Session start failed: {e}"))
}

pub fn resolve_user(username: &str) -> Result<User, String> {
    User::from_name(username)
        .map_err(|e| format!("User lookup failed: {e}"))?
        .ok_or_else(|| format!("User not found: {username}"))
}

pub fn resolve_groups(user: &User) -> Result<Vec<u32>, String> {
    let username =
        CString::new(user.name.as_str()).map_err(|e| format!("Invalid username: {e}"))?;

    let groups =
        getgrouplist(&username, user.gid).map_err(|e| format!("Group loopup failed: {e}"))?;

    Ok(groups.into_iter().map(|gid| gid.as_raw()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_current_user() {
        let username = std::env::var("USER").expect("USER is not set");

        let user = resolve_user(&username).expect("User lookup failed");

        assert_eq!(user.name, username);

        println!("UID: {}", user.uid);
        println!("GID: {}", user.gid);
        println!("Home: {}", user.dir.display());
    }

    #[test]
    fn resolve_current_user_groups() {
        let username = std::env::var("USER").expect("USER is not set");
        let user = resolve_user(&username).expect("User lookup failed");
        let groups = resolve_groups(&user).expect("Group lookup failed");

        println!("Groups: {groups:?}");

        assert!(!groups.is_empty());
        assert!(groups.contains(&user.gid.as_raw()));
    }

    #[test]
    fn run_as_current_user() {
        let username = std::env::var("USER").expect("USER is not set");
        let user = resolve_user(&username).expect("User lookup failed");
        let status = run("/usr/bin/id", &user).expect("Process start failed");

        assert!(status.success());
    }

    #[test]
    #[ignore = "Requires root privileges"]
    fn run_as_user_from_root() {
        assert!(nix::unistd::Uid::effective().is_root());

        let user = resolve_user("damian").unwrap();
        let status = run("/usr/bin/id", &user).unwrap();

        assert!(status.success());
    }
}
