use crate::daemon;
use std::{io::BufReader, os::unix::net::UnixStream};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Command,
    Insert,
    Visual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Username,
    Password,
    Session,
    Login,
}

pub struct App {
    pub mode: Mode,
    pub username: String,
    pub password: String,
    pub sessions: Vec<String>,
    pub selected_session: usize,
    pub focus: Focus,

    pub error: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            mode: Mode::Insert,
            username: String::new(),
            password: String::new(),
            sessions: vec!["Keywim (Wayland)".into(), "i3 (X11)".into()],
            selected_session: 0,
            focus: Focus::Username,

            error: None,
        }
    }

    pub fn next_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Username => Focus::Password,
            Focus::Password => Focus::Session,
            Focus::Session => Focus::Login,
            Focus::Login => Focus::Username,
        };
    }

    pub fn previous_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Username => Focus::Login,
            Focus::Password => Focus::Username,
            Focus::Session => Focus::Password,
            Focus::Login => Focus::Session,
        };
    }

    pub fn next_session(&mut self) {
        let count = self.sessions.len();

        if count == 0 {
            return;
        }

        if self.selected_session == count - 1 {
            self.selected_session = 0;
        } else {
            self.selected_session += 1;
        }
    }

    pub fn previous_session(&mut self) {
        let count = self.sessions.len();

        if count == 0 {
            return;
        }

        if self.selected_session == 0 {
            self.selected_session = count - 1;
        } else {
            self.selected_session -= 1;
        }
    }

    pub fn insert_char(&mut self, c: char) {
        match self.focus {
            Focus::Username => self.username.push(c),
            Focus::Password => self.password.push(c),
            _ => {}
        }
    }

    pub fn backspace(&mut self) {
        match self.focus {
            Focus::Username => {
                self.username.pop();
            }
            Focus::Password => {
                self.password.pop();
            }
            _ => {}
        }
    }

    pub fn login(&mut self) -> Result<BufReader<UnixStream>, String> {
        if self.username.trim().is_empty() {
            self.focus = Focus::Username;
            return Err("Username is empty".into());
        }
        if self.password.is_empty() {
            self.focus = Focus::Password;
            return Err("Password is empty".into());
        }
        if self.sessions.get(self.selected_session).is_none() {
            self.focus = Focus::Session;
            return Err("No session selected".into());
        }

        let session = self
            .sessions
            .get(self.selected_session)
            .ok_or("No session selected")?;

        let result = daemon::authenticate(&self.username, &self.password, session);
        self.password.clear();
        result
    }
}
