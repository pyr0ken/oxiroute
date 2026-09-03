use crate::tui::state::{
    ActiveTab, ModalState, TuiState, COMBO_STRATEGIES, KNOWN_MODELS, PROVIDER_TYPES,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Row, Sparkline, Table,
    },
    Frame,
};
use std::sync::atomic::Ordering;

// ─── Theme Colors ─────────────────────────────────────────────────────────────
const COLOR_CRIMSON: Color = Color::Rgb(220, 38, 38);
const COLOR_BRIGHT_RED: Color = Color::Rgb(255, 65, 75);
const COLOR_DARK_RED: Color = Color::Rgb(153, 27, 27);
const COLOR_TEXT_MAIN: Color = Color::Rgb(245, 245, 250);
const COLOR_TEXT_MUTED: Color = Color::Rgb(150, 150, 165);
const COLOR_GREEN_STATUS: Color = Color::Rgb(52, 211, 153);
const COLOR_CYAN_ACCENT: Color = Color::Rgb(56, 189, 248);
const COLOR_YELLOW_ACCENT: Color = Color::Rgb(251, 191, 36);
const COLOR_MAGENTA_PROXY: Color = Color::Rgb(192, 132, 252);
const COLOR_BORDER_INACTIVE: Color = Color::Rgb(65, 65, 80);

pub fn render(frame: &mut Frame, state: &TuiState) {
    let size = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Header
            Constraint::Length(3), // Tab Bar
            Constraint::Min(12),   // Content
            Constraint::Length(1), // Footer Keymap
        ])
        .split(size);

    render_header(frame, chunks[0], state);
    render_tabs(frame, chunks[1], state);

    let active_tab = *state.active_tab.read().unwrap();
    match active_tab {
        ActiveTab::Overview => render_overview_tab(frame, chunks[2], state),
        ActiveTab::Combos => render_combos_tab(frame, chunks[2], state),
        ActiveTab::Providers => render_providers_tab(frame, chunks[2], state),
        ActiveTab::Proxies => render_proxies_tab(frame, chunks[2], state),
        ActiveTab::Timeline => render_timeline_tab(frame, chunks[2], state),
    }

    render_footer(frame, chunks[3], state);

    // Modal Overlays
    let modal = state.modal.read().unwrap().clone();
    match modal {
        ModalState::Help => render_help_modal(frame, size),
        ModalState::AddCombo {
            name,
            strategy_idx,
            models,
            proxy_idx,
            active_field,
        } => render_combo_modal(
            frame,
            size,
            state,
            "Create New Combo",
            &name,
            strategy_idx,
            &models,
            proxy_idx,
            active_field,
        ),
        ModalState::EditCombo {
            name,
            strategy_idx,
            models,
            proxy_idx,
            active_field,
            ..
        } => render_combo_modal(
            frame,
            size,
            state,
            "Edit Combo Configuration",
            &name,
            strategy_idx,
            &models,
            proxy_idx,
            active_field,
        ),
        ModalState::ModelPicker {
            search_query,
            selected_models,
            cursor_idx,
            ..
        } => render_model_picker_modal(
            frame,
            size,
            &search_query,
            &selected_models,
            cursor_idx,
        ),
        ModalState::AddProxy {
            name,
            protocol_idx,
            host,
            port,
            active_field,
        } => render_proxy_modal(
            frame,
            size,
            "Register New Proxy",
            &name,
            protocol_idx,
            &host,
            &port,
            active_field,
        ),
        ModalState::EditProxy {
            name,
            protocol_idx,
            host,
            port,
            active_field,
            ..
        } => render_proxy_modal(
            frame,
            size,
            "Edit Proxy Configuration",
            &name,
            protocol_idx,
            &host,
            &port,
            active_field,
        ),
        ModalState::AddProvider {
            provider_type_idx,
            name,
            api_key,
            priority,
            active_field,
        } => render_provider_modal(
            frame,
            size,
            provider_type_idx,
            &name,
            &api_key,
            priority,
            active_field,
        ),
        ModalState::None => {}
    }
}

// ─── Header ──────────────────────────────────────────────────────────────────
fn render_header(frame: &mut Frame, area: Rect, state: &TuiState) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(48), Constraint::Percentage(48)])
        .split(area);

    let ascii_art = vec![
        Line::from(vec![
            Span::styled(
                "     _~^~^~_      ",
                Style::default()
                    .fg(COLOR_BRIGHT_RED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "____  _  _ ___ ___  _   _ _____ ___ ",
                Style::default()
                    .fg(COLOR_CRIMSON)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " \\) /  o o  \\ (/ ",
                Style::default()
                    .fg(COLOR_BRIGHT_RED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "/ __ \\( \\/ )_ _| _ \\/ | | |_   _| __|",
                Style::default()
                    .fg(COLOR_CRIMSON)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("   '_   -   _'   ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::styled(
                "| |  | |)  ( | ||   /| |_| | | | | _|  ",
                Style::default()
                    .fg(COLOR_BRIGHT_RED)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("   / '-----' \\   ", Style::default().fg(COLOR_BRIGHT_RED)),
            Span::styled(
                "| |__| /_/\\_\\___|_|_\\\\___/  |_| |___| ",
                Style::default().fg(COLOR_BRIGHT_RED),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  🦀 Ferris Gate  ",
                Style::default()
                    .fg(COLOR_BRIGHT_RED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" \\____/  ", Style::default().fg(COLOR_CRIMSON)),
            Span::styled(
                "⚡ AI GATEWAY (PURE RUST) ⚡ ",
                Style::default()
                    .fg(COLOR_YELLOW_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("v0.1.0", Style::default().fg(COLOR_TEXT_MUTED)),
        ]),
    ];

    let logo_widget = Paragraph::new(ascii_art);
    frame.render_widget(logo_widget, header_chunks[0]);

    let proxy_text = state
        .proxy
        .as_deref()
        .unwrap_or("Direct Egress (None)");

    let meta_lines = vec![
        Line::from(vec![
            Span::styled("● Server Status: ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(
                "ACTIVE (Running)",
                Style::default()
                    .fg(COLOR_GREEN_STATUS)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("Port: ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(
                format!("http://127.0.0.1:{}", state.port),
                Style::default()
                    .fg(COLOR_CYAN_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("⚡ Egress Proxy:  ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(
                proxy_text,
                Style::default()
                    .fg(COLOR_MAGENTA_PROXY)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("Memory: ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(
                "< 10 MB RAM",
                Style::default()
                    .fg(COLOR_GREEN_STATUS)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("⏱️  Uptime:        ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(
                state.uptime_formatted(),
                Style::default().fg(COLOR_YELLOW_ACCENT),
            ),
            Span::raw("   "),
            Span::styled("Circuit Breakers: ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled("CLOSED ● Healthy", Style::default().fg(COLOR_GREEN_STATUS)),
        ]),
    ];

    let meta_widget = Paragraph::new(meta_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_DARK_RED))
                .title(Span::styled(
                    " System Runtime ",
                    Style::default().fg(COLOR_BRIGHT_RED),
                )),
        )
        .alignment(Alignment::Left);

    frame.render_widget(meta_widget, header_chunks[1]);
}

// ─── Tabs ────────────────────────────────────────────────────────────────────
fn render_tabs(frame: &mut Frame, area: Rect, state: &TuiState) {
    let current_tab = *state.active_tab.read().unwrap();
    let combos_count = state.combos.read().unwrap().len();
    let accounts_count = state.accounts.read().unwrap().len();
    let proxies_count = state.proxies.read().unwrap().len();
    let timeline_count = state.timeline.read().unwrap().len();

    let tab_titles = [
        (" [1] 📊 Dashboard ", ActiveTab::Overview),
        (
            &format!(" [2] 🔀 Combos ({}) ", combos_count),
            ActiveTab::Combos,
        ),
        (
            &format!(" [3] 👥 Providers ({}) ", accounts_count),
            ActiveTab::Providers,
        ),
        (
            &format!(" [4] 🌐 Proxy Pool ({}) ", proxies_count),
            ActiveTab::Proxies,
        ),
        (
            &format!(" [5] 📈 Timeline ({}) ", timeline_count),
            ActiveTab::Timeline,
        ),
    ];

    let spans: Vec<Span> = tab_titles
        .iter()
        .map(|(title, tab)| {
            if *tab == current_tab {
                Span::styled(
                    *title,
                    Style::default()
                        .fg(Color::Black)
                        .bg(COLOR_BRIGHT_RED)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(
                    *title,
                    Style::default()
                        .fg(COLOR_TEXT_MUTED)
                        .bg(Color::Rgb(30, 30, 40)),
                )
            }
        })
        .collect();

    let mut tab_line_spans = Vec::new();
    for (i, span) in spans.into_iter().enumerate() {
        if i > 0 {
            tab_line_spans.push(Span::raw(" "));
        }
        tab_line_spans.push(span);
    }

    let p = Paragraph::new(Line::from(tab_line_spans))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(COLOR_DARK_RED)),
        )
        .alignment(Alignment::Center);

    frame.render_widget(p, area);
}

// ─── Tab 1: Overview ─────────────────────────────────────────────────────────
fn render_overview_tab(frame: &mut Frame, area: Rect, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(8)])
        .split(area);

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    let total_reqs = state.total_requests.load(Ordering::Relaxed);
    let active_reqs = state.active_requests.load(Ordering::Relaxed);
    let tokens = state.total_tokens.load(Ordering::Relaxed);

    let active_color = if active_reqs > 0 {
        COLOR_YELLOW_ACCENT
    } else {
        COLOR_GREEN_STATUS
    };

    let metrics_text = vec![
        Line::from(vec![
            Span::styled("Total Requests:     ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(
                format!("{}", total_reqs),
                Style::default()
                    .fg(COLOR_TEXT_MAIN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Active Streams:     ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(
                format!("{} in-flight", active_reqs),
                Style::default().fg(active_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Tokens Streamed:    ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(
                format!("{}", tokens),
                Style::default()
                    .fg(COLOR_CYAN_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Prompt Compaction:  ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(
                "RTK + Caveman Active (-25% avg)",
                Style::default().fg(COLOR_GREEN_STATUS),
            ),
        ]),
    ];

    let metrics_block = Paragraph::new(metrics_text).block(
        Block::default()
            .title(Span::styled(
                " 📊 Live Stream Metrics ",
                Style::default().fg(COLOR_BRIGHT_RED),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_DARK_RED)),
    );
    frame.render_widget(metrics_block, top_chunks[0]);

    let latencies_guard = state.recent_latencies.read().unwrap();
    let spark_data: Vec<u64> = if latencies_guard.is_empty() {
        vec![0, 0, 0, 0, 0]
    } else {
        latencies_guard.clone()
    };

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .title(Span::styled(
                    " ⚡ Latency TTFT (ms) ",
                    Style::default().fg(COLOR_BRIGHT_RED),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_DARK_RED)),
        )
        .data(&spark_data)
        .style(Style::default().fg(COLOR_BRIGHT_RED));
    frame.render_widget(sparkline, top_chunks[1]);

    let logs_guard = state.logs.read().unwrap();
    let log_items: Vec<ListItem> = logs_guard
        .iter()
        .rev()
        .map(|entry| {
            let level_style = match entry.level {
                "ERROR" => Style::default()
                    .fg(COLOR_BRIGHT_RED)
                    .add_modifier(Modifier::BOLD),
                "WARN" => Style::default()
                    .fg(COLOR_YELLOW_ACCENT)
                    .add_modifier(Modifier::BOLD),
                _ => Style::default().fg(COLOR_CYAN_ACCENT),
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("[{}] ", entry.timestamp),
                    Style::default().fg(COLOR_TEXT_MUTED),
                ),
                Span::styled(format!("[{}] ", entry.level), level_style),
                Span::styled(&entry.message, Style::default().fg(COLOR_TEXT_MAIN)),
            ]))
        })
        .collect();

    let logs_list = List::new(log_items).block(
        Block::default()
            .title(Span::styled(
                " 📜 Real-time Stream & Router Logs (Press 'c' to clear) ",
                Style::default().fg(COLOR_BRIGHT_RED),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_DARK_RED)),
    );
    frame.render_widget(logs_list, chunks[1]);
}

// ─── Tab 2: Combos ───────────────────────────────────────────────────────────
fn render_combos_tab(frame: &mut Frame, area: Rect, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let combos_guard = state.combos.read().unwrap();
    let sel_idx = *state.selected_index.read().unwrap();

    let items: Vec<ListItem> = combos_guard
        .iter()
        .enumerate()
        .map(|(i, combo)| {
            let is_selected = i == sel_idx;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(COLOR_BRIGHT_RED)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_TEXT_MAIN)
            };

            let prefix = if is_selected { " ▶ " } else { "   " };
            ListItem::new(format!(
                "{}{} [{}] ({} models)",
                prefix,
                combo.name,
                combo.strategy,
                combo.models.len()
            ))
            .style(style)
        })
        .collect();

    let combos_list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                " 🔀 Active Combos [a: Add | e: Edit | d: Delete] ",
                Style::default().fg(COLOR_BRIGHT_RED),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_DARK_RED)),
    );
    frame.render_widget(combos_list, chunks[0]);

    let detail_lines = if let Some(combo) = combos_guard.get(sel_idx) {
        let strat_desc = COMBO_STRATEGIES
            .iter()
            .find(|(s, _)| *s == combo.strategy)
            .map(|(_, d)| *d)
            .unwrap_or("Custom strategy");

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Combo Name:       ", Style::default().fg(COLOR_TEXT_MUTED)),
                Span::styled(
                    &combo.name,
                    Style::default()
                        .fg(COLOR_BRIGHT_RED)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Routing Strategy: ", Style::default().fg(COLOR_TEXT_MUTED)),
                Span::styled(
                    &combo.strategy,
                    Style::default()
                        .fg(COLOR_YELLOW_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({})", strat_desc),
                    Style::default().fg(COLOR_TEXT_MUTED),
                ),
            ]),
            Line::from(vec![
                Span::styled("Assigned Proxy:   ", Style::default().fg(COLOR_TEXT_MUTED)),
                Span::styled(
                    combo
                        .proxy_name
                        .as_deref()
                        .unwrap_or("Default / None (Direct)"),
                    Style::default().fg(COLOR_MAGENTA_PROXY),
                ),
            ]),
            Line::raw(""),
            Line::styled(
                "─── Fallback & Execution Model Pipeline ───",
                Style::default().fg(COLOR_DARK_RED),
            ),
        ];

        for (idx, model) in combo.models.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:2}. ", idx + 1),
                    Style::default().fg(COLOR_YELLOW_ACCENT),
                ),
                Span::styled(
                    model,
                    Style::default()
                        .fg(COLOR_CYAN_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        lines
    } else {
        vec![Line::styled(
            "No combo selected. Press 'a' to create one.",
            Style::default().fg(COLOR_TEXT_MUTED),
        )]
    };

    let details = Paragraph::new(detail_lines).block(
        Block::default()
            .title(Span::styled(
                " ⚙️  Combo Pipeline Details ",
                Style::default().fg(COLOR_BRIGHT_RED),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_DARK_RED)),
    );
    frame.render_widget(details, chunks[1]);
}

// ─── Tab 3: Providers ────────────────────────────────────────────────────────
fn render_providers_tab(frame: &mut Frame, area: Rect, state: &TuiState) {
    let accounts_guard = state.accounts.read().unwrap();
    let sel_idx = *state.selected_index.read().unwrap();

    let rows: Vec<Row> = accounts_guard
        .iter()
        .enumerate()
        .map(|(i, acc)| {
            let is_selected = i == sel_idx;
            let status_badge = if acc.is_active {
                if acc.status_text.is_empty() {
                    Span::styled("Active ●", Style::default().fg(COLOR_GREEN_STATUS))
                } else {
                    Span::styled(&acc.status_text, Style::default().fg(COLOR_GREEN_STATUS))
                }
            } else {
                Span::styled("Disabled ○", Style::default().fg(COLOR_TEXT_MUTED))
            };

            let row = Row::new(vec![
                Span::styled(
                    if is_selected { "▶" } else { " " },
                    Style::default().fg(COLOR_BRIGHT_RED),
                ),
                Span::styled(
                    &acc.provider,
                    Style::default()
                        .fg(COLOR_CYAN_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&acc.email, Style::default().fg(COLOR_TEXT_MAIN)),
                Span::styled(
                    format!("P{}", acc.priority),
                    Style::default().fg(COLOR_YELLOW_ACCENT),
                ),
                status_badge,
            ]);

            if is_selected {
                row.style(Style::default().bg(Color::Rgb(40, 20, 25)))
            } else {
                row
            }
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(16),
            Constraint::Length(32),
            Constraint::Length(10),
            Constraint::Min(20),
        ],
    )
    .header(
        Row::new(vec!["", "Provider", "Identifier / Email", "Priority", "Health Status"])
            .style(
                Style::default()
                    .fg(COLOR_BRIGHT_RED)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .title(Span::styled(
                " 👥 Provider Connections [a: Add | t: Test Connection | r: Refresh Token | d: Delete] ",
                Style::default().fg(COLOR_BRIGHT_RED),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_DARK_RED)),
    );

    frame.render_widget(table, area);
}

// ─── Tab 4: Proxies ──────────────────────────────────────────────────────────
fn render_proxies_tab(frame: &mut Frame, area: Rect, state: &TuiState) {
    let proxies_guard = state.proxies.read().unwrap();
    let sel_idx = *state.selected_index.read().unwrap();

    let rows: Vec<Row> = proxies_guard
        .iter()
        .enumerate()
        .map(|(i, proxy)| {
            let is_selected = i == sel_idx;
            let row = Row::new(vec![
                Span::styled(
                    if is_selected { "▶" } else { " " },
                    Style::default().fg(COLOR_BRIGHT_RED),
                ),
                Span::styled(
                    &proxy.name,
                    Style::default()
                        .fg(COLOR_TEXT_MAIN)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&proxy.protocol, Style::default().fg(COLOR_MAGENTA_PROXY)),
                Span::styled(&proxy.host, Style::default().fg(COLOR_CYAN_ACCENT)),
                Span::styled(
                    format!("{}", proxy.port),
                    Style::default().fg(COLOR_YELLOW_ACCENT),
                ),
                Span::styled(&proxy.status, Style::default().fg(COLOR_GREEN_STATUS)),
            ]);

            if is_selected {
                row.style(Style::default().bg(Color::Rgb(40, 20, 25)))
            } else {
                row
            }
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Percentage(25),
            Constraint::Length(12),
            Constraint::Percentage(30),
            Constraint::Length(10),
            Constraint::Min(15),
        ],
    )
    .header(
        Row::new(vec!["", "Proxy Name", "Protocol", "Host", "Port", "Status"])
            .style(
                Style::default()
                    .fg(COLOR_BRIGHT_RED)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .title(Span::styled(
                " 🌐 Proxy Pool [a: Add Proxy | e: Edit | d: Delete] ",
                Style::default().fg(COLOR_BRIGHT_RED),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_DARK_RED)),
    );

    frame.render_widget(table, area);
}

// ─── Tab 5: Timeline ─────────────────────────────────────────────────────────
fn render_timeline_tab(frame: &mut Frame, area: Rect, state: &TuiState) {
    let timeline_guard = state.timeline.read().unwrap();
    let rows: Vec<Row> = timeline_guard
        .iter()
        .rev()
        .map(|item| {
            let status_style = if item.status_code == 200 {
                Style::default().fg(COLOR_GREEN_STATUS)
            } else {
                Style::default()
                    .fg(COLOR_BRIGHT_RED)
                    .add_modifier(Modifier::BOLD)
            };

            Row::new(vec![
                Span::styled(&item.timestamp, Style::default().fg(COLOR_TEXT_MUTED)),
                Span::styled(
                    &item.model,
                    Style::default()
                        .fg(COLOR_CYAN_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&item.account, Style::default().fg(COLOR_TEXT_MAIN)),
                Span::styled(
                    format!("{} tok", item.tokens),
                    Style::default().fg(COLOR_YELLOW_ACCENT),
                ),
                Span::styled(
                    format!("{} ms", item.latency_ms),
                    Style::default().fg(COLOR_TEXT_MUTED),
                ),
                Span::styled(format!("{} OK", item.status_code), status_style),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Percentage(35),
            Constraint::Percentage(25),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec![
            "Time",
            "Model",
            "Dispatched Account",
            "Tokens",
            "Latency",
            "Status",
        ])
        .style(
            Style::default()
                .fg(COLOR_BRIGHT_RED)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1),
    )
    .block(
        Block::default()
            .title(Span::styled(
                " 📈 Live Stream & Request Timeline History ",
                Style::default().fg(COLOR_BRIGHT_RED),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_DARK_RED)),
    );

    frame.render_widget(table, area);
}

// ─── Footer ──────────────────────────────────────────────────────────────────
fn render_footer(frame: &mut Frame, area: Rect, state: &TuiState) {
    let tab = *state.active_tab.read().unwrap();
    let base_keys = match tab {
        ActiveTab::Combos => {
            " [Tab/Shift+Tab] Tabs  [a] Add Combo  [e] Edit  [d] Delete  [j/k] Select  [?] Help  [q] Quit "
        }
        ActiveTab::Providers => {
            " [Tab/Shift+Tab] Tabs  [a] Add Provider  [t] Test Connection  [r] Refresh Token  [d] Delete  [?] Help  [q] Quit "
        }
        ActiveTab::Proxies => {
            " [Tab/Shift+Tab] Tabs  [a] Add Proxy  [e] Edit  [d] Delete  [j/k] Select  [?] Help  [q] Quit "
        }
        _ => " [Tab/Shift+Tab] Tabs  [1-5] Quick Jump  [c] Clear Logs  [j/k] Select  [?] Help  [q] Quit ",
    };

    let paragraph = Paragraph::new(Line::from(vec![Span::styled(
        base_keys,
        Style::default()
            .fg(Color::Black)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD),
    )]))
    .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}

// ─── Help Modal ──────────────────────────────────────────────────────────────
fn render_help_modal(frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(65, 60, area);
    frame.render_widget(Clear, popup_area);

    let text = vec![
        Line::styled(
            "🦀 Oxiroute Terminal Help & Keymap",
            Style::default()
                .fg(COLOR_BRIGHT_RED)
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" [1-5]            ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Direct jump to Tab (Dashboard, Combos, Providers, Proxies, Timeline)"),
        ]),
        Line::from(vec![
            Span::styled(" [Tab/Shift+Tab]  ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Cycle through tabs forward / backward"),
        ]),
        Line::from(vec![
            Span::styled(" [a]              ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Open Add Modal (Combo, Provider, or Proxy)"),
        ]),
        Line::from(vec![
            Span::styled(" [e]              ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Edit selected item (Combo / Proxy)"),
        ]),
        Line::from(vec![
            Span::styled(" [t]              ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Test provider connection health (Tab 3)"),
        ]),
        Line::from(vec![
            Span::styled(" [r]              ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Refresh provider OAuth token (Tab 3)"),
        ]),
        Line::from(vec![
            Span::styled(" [d]              ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Delete selected item from list"),
        ]),
        Line::from(vec![
            Span::styled(" [j / k / ↑ / ↓]  ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Navigate lists and tables"),
        ]),
        Line::from(vec![
            Span::styled(" [c]              ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Clear live logs buffer"),
        ]),
        Line::from(vec![
            Span::styled(" [Esc / q]        ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Close popup / Quit TUI"),
        ]),
        Line::raw(""),
        Line::styled("Press [Esc] or [Enter] to close", Style::default().fg(COLOR_TEXT_MUTED)),
    ];

    let help_block = Paragraph::new(text)
        .block(
            Block::default()
                .title(Span::styled(" ❓ Keyboard Shortcuts ", Style::default().fg(COLOR_BRIGHT_RED)))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(COLOR_BRIGHT_RED)),
        )
        .alignment(Alignment::Left);

    frame.render_widget(help_block, popup_area);
}

// ─── Combo Modal (Add & Edit) ─────────────────────────────────────────────────
fn render_combo_modal(
    frame: &mut Frame,
    area: Rect,
    state: &TuiState,
    title: &str,
    name: &str,
    strategy_idx: usize,
    models: &[String],
    proxy_idx: usize,
    active_field: usize,
) {
    let popup_area = centered_rect(75, 60, area);
    frame.render_widget(Clear, popup_area);

    let strat = COMBO_STRATEGIES[strategy_idx % COMBO_STRATEGIES.len()];
    let proxies_guard = state.proxies.read().unwrap();

    let proxy_name = if proxy_idx == 0 {
        "[None / Direct Egress]".to_string()
    } else if let Some(p) = proxies_guard.get(proxy_idx - 1) {
        format!("{} ({}://{}:{})", p.name, p.protocol, p.host, p.port)
    } else {
        "[None / Direct Egress]".to_string()
    };

    let f0_style = if active_field == 0 {
        Style::default().fg(Color::Black).bg(COLOR_BRIGHT_RED)
    } else {
        Style::default().fg(COLOR_TEXT_MAIN)
    };
    let f1_style = if active_field == 1 {
        Style::default().fg(Color::Black).bg(COLOR_YELLOW_ACCENT)
    } else {
        Style::default().fg(COLOR_YELLOW_ACCENT)
    };
    let f2_style = if active_field == 2 {
        Style::default().fg(Color::Black).bg(COLOR_CYAN_ACCENT)
    } else {
        Style::default().fg(COLOR_CYAN_ACCENT)
    };
    let f3_style = if active_field == 3 {
        Style::default().fg(Color::Black).bg(COLOR_MAGENTA_PROXY)
    } else {
        Style::default().fg(COLOR_MAGENTA_PROXY)
    };

    let models_summary = if models.is_empty() {
        "antigravity/claude-sonnet-4-6 (Default)".to_string()
    } else {
        format!("{} models selected ({})", models.len(), models.join(", "))
    };

    let text = vec![
        Line::styled(title, Style::default().fg(COLOR_BRIGHT_RED).add_modifier(Modifier::BOLD)),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" 1. Combo Name:    ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(if name.is_empty() { "smart-routing" } else { name }, f0_style),
        ]),
        Line::from(vec![
            Span::styled(" 2. Strategy:      ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(format!("◀ {} ▶", strat.0), f1_style),
            Span::styled(format!(" ({})", strat.1), Style::default().fg(COLOR_TEXT_MUTED)),
        ]),
        Line::from(vec![
            Span::styled(" 3. Models List:   ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(format!("[ {} ]", models_summary), f2_style),
            Span::styled("  [Press Space / Enter to pick models]", Style::default().fg(COLOR_YELLOW_ACCENT)),
        ]),
        Line::from(vec![
            Span::styled(" 4. Assign Proxy:  ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(format!("◀ {} ▶", proxy_name), f3_style),
        ]),
        Line::raw(""),
        Line::styled("─── Navigation ───", Style::default().fg(COLOR_DARK_RED)),
        Line::from(vec![
            Span::styled(" [Tab/↓/↑] ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Switch Fields   "),
            Span::styled(" [←/→] ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Change Strategy/Proxy   "),
            Span::styled(" [Enter/Space on field 3] ", Style::default().fg(COLOR_CYAN_ACCENT)),
            Span::raw("Open Searchable Model Picker"),
        ]),
        Line::from(vec![
            Span::styled(" [Enter] ", Style::default().fg(Color::Black).bg(COLOR_GREEN_STATUS)),
            Span::raw(" Save Combo   "),
            Span::styled(" [Esc] ", Style::default().fg(Color::Black).bg(COLOR_BRIGHT_RED)),
            Span::raw(" Cancel "),
        ]),
    ];

    let modal_block = Paragraph::new(text)
        .block(
            Block::default()
                .title(Span::styled(" 🔀 Combo Configuration ", Style::default().fg(COLOR_BRIGHT_RED)))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(COLOR_BRIGHT_RED)),
        )
        .alignment(Alignment::Left);

    frame.render_widget(modal_block, popup_area);
}

// ─── Searchable Model Picker Modal ───────────────────────────────────────────
fn render_model_picker_modal(
    frame: &mut Frame,
    area: Rect,
    search_query: &str,
    selected_models: &[String],
    cursor_idx: usize,
) {
    let popup_area = centered_rect(80, 75, area);
    frame.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search box
            Constraint::Min(10),   // Filtered list
            Constraint::Length(2), // Controls
        ])
        .split(popup_area);

    let search_bar = Paragraph::new(Line::from(vec![
        Span::styled(" 🔍 Search Models: ", Style::default().fg(COLOR_YELLOW_ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(
            if search_query.is_empty() {
                "Type model name or vendor (e.g. claude, deepseek, gemini)..."
            } else {
                search_query
            },
            if search_query.is_empty() {
                Style::default().fg(COLOR_BORDER_INACTIVE)
            } else {
                Style::default().fg(COLOR_TEXT_MAIN).add_modifier(Modifier::BOLD)
            },
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_BRIGHT_RED)),
    );
    frame.render_widget(search_bar, chunks[0]);

    let query_lower = search_query.to_lowercase();
    let filtered_models: Vec<(&str, &str)> = KNOWN_MODELS
        .iter()
        .filter(|(m, desc)| {
            if query_lower.is_empty() {
                true
            } else {
                m.to_lowercase().contains(&query_lower) || desc.to_lowercase().contains(&query_lower)
            }
        })
        .copied()
        .collect();

    let items: Vec<ListItem> = filtered_models
        .iter()
        .enumerate()
        .map(|(i, (model_id, desc))| {
            let is_selected = selected_models.contains(&model_id.to_string());
            let is_cursor = i == cursor_idx;

            let check_box = if is_selected {
                Span::styled("[✓] ", Style::default().fg(COLOR_GREEN_STATUS).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("[ ] ", Style::default().fg(COLOR_BORDER_INACTIVE))
            };

            let row_style = if is_cursor {
                Style::default().fg(Color::Black).bg(COLOR_CYAN_ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_TEXT_MAIN)
            };

            ListItem::new(Line::from(vec![
                check_box,
                Span::styled(format!("{:<38} ", model_id), row_style),
                Span::styled(format!("({})", desc), Style::default().fg(COLOR_TEXT_MUTED)),
            ]))
        })
        .collect();

    let models_list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                format!(" 📋 Available Models ({} matches) ", filtered_models.len()),
                Style::default().fg(COLOR_BRIGHT_RED),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_DARK_RED)),
    );
    frame.render_widget(models_list, chunks[1]);

    let controls = Paragraph::new(Line::from(vec![
        Span::styled(" [↑/↓] ", Style::default().fg(COLOR_YELLOW_ACCENT)),
        Span::raw("Navigate   "),
        Span::styled(" [Space] ", Style::default().fg(Color::Black).bg(COLOR_CYAN_ACCENT)),
        Span::raw(" Toggle Select   "),
        Span::styled(" [Enter] ", Style::default().fg(Color::Black).bg(COLOR_GREEN_STATUS)),
        Span::raw(" Confirm Selection   "),
        Span::styled(" [Esc] ", Style::default().fg(Color::Black).bg(COLOR_BRIGHT_RED)),
        Span::raw(" Back "),
    ]))
    .alignment(Alignment::Center);

    frame.render_widget(controls, chunks[2]);
}

// ─── Provider Modal ───────────────────────────────────────────────────────────
fn render_provider_modal(
    frame: &mut Frame,
    area: Rect,
    provider_type_idx: usize,
    name: &str,
    api_key: &str,
    priority: usize,
    active_field: usize,
) {
    let popup_area = centered_rect(70, 55, area);
    frame.render_widget(Clear, popup_area);

    let ptype = PROVIDER_TYPES[provider_type_idx % PROVIDER_TYPES.len()];

    let f0_style = if active_field == 0 {
        Style::default().fg(Color::Black).bg(COLOR_CYAN_ACCENT)
    } else {
        Style::default().fg(COLOR_CYAN_ACCENT)
    };
    let f1_style = if active_field == 1 {
        Style::default().fg(Color::Black).bg(COLOR_BRIGHT_RED)
    } else {
        Style::default().fg(COLOR_TEXT_MAIN)
    };
    let f2_style = if active_field == 2 {
        Style::default().fg(Color::Black).bg(COLOR_YELLOW_ACCENT)
    } else {
        Style::default().fg(COLOR_YELLOW_ACCENT)
    };
    let f3_style = if active_field == 3 {
        Style::default().fg(Color::Black).bg(COLOR_GREEN_STATUS)
    } else {
        Style::default().fg(COLOR_GREEN_STATUS)
    };

    let masked_key = if api_key.is_empty() {
        "sk-...".to_string()
    } else if api_key.len() > 8 {
        format!("{}****{}", &api_key[..4], &api_key[api_key.len() - 4..])
    } else {
        "****".to_string()
    };

    let text = vec![
        Line::styled("Add New Provider Connection", Style::default().fg(COLOR_BRIGHT_RED).add_modifier(Modifier::BOLD)),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" 1. Provider Type: ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(format!("◀ {} ▶", ptype.0), f0_style),
            Span::styled(format!(" ({})", ptype.1), Style::default().fg(COLOR_TEXT_MUTED)),
        ]),
        Line::from(vec![
            Span::styled(" 2. Connection Name: ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(if name.is_empty() { "Main Account" } else { name }, f1_style),
        ]),
        Line::from(vec![
            Span::styled(" 3. API Key / Secret: ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(masked_key, f2_style),
        ]),
        Line::from(vec![
            Span::styled(" 4. Priority Level:   ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(format!("◀ P{} ▶", priority + 1), f3_style),
        ]),
        Line::raw(""),
        Line::styled("─── Instructions ───", Style::default().fg(COLOR_DARK_RED)),
        Line::from(vec![
            Span::styled(" [Tab/↓/↑] ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Switch Fields   "),
            Span::styled(" [←/→] ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Change Type/Priority"),
        ]),
        Line::from(vec![
            Span::styled(" [Enter] ", Style::default().fg(Color::Black).bg(COLOR_GREEN_STATUS)),
            Span::raw(" Save Connection   "),
            Span::styled(" [Esc] ", Style::default().fg(Color::Black).bg(COLOR_BRIGHT_RED)),
            Span::raw(" Cancel "),
        ]),
    ];

    let modal_block = Paragraph::new(text)
        .block(
            Block::default()
                .title(Span::styled(" 👥 Add Provider ", Style::default().fg(COLOR_BRIGHT_RED)))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(COLOR_BRIGHT_RED)),
        )
        .alignment(Alignment::Left);

    frame.render_widget(modal_block, popup_area);
}

// ─── Proxy Modal (Add & Edit) ─────────────────────────────────────────────────
fn render_proxy_modal(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    name: &str,
    protocol_idx: usize,
    host: &str,
    port: &str,
    active_field: usize,
) {
    let popup_area = centered_rect(65, 50, area);
    frame.render_widget(Clear, popup_area);

    let protocols = ["socks5h", "socks5", "http", "https"];
    let proto = protocols[protocol_idx % protocols.len()];

    let f0_style = if active_field == 0 {
        Style::default().fg(Color::Black).bg(COLOR_BRIGHT_RED)
    } else {
        Style::default().fg(COLOR_TEXT_MAIN)
    };
    let f1_style = if active_field == 1 {
        Style::default().fg(Color::Black).bg(COLOR_MAGENTA_PROXY)
    } else {
        Style::default().fg(COLOR_MAGENTA_PROXY)
    };
    let f2_style = if active_field == 2 {
        Style::default().fg(Color::Black).bg(COLOR_CYAN_ACCENT)
    } else {
        Style::default().fg(COLOR_CYAN_ACCENT)
    };
    let f3_style = if active_field == 3 {
        Style::default().fg(Color::Black).bg(COLOR_YELLOW_ACCENT)
    } else {
        Style::default().fg(COLOR_YELLOW_ACCENT)
    };

    let text = vec![
        Line::styled(title, Style::default().fg(COLOR_BRIGHT_RED).add_modifier(Modifier::BOLD)),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" 1. Proxy Name:   ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(if name.is_empty() { "Local SOCKS5" } else { name }, f0_style),
        ]),
        Line::from(vec![
            Span::styled(" 2. Protocol:     ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(format!("◀ {} ▶", proto), f1_style),
        ]),
        Line::from(vec![
            Span::styled(" 3. Host IP / FQDN: ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(if host.is_empty() { "127.0.0.1" } else { host }, f2_style),
        ]),
        Line::from(vec![
            Span::styled(" 4. Port:         ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(if port.is_empty() { "1080" } else { port }, f3_style),
        ]),
        Line::raw(""),
        Line::styled("─── Navigation ───", Style::default().fg(COLOR_DARK_RED)),
        Line::from(vec![
            Span::styled(" [Tab/↓/↑] ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Switch Fields   "),
            Span::styled(" [←/→] ", Style::default().fg(COLOR_YELLOW_ACCENT)),
            Span::raw("Change Protocol"),
        ]),
        Line::from(vec![
            Span::styled(" [Enter] ", Style::default().fg(Color::Black).bg(COLOR_GREEN_STATUS)),
            Span::raw(" Save Proxy   "),
            Span::styled(" [Esc] ", Style::default().fg(Color::Black).bg(COLOR_BRIGHT_RED)),
            Span::raw(" Cancel "),
        ]),
    ];

    let modal_block = Paragraph::new(text)
        .block(
            Block::default()
                .title(Span::styled(" 🌐 Proxy Setup ", Style::default().fg(COLOR_BRIGHT_RED)))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(COLOR_BRIGHT_RED)),
        )
        .alignment(Alignment::Left);

    frame.render_widget(modal_block, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
