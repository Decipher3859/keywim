use std::process::ExitStatus;

use crate::session;
use pam::Client;

pub fn run_session(username: &str, password: &str, session: &str) -> Result<(), String> {
    let mut auth = Client::with_password("keywim").map_err(|e| format!("PAM: {e:?}"))?;

    auth.conversation_mut().set_credentials(username, password);

    auth.authenticate().map_err(|e| format!("PAM: {e:?}"))?;
    auth.open_session()
        .map_err(|e| format!("PAM session: {e:?}"))?;

    println!("PAM session opened successfully");

    let command = match session {
        "Keywim (Wayland)" => "/home/damian/dev/keywim/target/debug/keywim",
        _ => return Err(format!("Unknown session: {session}")),
    };
    let user = session::resolve_user(username)?;
    let status: ExitStatus = session::run(command, &user)?;

    println!("Session exited with: {status}");

    Ok(())
}
