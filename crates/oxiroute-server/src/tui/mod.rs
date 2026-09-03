pub mod state;
pub mod ui;

pub use state::{
    find_model_suggestion, AccountDisplay, ActiveTab, ComboDisplay, ModalState, ProxyDisplay,
    TuiState, COMBO_STRATEGIES,
};

use crossterm::{
    event::{self, Event as CEvent, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::Arc;
use std::time::Duration;

pub fn run_tui(state: Arc<TuiState>) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(150);

    loop {
        terminal.draw(|f| ui::render(f, &state))?;

        if event::poll(tick_rate)? {
            if let CEvent::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                let is_modal_open = {
                    let m = state.modal.read().unwrap();
                    !matches!(*m, ModalState::None)
                };

                if is_modal_open {
                    handle_modal_key(&state, key.code);
                } else {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('?') | KeyCode::Char('h') => {
                            state.open_modal(ModalState::Help);
                        }
                        // Tab / Shift+Tab navigation
                        KeyCode::Tab => {
                            let curr = *state.active_tab.read().unwrap();
                            state.set_tab(curr.next());
                        }
                        KeyCode::BackTab => {
                            let curr = *state.active_tab.read().unwrap();
                            state.set_tab(curr.prev());
                        }
                        KeyCode::Char('1') => state.set_tab(ActiveTab::Overview),
                        KeyCode::Char('2') => state.set_tab(ActiveTab::Combos),
                        KeyCode::Char('3') => state.set_tab(ActiveTab::Providers),
                        KeyCode::Char('4') => state.set_tab(ActiveTab::Proxies),
                        KeyCode::Char('5') => state.set_tab(ActiveTab::Timeline),
                        KeyCode::Char('j') | KeyCode::Down => {
                            let mut sel = state.selected_index.write().unwrap();
                            *sel = sel.saturating_add(1);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            let mut sel = state.selected_index.write().unwrap();
                            *sel = sel.saturating_sub(1);
                        }
                        KeyCode::Char('c') => {
                            if let Ok(mut logs) = state.logs.write() {
                                logs.clear();
                            }
                        }
                        KeyCode::Char('a') => {
                            let tab = *state.active_tab.read().unwrap();
                            match tab {
                                ActiveTab::Combos => {
                                    state.open_modal(ModalState::AddCombo {
                                        name: String::new(),
                                        strategy_idx: 0,
                                        models_input: String::new(),
                                        active_field: 0,
                                    });
                                }
                                ActiveTab::Proxies => {
                                    state.open_modal(ModalState::AddProxy {
                                        name: String::new(),
                                        protocol_idx: 0,
                                        host: "127.0.0.1".to_string(),
                                        port: "1080".to_string(),
                                        active_field: 0,
                                    });
                                }
                                _ => {}
                            }
                        }
                        KeyCode::Char('e') => {
                            let tab = *state.active_tab.read().unwrap();
                            let sel_idx = *state.selected_index.read().unwrap();
                            match tab {
                                ActiveTab::Combos => {
                                    let combos_guard = state.combos.read().unwrap();
                                    if let Some(combo) = combos_guard.get(sel_idx) {
                                        let strat_idx = COMBO_STRATEGIES
                                            .iter()
                                            .position(|(s, _)| *s == combo.strategy)
                                            .unwrap_or(0);
                                        state.open_modal(ModalState::EditCombo {
                                            combo_id: combo.id.clone(),
                                            name: combo.name.clone(),
                                            strategy_idx: strat_idx,
                                            models_input: combo.models.join(", "),
                                            active_field: 0,
                                        });
                                    }
                                }
                                ActiveTab::Proxies => {
                                    let proxies_guard = state.proxies.read().unwrap();
                                    if let Some(proxy) = proxies_guard.get(sel_idx) {
                                        let protocols = ["socks5h", "socks5", "http", "https"];
                                        let proto_idx = protocols
                                            .iter()
                                            .position(|p| *p == proxy.protocol)
                                            .unwrap_or(0);
                                        state.open_modal(ModalState::EditProxy {
                                            proxy_id: proxy.id.clone(),
                                            name: proxy.name.clone(),
                                            protocol_idx: proto_idx,
                                            host: proxy.host.clone(),
                                            port: format!("{}", proxy.port),
                                            active_field: 0,
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }
                        KeyCode::Char('d') | KeyCode::Delete => {
                            let tab = *state.active_tab.read().unwrap();
                            let sel_idx = *state.selected_index.read().unwrap();
                            match tab {
                                ActiveTab::Combos => {
                                    let mut combos_guard = state.combos.write().unwrap();
                                    if sel_idx < combos_guard.len() {
                                        let removed = combos_guard.remove(sel_idx);
                                        state.add_log(
                                            "WARN",
                                            format!("Deleted combo: '{}'", removed.name),
                                        );
                                    }
                                }
                                ActiveTab::Proxies => {
                                    let mut proxies_guard = state.proxies.write().unwrap();
                                    if sel_idx < proxies_guard.len() {
                                        let removed = proxies_guard.remove(sel_idx);
                                        state.add_log(
                                            "WARN",
                                            format!("Deleted proxy: '{}'", removed.name),
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn handle_modal_key(state: &TuiState, code: KeyCode) {
    let current_modal = {
        let guard = state.modal.read().unwrap();
        guard.clone()
    };

    let next_modal = match current_modal {
        ModalState::Help => {
            if matches!(code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter) {
                ModalState::None
            } else {
                ModalState::Help
            }
        }
        ModalState::AddCombo {
            mut name,
            mut strategy_idx,
            mut models_input,
            mut active_field,
        } => handle_combo_step(
            state,
            None,
            &mut name,
            &mut strategy_idx,
            &mut models_input,
            &mut active_field,
            code,
        ),
        ModalState::EditCombo {
            combo_id,
            mut name,
            mut strategy_idx,
            mut models_input,
            mut active_field,
        } => handle_combo_step(
            state,
            Some(combo_id),
            &mut name,
            &mut strategy_idx,
            &mut models_input,
            &mut active_field,
            code,
        ),
        ModalState::AddProxy {
            mut name,
            mut protocol_idx,
            mut host,
            mut port,
            mut active_field,
        } => handle_proxy_step(
            state,
            None,
            &mut name,
            &mut protocol_idx,
            &mut host,
            &mut port,
            &mut active_field,
            code,
        ),
        ModalState::EditProxy {
            proxy_id,
            mut name,
            mut protocol_idx,
            mut host,
            mut port,
            mut active_field,
        } => handle_proxy_step(
            state,
            Some(proxy_id),
            &mut name,
            &mut protocol_idx,
            &mut host,
            &mut port,
            &mut active_field,
            code,
        ),
        ModalState::None => ModalState::None,
    };

    *state.modal.write().unwrap() = next_modal;
}

fn handle_combo_step(
    state: &TuiState,
    combo_id: Option<String>,
    name: &mut String,
    strategy_idx: &mut usize,
    models_input: &mut String,
    active_field: &mut usize,
    code: KeyCode,
) -> ModalState {
    match code {
        KeyCode::Esc => ModalState::None,
        KeyCode::Tab => {
            if *active_field == 2 {
                if let Some(sug) = find_model_suggestion(models_input) {
                    models_input.push_str(sug);
                    return ModalState::AddCombo {
                        name: name.clone(),
                        strategy_idx: *strategy_idx,
                        models_input: models_input.clone(),
                        active_field: *active_field,
                    };
                }
            }
            *active_field = (*active_field + 1) % 3;
            if let Some(id) = combo_id {
                ModalState::EditCombo {
                    combo_id: id,
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddCombo {
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::BackTab | KeyCode::Up => {
            *active_field = (*active_field + 2) % 3;
            if let Some(id) = combo_id {
                ModalState::EditCombo {
                    combo_id: id,
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddCombo {
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::Down => {
            *active_field = (*active_field + 1) % 3;
            if let Some(id) = combo_id {
                ModalState::EditCombo {
                    combo_id: id,
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddCombo {
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::Right => {
            if *active_field == 1 {
                *strategy_idx = (*strategy_idx + 1) % COMBO_STRATEGIES.len();
            } else if *active_field == 2 {
                if let Some(sug) = find_model_suggestion(models_input) {
                    models_input.push_str(sug);
                }
            }
            if let Some(id) = combo_id {
                ModalState::EditCombo {
                    combo_id: id,
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddCombo {
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::Left => {
            if *active_field == 1 {
                *strategy_idx = if *strategy_idx == 0 {
                    COMBO_STRATEGIES.len() - 1
                } else {
                    *strategy_idx - 1
                };
            }
            if let Some(id) = combo_id {
                ModalState::EditCombo {
                    combo_id: id,
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddCombo {
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::Char(c) => {
            match *active_field {
                0 => name.push(c),
                2 => models_input.push(c),
                _ => {}
            }
            if let Some(id) = combo_id {
                ModalState::EditCombo {
                    combo_id: id,
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddCombo {
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::Backspace => {
            match *active_field {
                0 => {
                    name.pop();
                }
                2 => {
                    models_input.pop();
                }
                _ => {}
            }
            if let Some(id) = combo_id {
                ModalState::EditCombo {
                    combo_id: id,
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddCombo {
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::Enter => {
            if !name.trim().is_empty() {
                let strat_name = COMBO_STRATEGIES[*strategy_idx % COMBO_STRATEGIES.len()].0;
                let models: Vec<String> = models_input
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                let final_id = combo_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let new_combo = ComboDisplay {
                    id: final_id.clone(),
                    name: name.trim().to_string(),
                    strategy: strat_name.to_string(),
                    models: if models.is_empty() {
                        vec!["antigravity/claude-sonnet-4-6".to_string()]
                    } else {
                        models
                    },
                    proxy_name: None,
                };

                if let Ok(mut combos) = state.combos.write() {
                    if let Some(pos) = combos.iter().position(|c| c.id == final_id) {
                        combos[pos] = new_combo;
                        state.add_log(
                            "INFO",
                            format!("Updated combo: '{}' [{}]", name, strat_name),
                        );
                    } else {
                        combos.push(new_combo);
                        state.add_log(
                            "INFO",
                            format!("Created new combo: '{}' [{}]", name, strat_name),
                        );
                    }
                }
            }
            ModalState::None
        }
        _ => {
            if let Some(id) = combo_id {
                ModalState::EditCombo {
                    combo_id: id,
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddCombo {
                    name: name.clone(),
                    strategy_idx: *strategy_idx,
                    models_input: models_input.clone(),
                    active_field: *active_field,
                }
            }
        }
    }
}

fn handle_proxy_step(
    state: &TuiState,
    proxy_id: Option<String>,
    name: &mut String,
    protocol_idx: &mut usize,
    host: &mut String,
    port: &mut String,
    active_field: &mut usize,
    code: KeyCode,
) -> ModalState {
    match code {
        KeyCode::Esc => ModalState::None,
        KeyCode::Tab | KeyCode::Down => {
            *active_field = (*active_field + 1) % 4;
            if let Some(id) = proxy_id {
                ModalState::EditProxy {
                    proxy_id: id,
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddProxy {
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::BackTab | KeyCode::Up => {
            *active_field = (*active_field + 3) % 4;
            if let Some(id) = proxy_id {
                ModalState::EditProxy {
                    proxy_id: id,
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddProxy {
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::Left => {
            if *active_field == 1 {
                *protocol_idx = if *protocol_idx == 0 {
                    3
                } else {
                    *protocol_idx - 1
                };
            }
            if let Some(id) = proxy_id {
                ModalState::EditProxy {
                    proxy_id: id,
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddProxy {
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::Right => {
            if *active_field == 1 {
                *protocol_idx = (*protocol_idx + 1) % 4;
            }
            if let Some(id) = proxy_id {
                ModalState::EditProxy {
                    proxy_id: id,
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddProxy {
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::Char(c) => {
            match *active_field {
                0 => name.push(c),
                2 => host.push(c),
                3 => port.push(c),
                _ => {}
            }
            if let Some(id) = proxy_id {
                ModalState::EditProxy {
                    proxy_id: id,
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddProxy {
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::Backspace => {
            match *active_field {
                0 => {
                    name.pop();
                }
                2 => {
                    host.pop();
                }
                3 => {
                    port.pop();
                }
                _ => {}
            }
            if let Some(id) = proxy_id {
                ModalState::EditProxy {
                    proxy_id: id,
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddProxy {
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            }
        }
        KeyCode::Enter => {
            if !host.trim().is_empty() {
                let protocols = ["socks5h", "socks5", "http", "https"];
                let proto = protocols[*protocol_idx % protocols.len()];
                let p_num: u16 = port.parse().unwrap_or(1080);

                let final_id = proxy_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let new_proxy = ProxyDisplay {
                    id: final_id.clone(),
                    name: if name.trim().is_empty() {
                        format!("{}:{}", host, p_num)
                    } else {
                        name.trim().to_string()
                    },
                    protocol: proto.to_string(),
                    host: host.trim().to_string(),
                    port: p_num,
                    status: "Active ●".to_string(),
                };

                if let Ok(mut proxies) = state.proxies.write() {
                    if let Some(pos) = proxies.iter().position(|p| p.id == final_id) {
                        proxies[pos] = new_proxy;
                        state.add_log(
                            "INFO",
                            format!("Updated proxy: {}://{}:{}", proto, host, p_num),
                        );
                    } else {
                        proxies.push(new_proxy);
                        state.add_log(
                            "INFO",
                            format!("Registered proxy: {}://{}:{}", proto, host, p_num),
                        );
                    }
                }
            }
            ModalState::None
        }
        _ => {
            if let Some(id) = proxy_id {
                ModalState::EditProxy {
                    proxy_id: id,
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            } else {
                ModalState::AddProxy {
                    name: name.clone(),
                    protocol_idx: *protocol_idx,
                    host: host.clone(),
                    port: port.clone(),
                    active_field: *active_field,
                }
            }
        }
    }
}
