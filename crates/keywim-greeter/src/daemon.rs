use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
};

pub fn authenticate(
    username: &str,
    password: &str,
    session: &str,
) -> Result<BufReader<UnixStream>, String> {
    let mut stream = UnixStream::connect("/tmp/keywim-dev.sock")
        .map_err(|e| format!("Daemon connection failed: {e}"))?;
    if username.contains('\n') || password.contains('\n') {
        return Err("Invalid credendtials format".into());
    }

    writeln!(stream, "AUTH").map_err(|e| e.to_string())?;
    writeln!(stream, "{username}").map_err(|e| e.to_string())?;
    writeln!(stream, "{password}").map_err(|e| e.to_string())?;
    writeln!(stream, "{session}").map_err(|e| e.to_string())?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response).map_err(|e| e.to_string())?;

    match response.trim() {
        "OK" => Ok(reader),
        "DENIED" => Err("Authentication failed".into()),
        other => Err(format!("Unexpected daemon response: {other}")),
    }
}
