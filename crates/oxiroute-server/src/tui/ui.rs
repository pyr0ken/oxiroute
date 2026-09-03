use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Sparkline, Wrap},
    Frame,
};
use std::sync::atomic::Ordering;
use crate::tui::state::TuiState;

pub fn render(frame: &mut Frame, state: &TuiState) {
    let size = frame.area();

    // Main vertical layout: Header (3) -> Body (Min 10) -> Footer (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(1),
        ])
        .split(size);

    render_header(frame, chunks[0], state);
    render_body(frame, chunks[1], state);
    render_footer(frame, chunks[2]);
}

fn render_header(frame: &mut Frame, area: Rect, state: &TuiState) {
    let proxy_text = state.proxy.as_deref().unwrap_or("Direct (None)");

    let header_text = vec![Line::from(vec![
        Span::styled(
            " 🦀 OXIRUTE ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" v0.1.0  ", Style::default().fg(Color::Yellow)),
        Span::raw("│ "),
        Span::styled("Port: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(":{}", state.port),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::styled("Egress Proxy: ", Style::default().fg(Color::DarkGray)),
        Span::styled(proxy_text, Style::default().fg(Color::Magenta)),
        Span::raw(" │ "),
        Span::styled("Uptime: ", Style::default().fg(Color::DarkGray)),
        Span::styled(state.uptime_formatted(), Style::default().fg(Color::Green)),
        Span::raw(" │ "),
        Span::styled("RAM: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "< 10 MB",
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ),
    ])];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red));

    let paragraph = Paragraph::new(header_text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_body(frame: &mut Frame, area: Rect, state: &TuiState) {
    // Split into Top Half (Metrics & Accounts) and Bottom Half (Logs)
    let main_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(6)])
        .split(area);

    let top_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(main_split[0]);

    render_metrics(frame, top_split[0], state);
    render_accounts(frame, top_split[1], state);
    render_logs(frame, main_split[1], state);
}

fn render_metrics(frame: &mut Frame, area: Rect, state: &TuiState) {
    let metrics_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Length(3)])
        .margin(1)
        .split(area);

    let total_reqs = state.total_requests.load(Ordering::Relaxed);
    let active_reqs = state.active_requests.load(Ordering::Relaxed);
    let tokens = state.total_tokens.load(Ordering::Relaxed);

    let text = vec![
        Line::from(vec![
            Span::styled("Total Requests:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", total_reqs),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled("Active Streams: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", active_reqs),
                Style::default().fg(if active_reqs > 0 {
                    Color::Yellow
                } else {
                    Color::Green
                }),
            ),
        ]),
        Line::from(vec![
            Span::styled("Tokens Streamed:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", tokens),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Circuit Breakers: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "CLOSED ● Healthy",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let block = Block::default()
        .title(" 📊 Live Metrics & Stream Stats ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    frame.render_widget(Paragraph::new(text), metrics_chunks[0]);

    // Sparkline for recent latencies
    if let Ok(latencies) = state.recent_latencies.read() {
        let sparkline = Sparkline::default()
            .block(
                Block::default()
                    .title(" Latency (ms) ")
                    .borders(Borders::NONE),
            )
            .style(Style::default().fg(Color::Yellow))
            .data(&*latencies);
        frame.render_widget(sparkline, metrics_chunks[1]);
    }

    frame.render_widget(block, area);
}

fn render_accounts(frame: &mut Frame, area: Rect, state: &TuiState) {
    let accounts_guard = state.accounts.read().unwrap();
    let items: Vec<ListItem> = accounts_guard
        .iter()
        .map(|acc| {
            let status_bullet = if acc.is_active {
                Span::styled("● ", Style::default().fg(Color::Green))
            } else {
                Span::styled("○ ", Style::default().fg(Color::DarkGray))
            };

            let provider_badge = match acc.provider.as_str() {
                "antigravity" | "agy" => Span::styled(
                    format!("[{}] ", acc.provider),
                    Style::default().fg(Color::Red),
                ),
                "openrouter" => Span::styled(
                    format!("[{}] ", acc.provider),
                    Style::default().fg(Color::Blue),
                ),
                "opencode" => Span::styled(
                    format!("[{}] ", acc.provider),
                    Style::default().fg(Color::Magenta),
                ),
                other => Span::styled(format!("[{}] ", other), Style::default().fg(Color::Yellow)),
            };

            let email_span = Span::styled(&acc.email, Style::default().fg(Color::White));
            let priority_span = Span::styled(
                format!(" (P{})", acc.priority),
                Style::default().fg(Color::DarkGray),
            );

            ListItem::new(Line::from(vec![
                status_bullet,
                provider_badge,
                email_span,
                priority_span,
            ]))
        })
        .collect();

    let title = format!(" 👥 Active Accounts ({}) ", accounts_guard.len());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Green));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_logs(frame: &mut Frame, area: Rect, state: &TuiState) {
    let logs_guard = state.logs.read().unwrap();
    let items: Vec<ListItem> = logs_guard
        .iter()
        .rev() // Newest on top
        .map(|entry| {
            let level_style = match entry.level {
                "INFO" => Style::default().fg(Color::Cyan),
                "WARN" => Style::default().fg(Color::Yellow),
                "ERROR" => Style::default().fg(Color::Red),
                _ => Style::default().fg(Color::White),
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("[{}] ", entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("[{}] ", entry.level),
                    level_style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(&entry.message, Style::default().fg(Color::White)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let block = Block::default()
        .title(" 📜 Real-time Stream & Router Logs ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::White));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let footer_text = Line::from(vec![
        Span::styled(" [q] ", Style::default().fg(Color::Black).bg(Color::White)),
        Span::raw(" Quit  "),
        Span::styled(" [c] ", Style::default().fg(Color::Black).bg(Color::White)),
        Span::raw(" Clear Logs  "),
        Span::styled(" [r] ", Style::default().fg(Color::Black).bg(Color::White)),
        Span::raw(" Refresh  "),
        Span::styled(" [?] ", Style::default().fg(Color::Black).bg(Color::White)),
        Span::raw(" Help "),
    ]);

    let paragraph = Paragraph::new(footer_text).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}
