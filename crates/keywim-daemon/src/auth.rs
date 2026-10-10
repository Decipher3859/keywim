use std::process::ExitStatus;

use crate::session;
use pam::Client;

pub fn run_session(
    username: &str,
    password: &str,
    session: &str,
    on_started: impl FnOnce(),
) -> Result<(), String> {
    let command = match session {
        "Keywim (Wayland)" => "/home/damian/dev/keywim/target/debug/keywim",
        _ => return Err(format!("Unknown session: {session}")),
    };
    let mut auth = Client::with_password("keywim").map_err(|e| format!("PAM: {e:?}"))?;

    auth.conversation_mut().set_credentials(username, password);

    auth.authenticate().map_err(|e| format!("PAM: {e:?}"))?;
    let authenticated_user = auth.get_user().map_err(|e| format!("PAM user: {e}"))?;
    let user = session::resolve_user(&authenticated_user)?;

    auth.open_session()
        .map_err(|e| format!("PAM session: {e:?}"))?;

    println!("PAM session opened successfully");

    let mut child = session::run(command, &user)?;

    on_started();

    let status: ExitStatus = child
        .wait()
        .map_err(|e| format!("Session wait failed: {e}"))?;

    println!("Session exited with: {status}");

    Ok(())
}
