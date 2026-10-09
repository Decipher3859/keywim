mod app;
mod daemon;
mod ui;

use app::{App, Focus, Mode};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::run;
use std::io::Result;
use ui::render;

fn main() -> Result<()> {
    run(|terminal| {
        let mut app = App::new();

        loop {
            terminal.draw(|frame| render(frame, &app))?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.mode {
                    Mode::Insert => match key.code {
                        KeyCode::Esc => app.mode = Mode::Command,

                        KeyCode::Enter if app.focus == Focus::Login => match app.login() {
                            Ok(()) => {
                                app.error = Some("Daemon antwortet: OK".into());
                            }
                            Err(error) => {
                                app.error = Some(error);
                            }
                        },

                        KeyCode::Tab | KeyCode::Enter => app.next_focus(),
                        KeyCode::BackTab => app.previous_focus(),

                        KeyCode::Up | KeyCode::Char('k') if app.focus == Focus::Session => {
                            app.previous_session();
                        }

                        KeyCode::Down | KeyCode::Char('j') if app.focus == Focus::Session => {
                            app.next_session();
                        }

                        KeyCode::Backspace if app.focus != Focus::Session => {
                            app.backspace();
                        }

                        KeyCode::Char(c) if app.focus != Focus::Session => {
                            app.insert_char(c);
                        }

                        _ => {}
                    },

                    Mode::Command => match key.code {
                        KeyCode::Esc => break,

                        KeyCode::Char('i') | KeyCode::Enter => app.mode = Mode::Insert,
                        KeyCode::Char('v') => app.mode = Mode::Visual,

                        KeyCode::Char('j') => app.next_focus(),
                        KeyCode::Char('k') => app.previous_focus(),

                        _ => {}
                    },

                    Mode::Visual => match key.code {
                        KeyCode::Esc => app.mode = Mode::Command,

                        _ => {}
                    },
                }
            }
        }

        Ok(())
    })
}
