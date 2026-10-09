use crate::app::{App, Focus, Mode};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};
use std::fs;

pub fn render(frame: &mut Frame, app: &App) {
    let hostname = fs::read_to_string("/etc/hostname").unwrap_or("KEYWIM LOGIN".to_string());
    let hostname = hostname.trim();

    let area = frame.area();
    let panel = Layout::vertical([Constraint::Length(13)])
        .flex(Flex::Center)
        .split(area)[0];

    let panel = Layout::horizontal([Constraint::Length(44)])
        .flex(Flex::Center)
        .split(panel)[0];

    let block = Block::default()
        .title(format!(" {hostname} "))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    frame.render_widget(block, panel);

    let inner = panel.inner(Margin {
        vertical: 2,
        horizontal: 3,
    });

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    draw_field(
        frame,
        rows[0],
        "User",
        &app.username,
        app.focus == Focus::Username,
    );

    let password_display = if app.mode == Mode::Visual && app.focus == Focus::Password {
        app.password.clone()
    } else {
        "*".repeat(app.password.chars().count())
    };

    draw_field(
        frame,
        rows[1],
        "Password",
        &password_display,
        app.focus == Focus::Password,
    );

    let session = app
        .sessions
        .get(app.selected_session)
        .map(String::as_str)
        .unwrap_or("No session available");

    draw_field(
        frame,
        rows[3],
        "Session",
        session,
        app.focus == Focus::Session,
    );

    draw_field(
        frame,
        rows[5],
        "",
        "[ Anmelden ]",
        app.focus == Focus::Login,
    );

    if let Some(error) = &app.error {
        let message = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));

        frame.render_widget(message, rows[4]);
    }
}

fn draw_field(frame: &mut Frame, area: Rect, label: &str, value: &str, active: bool) {
    let style = if active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let text = if label.is_empty() {
        value.to_string()
    } else {
        format!("{label}: {value}")
    };

    frame.render_widget(Paragraph::new(text).style(style), area);
}
