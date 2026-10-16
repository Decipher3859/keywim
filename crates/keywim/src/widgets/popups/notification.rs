use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Flex, Layout, Margin},
    style::{Color, Style},
    text,
    widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap},
};

pub fn render_notification(
    buffer: &mut Buffer,
    title: &str,
    content: &str,
    notification_type: &str,
) {
    let area = buffer.area;
    let layout = Layout::vertical([Constraint::Length(17)])
        .flex(Flex::Center)
        .split(area)[0];

    let layout = Layout::horizontal([Constraint::Length(44)])
        .flex(Flex::Center)
        .split(layout)[0];

    let title_text = match notification_type {
        "caution" => format!("⚠ {title}"),
        "warning" => format!("! {title}"),
        _ => title.to_owned(),
    };

    let notification_title =
        text::Line::from(format!(" {title_text} ")).style(match notification_type {
            "caution" => Style::default().fg(Color::Red),
            "warning" => Style::default().fg(Color::Yellow),
            "notification" => Style::default().fg(Color::White),
            _ => Style::default().fg(Color::White),
        });

    let block = Block::default()
        .title(notification_title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    block.render(layout, buffer);

    let inner = layout.inner(Margin {
        vertical: 1,
        horizontal: 3,
    });

    let notification_content = Paragraph::new(content)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });

    notification_content.render(inner, buffer);
}

pub fn rasterize_notification(
    rasterizer: &crate::widgets::rasterization::WidgetRasterizer,
    title: &str,
    content: &str,
    notification_type: &str,
) -> Result<crate::widgets::rasterization::RasterizedWidget, &'static str> {
    // Limit the image to the popup so its background cannot cover the screen.
    let mut cells = Buffer::empty(ratatui::layout::Rect::new(0, 0, 44, 17));
    render_notification(&mut cells, title, content, notification_type);
    rasterizer.rasterize(&cells)
}
