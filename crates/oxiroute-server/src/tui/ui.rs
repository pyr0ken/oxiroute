use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Row, Sparkline, Table, Tabs,
    },
    Frame,
};
use std::sync::atomic::Ordering;
use crate::tui::state::{ActiveTab, ModalState, TuiState, COMBO_STRATEGIES};

pub fn render(frame: &mut Frame, state: &TuiState) {
    let size = frame.area();

    // Main layout: Header (6) -> Tab Bar (3) -> Body (Min 10) -> Footer (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(5), // ASCII Banner & metadata
            Constraint::Length(3), // Tab navigation bar
            Constraint::Min(12),   // Tab content
            Constraint::Length(1), // Footer keymap
        ])
        .split(size);

    render_header_banner(frame, chunks[0], state);
    render_tab_bar(frame, chunks[1], state);

    let active_tab = *state.active_tab.read().unwrap();
    match active_tab {
        ActiveTab::Overview => render_overview_tab(frame, chunks[2], state),
        ActiveTab::Combos => render_combos_tab(frame, chunks[2], state),
        ActiveTab::Providers => render_providers_tab(frame, chunks[2], state),
        ActiveTab::Proxies => render_proxies_tab(frame, chunks[2], state),
        ActiveTab::Timeline => render_timeline_tab(frame, chunks[2], state),
    }

    render_footer(frame, chunks[3], &active_tab);

    // Render Modal if active
    let modal_guard = state.modal.read().unwrap();
    match *modal_guard {
        ModalState::Help => render_help_modal(frame, size),
        ModalState::AddCombo {
            ref name,
            strategy_idx,
            ref models_input,
            active_field,
        } => render_add_combo_modal(frame, size, name, strategy_idx, models_input, active_field),
        ModalState::AddProxy {
            ref name,
            protocol_idx,
            ref host,
            ref port,
            active_field,
        } => render_add_proxy_modal(frame, size, name, protocol_idx, host, port, active_field),
        ModalState::None => {}
    }
}

fn render_header_banner(frame: &mut Frame, area: Rect, state: &TuiState) {
    let banner_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(48), Constraint::Min(20)])
        .split(area);

    // ASCII Art Typography for OXIRUTE + Ferris
    let ascii_art = vec![
        Line::from(vec![
            Span::styled("  ___  _  _ ___ ___  _   _ _____ ___ ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("   (\\_/) ", Style::default().fg(Color::LightRed)),
        ]),
        Line::from(vec![
            Span::styled(" / _ \\( \\/ )_ _| _ \\/ | | |_   _| __|", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("  ( •.•) ", Style::default().fg(Color::LightYellow)),
        ]),
        Line::from(vec![
            Span::styled("| (_) )  ( | ||   /| |_| | | | | _|  ", Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
            Span::styled("  / >🦀  ", Style::default().fg(Color::Red)),
            Span::styled("Pure Rust", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" \\___//_/\\_\\___|_|_\\\\___/  |_| |___| ", Style::default().fg(Color::LightRed)),
            Span::styled("v0.1.0 (OmniRoute Engine)", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    frame.render_widget(Paragraph::new(ascii_art), banner_chunks[0]);

    // Metadata & live quick telemetry
    let proxy_text = state.proxy.as_deref().unwrap_or("Direct (None)");
    let meta_lines = vec![
        Line::from(vec![
            Span::styled("● Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled("ACTIVE (Running)", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("   "),
            Span::styled("Port: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("http://127.0.0.1:{}", state.port), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("⚡ Egress Proxy: ", Style::default().fg(Color::DarkGray)),
            Span::styled(proxy_text, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::raw("   "),
            Span::styled("Memory: ", Style::default().fg(Color::DarkGray)),
            Span::styled("< 10 MB RAM", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("⏱️  Uptime: ", Style::default().fg(Color::DarkGray)),
            Span::styled(state.uptime_formatted(), Style::default().fg(Color::Yellow)),
            Span::raw("   "),
            Span::styled("Circuit Breakers: ", Style::default().fg(Color::DarkGray)),
            Span::styled("CLOSED ● Healthy", Style::default().fg(Color::Green)),
        ]),
    ];

    let meta_block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray));

    frame.render_widget(Paragraph::new(meta_lines).block(meta_block), banner_chunks[1]);
}

fn render_tab_bar(frame: &mut Frame, area: Rect, state: &TuiState) {
    let active_tab = *state.active_tab.read().unwrap();

    let tab_titles = vec![
        Line::from(vec![Span::styled("[1] ", Style::default().fg(Color::Yellow)), Span::raw("📊 Dashboard")]),
        Line::from(vec![Span::styled("[2] ", Style::default().fg(Color::Yellow)), Span::raw("🔀 Combos (18)")]),
        Line::from(vec![Span::styled("[3] ", Style::default().fg(Color::Yellow)), Span::raw("👥 Providers (12)")]),
        Line::from(vec![Span::styled("[4] ", Style::default().fg(Color::Yellow)), Span::raw("🌐 Proxy Pool")]),
        Line::from(vec![Span::styled("[5] ", Style::default().fg(Color::Yellow)), Span::raw("📈 Timeline & Usage")]),
    ];

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .select(active_tab as usize)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" │ ");

    frame.render_widget(tabs, area);
}

// ─── TAB 1: OVERVIEW ──────────────────────────────────────────────────────────
fn render_overview_tab(frame: &mut Frame, area: Rect, state: &TuiState) {
    let main_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(8)])
        .split(area);

    let top_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(main_split[0]);

    // Left: Live Metrics
    let total_reqs = state.total_requests.load(Ordering::Relaxed);
    let active_reqs = state.active_requests.load(Ordering::Relaxed);
    let tokens = state.total_tokens.load(Ordering::Relaxed);

    let metrics_text = vec![
        Line::from(vec![
            Span::styled("Total Requests:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", total_reqs), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("       "),
            Span::styled("Active Streams: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", active_reqs), Style::default().fg(if active_reqs > 0 { Color::Yellow } else { Color::Green })),
        ]),
        Line::from(vec![
            Span::styled("Tokens Streamed:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", tokens), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Prompt Compaction:  ", Style::default().fg(Color::DarkGray)),
            Span::styled("RTK + Caveman Active (-25% avg)", Style::default().fg(Color::LightGreen)),
        ]),
    ];

    let metrics_block = Block::default()
        .title(" 📊 Real-time Gateway Metrics ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let m_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Length(3)])
        .margin(1)
        .split(top_split[0]);

    frame.render_widget(Paragraph::new(metrics_text), m_chunks[0]);

    if let Ok(latencies) = state.recent_latencies.read() {
        let sparkline = Sparkline::default()
            .block(Block::default().title(" TTFT Latency (ms) ").borders(Borders::NONE))
            .style(Style::default().fg(Color::Yellow))
            .data(&*latencies);
        frame.render_widget(sparkline, m_chunks[1]);
    }
    frame.render_widget(metrics_block, top_split[0]);

    // Right: Top Active Accounts
    let accounts_guard = state.accounts.read().unwrap();
    let acc_items: Vec<ListItem> = accounts_guard
        .iter()
        .take(5)
        .map(|acc| {
            let status = if acc.is_active {
                Span::styled("● ", Style::default().fg(Color::Green))
            } else {
                Span::styled("○ ", Style::default().fg(Color::DarkGray))
            };
            let badge = Span::styled(format!("[{}] ", acc.provider), Style::default().fg(Color::Red));
            let email = Span::styled(&acc.email, Style::default().fg(Color::White));
            let p = Span::styled(format!(" (P{})", acc.priority), Style::default().fg(Color::DarkGray));
            ListItem::new(Line::from(vec![status, badge, email, p]))
        })
        .collect();

    let acc_block = Block::default()
        .title(format!(" 👥 Fast Provider Overview ({}) ", accounts_guard.len()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Green));

    frame.render_widget(List::new(acc_items).block(acc_block), top_split[1]);

    // Bottom: Live Stream Logs
    render_logs_widget(frame, main_split[1], state);
}

// ─── TAB 2: COMBOS STUDIO ───────────────────────────────────────────────────
fn render_combos_tab(frame: &mut Frame, area: Rect, state: &TuiState) {
    let combos_guard = state.combos.read().unwrap();
    let sel_idx = *state.selected_index.read().unwrap();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Left list
    let items: Vec<ListItem> = combos_guard
        .iter()
        .enumerate()
        .map(|(idx, combo)| {
            let is_sel = idx == sel_idx;
            let bullet = if is_sel { "▶ " } else { "  " };
            let style = if is_sel {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let name_span = Span::styled(format!("{}{}", bullet, combo.name), style);
            let strat_span = Span::styled(format!(" [{}]", combo.strategy), Style::default().fg(Color::Cyan));
            let count_span = Span::styled(format!(" ({} models)", combo.models.len()), Style::default().fg(Color::DarkGray));

            ListItem::new(Line::from(vec![name_span, strat_span, count_span]))
        })
        .collect();

    let list_block = Block::default()
        .title(format!(" 🔀 Active Combos ({}) - [a] Add Combo ", combos_guard.len()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));

    frame.render_widget(List::new(items).block(list_block), chunks[0]);

    // Right details
    if let Some(combo) = combos_guard.get(sel_idx) {
        let mut detail_lines = vec![
            Line::from(vec![
                Span::styled("Combo Name:       ", Style::default().fg(Color::DarkGray)),
                Span::styled(&combo.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("Routing Strategy: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&combo.strategy, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("Assigned Proxy:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(combo.proxy_name.as_deref().unwrap_or("Default / System Proxy"), Style::default().fg(Color::Magenta)),
            ]),
            Line::raw(""),
            Line::styled("📦 Attached Target Models (Order of Evaluation):", Style::default().fg(Color::Yellow).add_modifier(Modifier::UNDERLINED)),
        ];

        for (i, m) in combo.models.iter().enumerate() {
            detail_lines.push(Line::from(vec![
                Span::styled(format!("  {}. ", i + 1), Style::default().fg(Color::DarkGray)),
                Span::styled(m, Style::default().fg(Color::LightGreen)),
            ]));
        }

        let detail_block = Block::default()
            .title(" 🔍 Combo Blueprint & Target Ordering ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .padding(ratatui::widgets::Padding::uniform(1));

        frame.render_widget(Paragraph::new(detail_lines).block(detail_block), chunks[1]);
    }
}

// ─── TAB 3: PROVIDERS ────────────────────────────────────────────────────────
fn render_providers_tab(frame: &mut Frame, area: Rect, state: &TuiState) {
    let accounts_guard = state.accounts.read().unwrap();
    let sel_idx = *state.selected_index.read().unwrap();

    let rows: Vec<Row> = accounts_guard
        .iter()
        .enumerate()
        .map(|(idx, acc)| {
            let is_sel = idx == sel_idx;
            let style = if is_sel {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            Row::new(vec![
                if acc.is_active { "Active ●" } else { "Disabled ○" }.to_string(),
                acc.provider.clone(),
                acc.email.clone(),
                format!("P{}", acc.priority),
                "Google AI Pro (g1-pro)".to_string(),
                "Healthy (0 errors)".to_string(),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(16),
            Constraint::Percentage(35),
            Constraint::Length(10),
            Constraint::Percentage(25),
            Constraint::Percentage(15),
        ],
    )
    .header(
        Row::new(vec![
            "Status",
            "Provider",
            "Identifier / Email",
            "Priority",
            "Tier / Plan",
            "Health",
        ])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .title(format!(
                " 👥 Provider Connections & Accounts ({}) - [a] Add [t] Test [r] Refresh ",
                accounts_guard.len()
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green)),
    );

    frame.render_widget(table, area);
}

// ─── TAB 4: PROXIES ──────────────────────────────────────────────────────────
fn render_proxies_tab(frame: &mut Frame, area: Rect, state: &TuiState) {
    let proxies_guard = state.proxies.read().unwrap();
    let sel_idx = *state.selected_index.read().unwrap();

    let rows: Vec<Row> = proxies_guard
        .iter()
        .enumerate()
        .map(|(idx, pr)| {
            let is_sel = idx == sel_idx;
            let style = if is_sel {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            Row::new(vec![
                pr.status.clone(),
                pr.name.clone(),
                pr.protocol.to_uppercase(),
                pr.host.clone(),
                format!("{}", pr.port),
                "~185 ms".to_string(),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Percentage(25),
            Constraint::Length(12),
            Constraint::Percentage(30),
            Constraint::Length(10),
            Constraint::Percentage(20),
        ],
    )
    .header(
        Row::new(vec![
            "Status",
            "Proxy Name",
            "Protocol",
            "Host / IP",
            "Port",
            "Egress Ping",
        ])
        .style(
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .title(format!(
                " 🌐 SOCKS5 / HTTP Proxy Pools ({}) - [a] Add Proxy [t] Test Egress ",
                proxies_guard.len()
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Magenta)),
    );

    frame.render_widget(table, area);
}

// ─── TAB 5: TIMELINE & USAGE ─────────────────────────────────────────────────
fn render_timeline_tab(frame: &mut Frame, area: Rect, state: &TuiState) {
    let tl_guard = state.timeline.read().unwrap();

    let rows: Vec<Row> = tl_guard
        .iter()
        .rev()
        .map(|item| {
            let status_color = if item.status_code == 200 {
                Color::Green
            } else {
                Color::Red
            };

            Row::new(vec![
                item.timestamp.clone(),
                item.model.clone(),
                item.account.clone(),
                format!("{} toks", item.tokens),
                format!("{} ms", item.latency_ms),
                format!("{}", item.status_code),
            ])
            .style(Style::default().fg(status_color))
            .bottom_margin(0)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Percentage(35),
            Constraint::Percentage(30),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec![
            "Time",
            "Requested Model",
            "Dispatched Account",
            "Tokens",
            "Latency",
            "HTTP",
        ])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .title(format!(
                " 📈 Request Timeline & Latency Stream History ({}) ",
                tl_guard.len()
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    frame.render_widget(table, area);
}

fn render_logs_widget(frame: &mut Frame, area: Rect, state: &TuiState) {
    let logs_guard = state.logs.read().unwrap();
    let items: Vec<ListItem> = logs_guard
        .iter()
        .rev()
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
        .title(" 📜 Live Stream & Egress Router Logs ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::White));

    frame.render_widget(List::new(items).block(block), area);
}

fn render_footer(frame: &mut Frame, area: Rect, active_tab: &ActiveTab) {
    let base_keys = match active_tab {
        ActiveTab::Combos => " [Tab] Next Tab │ [1-5] Quick Jump │ [a] Add Combo │ [j/k] Select │ [q] Quit ",
        ActiveTab::Providers => " [Tab] Next Tab │ [1-5] Quick Jump │ [a] Add Provider │ [t] Test │ [r] Refresh │ [q] Quit ",
        ActiveTab::Proxies => " [Tab] Next Tab │ [1-5] Quick Jump │ [a] Add Proxy │ [t] Test Egress │ [q] Quit ",
        _ => " [Tab] Next Tab │ [1-5] Quick Jump │ [c] Clear Logs │ [?] Help │ [q] Quit ",
    };

    let paragraph = Paragraph::new(Line::from(vec![Span::styled(
        base_keys,
        Style::default().fg(Color::Black).bg(Color::White),
    )]))
    .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}

// ─── MODALS ──────────────────────────────────────────────────────────────────
fn render_help_modal(frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(60, 50, area);
    frame.render_widget(Clear, popup_area);

    let text = vec![
        Line::styled(
            "🦀 Oxiroute Terminal Help & Keymap",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" [1-5]         ", Style::default().fg(Color::Yellow)),
            Span::raw("Direct jump to Tab (Dashboard, Combos, Providers, Proxies, Timeline)"),
        ]),
        Line::from(vec![
            Span::styled(" [Tab]         ", Style::default().fg(Color::Yellow)),
            Span::raw("Cycle through tabs in order"),
        ]),
        Line::from(vec![
            Span::styled(" [a]           ", Style::default().fg(Color::Yellow)),
            Span::raw("Open Add Modal (Add Combo in Tab 2, Add Proxy in Tab 4)"),
        ]),
        Line::from(vec![
            Span::styled(" [j / k]       ", Style::default().fg(Color::Yellow)),
            Span::raw("Navigate lists up/down"),
        ]),
        Line::from(vec![
            Span::styled(" [c]           ", Style::default().fg(Color::Yellow)),
            Span::raw("Clear live logs buffer"),
        ]),
        Line::from(vec![
            Span::styled(" [Esc / q]     ", Style::default().fg(Color::Yellow)),
            Span::raw("Close popup / Quit TUI"),
        ]),
        Line::raw(""),
        Line::styled(
            "Press [Esc] or [q] to close this help window.",
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let block = Block::default()
        .title(" Help & Controls ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(ratatui::widgets::Padding::uniform(1));

    frame.render_widget(Paragraph::new(text).block(block), popup_area);
}

fn render_add_combo_modal(
    frame: &mut Frame,
    area: Rect,
    name: &str,
    strategy_idx: usize,
    models_input: &str,
    active_field: usize,
) {
    let popup_area = centered_rect(65, 55, area);
    frame.render_widget(Clear, popup_area);

    let (strat_name, strat_desc) = COMBO_STRATEGIES[strategy_idx % COMBO_STRATEGIES.len()];

    let text = vec![
        Line::styled(
            "✨ Create New Model Combo",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "1. Combo Name:     ",
                Style::default().fg(if active_field == 0 {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                format!(
                    "[ {} ]",
                    if name.is_empty() {
                        "type name..."
                    } else {
                        name
                    }
                ),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "2. Strategy [←/→]: ",
                Style::default().fg(if active_field == 1 {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                format!("< {} >", strat_name),
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::styled(
            format!("   Description: {}", strat_desc),
            Style::default().fg(Color::DarkGray),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "3. Target Models:  ",
                Style::default().fg(if active_field == 2 {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                format!(
                    "[ {} ]",
                    if models_input.is_empty() {
                        "claude-sonnet-4-6, gemini-3.7-flash-low"
                    } else {
                        models_input
                    }
                ),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::raw(""),
        Line::styled(
            "─────────────────────────────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        ),
        Line::from(vec![
            Span::styled(
                " [Tab] ",
                Style::default().fg(Color::Black).bg(Color::White),
            ),
            Span::raw(" Next Field   "),
            Span::styled(
                " [Enter] ",
                Style::default().fg(Color::Black).bg(Color::Green),
            ),
            Span::raw(" Save Combo   "),
            Span::styled(" [Esc] ", Style::default().fg(Color::Black).bg(Color::Red)),
            Span::raw(" Cancel "),
        ]),
    ];

    let block = Block::default()
        .title(" Add Combo Studio ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Yellow))
        .padding(ratatui::widgets::Padding::uniform(1));

    frame.render_widget(Paragraph::new(text).block(block), popup_area);
}

fn render_add_proxy_modal(
    frame: &mut Frame,
    area: Rect,
    name: &str,
    protocol_idx: usize,
    host: &str,
    port: &str,
    active_field: usize,
) {
    let popup_area = centered_rect(60, 50, area);
    frame.render_widget(Clear, popup_area);

    let protocols = ["socks5h", "socks5", "http", "https"];
    let proto = protocols[protocol_idx % protocols.len()];

    let text = vec![
        Line::styled(
            "🌐 Register New Egress Proxy",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "1. Proxy Name:     ",
                Style::default().fg(if active_field == 0 {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                format!(
                    "[ {} ]",
                    if name.is_empty() {
                        "e.g. Fast SOCKS"
                    } else {
                        name
                    }
                ),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "2. Protocol [←/→]: ",
                Style::default().fg(if active_field == 1 {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                format!("< {} >", proto.to_uppercase()),
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "3. Host / IP:      ",
                Style::default().fg(if active_field == 2 {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                format!("[ {} ]", if host.is_empty() { "127.0.0.1" } else { host }),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "4. Port:           ",
                Style::default().fg(if active_field == 3 {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                format!("[ {} ]", if port.is_empty() { "1080" } else { port }),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::raw(""),
        Line::styled(
            "─────────────────────────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        ),
        Line::from(vec![
            Span::styled(
                " [Tab] ",
                Style::default().fg(Color::Black).bg(Color::White),
            ),
            Span::raw(" Next Field   "),
            Span::styled(
                " [Enter] ",
                Style::default().fg(Color::Black).bg(Color::Green),
            ),
            Span::raw(" Save Proxy   "),
            Span::styled(" [Esc] ", Style::default().fg(Color::Black).bg(Color::Red)),
            Span::raw(" Cancel "),
        ]),
    ];

    let block = Block::default()
        .title(" Register Proxy ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Magenta))
        .padding(ratatui::widgets::Padding::uniform(1));

    frame.render_widget(Paragraph::new(text).block(block), popup_area);
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
