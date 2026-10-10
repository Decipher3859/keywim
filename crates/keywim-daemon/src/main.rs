mod auth;
mod session;

use nix::unistd::{Group, chown};
use std::{
    fs,
    io::{BufRead, BufReader, Error, Result, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::Path,
};

fn main() -> Result<()> {
    let socket_path = "/tmp/keywim-dev.sock";

    if Path::new(socket_path).exists() {
        fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;

    let group = Group::from_name("keywim-greeter")
        .map_err(Error::from)?
        .ok_or_else(|| Error::other("Group keywim-greeter missing"))?;

    chown(socket_path, None, Some(group.gid)).map_err(Error::from)?;
    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o660))?;
    println!("Keywim daemon listening on {socket_path}");

    for connection in listener.incoming() {
        let mut stream = connection?;
        println!("Greeter connected");

        let mut reader = BufReader::new(&mut stream);
        let mut command = String::new();

        reader.read_line(&mut command)?;

        if command.trim() == "AUTH" {
            let mut username = String::new();
            let mut password = String::new();
            let mut session = String::new();

            reader.read_line(&mut username)?;
            reader.read_line(&mut password)?;
            reader.read_line(&mut session)?;

            drop(reader);

            let mut started = false;
            let result = auth::run_session(
                username.trim_end_matches(['\r', '\n']),
                password.trim_end_matches(['\r', '\n']),
                session.trim_end_matches(['\r', '\n']),
                || {
                    started = true;
                    if let Err(e) = stream.write_all(b"OK\n") {
                        eprintln!("Greeter response failed: {e}");
                    }
                },
            );
            if let Err(e) = result {
                eprintln!("Session failed: {e}");
                if !started && let Err(e) = stream.write_all(b"DENIED\n") {
                    eprintln!("Greeter response failed: {e}");
                }
            }
            if started && let Err(e) = stream.write_all(b"SESSION_ENDED\n") {
                eprintln!("Greeter response failed: {e}");
            }
        } else {
            drop(reader);
            stream.write_all(b"ERR Unknown\n")?;
        }
    }
    Ok(())
}
