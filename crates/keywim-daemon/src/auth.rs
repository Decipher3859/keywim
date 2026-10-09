use pam::Client;

pub fn authenticate(username: &str, password: &str) -> Result<(), String> {
    let mut auth = Client::with_password("keywim").map_err(|e| e.to_string())?;

    auth.conversation_mut().set_credentials(username, password);

    auth.authenticate().map_err(|e| format!("PAM: {e:?}"))?;

    Ok(())
}
