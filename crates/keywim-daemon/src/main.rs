mod auth;

use std::{
    fs,
    io::{BufRead, BufReader, Result, Write},
    os::unix::net::UnixListener,
    path::Path,
};

fn main() -> Result<()> {
    let socket_path = "/tmp/keywim-dev.sock";

    if Path::new(socket_path).exists() {
        fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;

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

            reader.read_line(&mut username)?;
            reader.read_line(&mut password)?;

            drop(reader);

            let result = auth::authenticate(
                username.trim_end_matches(['\r', '\n']),
                password.trim_end_matches(['\r', '\n']),
            );

            match result {
                Ok(()) => stream.write_all(b"OK\n")?,
                Err(e) => {
                    eprintln!("PAM authentication failed: {e}");
                    stream.write_all(b"DENIED\n")?;
                }
            }
        } else {
            drop(reader);
            stream.write_all(b"ERR Unknown\n")?;
        }
    }
    Ok(())
}
