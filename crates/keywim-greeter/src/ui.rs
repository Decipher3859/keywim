use crate::app::{App, Focus, Mode};
use ratatui::{
    Frame,
    layout::{
        Alignment, Constraint, Flex, HorizontalAlignment, Layout, Margin, Rect, VerticalAlignment,
    },
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph},
};
use std::fs;

pub fn render(frame: &mut Frame, app: &App) {
    let hostname = fs::read_to_string("/etc/hostname").unwrap_or("KEYWIM LOGIN".to_string());
    let hostname = hostname.trim();

    let area = frame.area();
    let panel = Layout::vertical([Constraint::Length(17)])
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
        vertical: 1,
        horizontal: 3,
    });

    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .flex(Flex::Center)
    .split(inner);

    draw_row(
        frame,
        rows[0],
        "Benutzer",
        &app.username,
        app.focus == Focus::Username,
    );

    let password_display = if app.mode == Mode::Visual && app.focus == Focus::Password {
        app.password.clone()
    } else {
        "*".repeat(app.password.chars().count())
    };

    draw_row(
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

    draw_row(
        frame,
        rows[3],
        "Session",
        session,
        app.focus == Focus::Session,
    );

    draw_button(frame, rows[5], "[ Anmelden ]", app.focus == Focus::Login);

    if let Some(error) = &app.error {
        let message = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));

        frame.render_widget(message, rows[4]);
    }
}

fn draw_row(frame: &mut Frame, area: Rect, label: &str, value: &str, active: bool) {
    let style = if active {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let columns = Layout::horizontal([Constraint::Length(11), Constraint::Fill(1)]).split(area);
    let text = value.to_string();
    let centered = Layout::vertical([Constraint::Length(1)])
        .flex(Flex::Center)
        .split(columns[0])[0];
    frame.render_widget(
        Paragraph::new(label)
            .alignment(HorizontalAlignment::Left)
            .style(Style::default().fg(Color::White)),
        centered,
    );

    frame.render_widget(
        Paragraph::new(text)
            .block(Block::new().borders(Borders::ALL))
            .style(style),
        columns[1],
    );
}

fn draw_button(frame: &mut Frame, area: Rect, value: &str, active: bool) {
    frame.render_widget(
        Paragraph::new(value).block(
            Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        ),
        area,
    )
}
