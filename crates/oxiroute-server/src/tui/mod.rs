pub mod state;
pub mod ui;

pub use state::{AccountDisplay, ActiveTab, ComboDisplay, ModalState, ProxyDisplay, TuiState, COMBO_STRATEGIES};

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
                        KeyCode::Tab => {
                            let curr = *state.active_tab.read().unwrap();
                            state.set_tab(curr.next());
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
    let mut modal_guard = state.modal.write().unwrap();

    match *modal_guard {
        ModalState::Help => {
            if matches!(code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter) {
                *modal_guard = ModalState::None;
            }
        }
        ModalState::AddCombo {
            ref mut name,
            ref mut strategy_idx,
            ref mut models_input,
            ref mut active_field,
        } => match code {
            KeyCode::Esc => {
                *modal_guard = ModalState::None;
            }
            KeyCode::Tab | KeyCode::Down => {
                *active_field = (*active_field + 1) % 3;
            }
            KeyCode::Up => {
                *active_field = (*active_field + 2) % 3;
            }
            KeyCode::Left => {
                if *active_field == 1 {
                    *strategy_idx = if *strategy_idx == 0 {
                        COMBO_STRATEGIES.len() - 1
                    } else {
                        *strategy_idx - 1
                    };
                }
            }
            KeyCode::Right => {
                if *active_field == 1 {
                    *strategy_idx = (*strategy_idx + 1) % COMBO_STRATEGIES.len();
                }
            }
            KeyCode::Char(c) => match *active_field {
                0 => name.push(c),
                2 => models_input.push(c),
                _ => {}
            },
            KeyCode::Backspace => match *active_field {
                0 => {
                    name.pop();
                }
                2 => {
                    models_input.pop();
                }
                _ => {}
            },
            KeyCode::Enter => {
                if !name.trim().is_empty() {
                    let strat_name = COMBO_STRATEGIES[*strategy_idx % COMBO_STRATEGIES.len()].0;
                    let models: Vec<String> = models_input
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();

                    let new_combo = ComboDisplay {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: name.trim().to_string(),
                        strategy: strat_name.to_string(),
                        models: if models.is_empty() {
                            vec!["claude-sonnet-4-6".to_string()]
                        } else {
                            models
                        },
                        proxy_name: None,
                    };

                    if let Ok(mut combos) = state.combos.write() {
                        combos.push(new_combo);
                    }
                    state.add_log("INFO", format!("Created new combo: '{}' [{}]", name, strat_name));
                }
                *modal_guard = ModalState::None;
            }
            _ => {}
        },
        ModalState::AddProxy {
            ref mut name,
            ref mut protocol_idx,
            ref mut host,
            ref mut port,
            ref mut active_field,
        } => match code {
            KeyCode::Esc => {
                *modal_guard = ModalState::None;
            }
            KeyCode::Tab | KeyCode::Down => {
                *active_field = (*active_field + 1) % 4;
            }
            KeyCode::Up => {
                *active_field = (*active_field + 3) % 4;
            }
            KeyCode::Left => {
                if *active_field == 1 {
                    *protocol_idx = if *protocol_idx == 0 { 3 } else { *protocol_idx - 1 };
                }
            }
            KeyCode::Right => {
                if *active_field == 1 {
                    *protocol_idx = (*protocol_idx + 1) % 4;
                }
            }
            KeyCode::Char(c) => match *active_field {
                0 => name.push(c),
                2 => host.push(c),
                3 => port.push(c),
                _ => {}
            },
            KeyCode::Backspace => match *active_field {
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
            },
            KeyCode::Enter => {
                if !host.trim().is_empty() {
                    let protocols = ["socks5h", "socks5", "http", "https"];
                    let proto = protocols[*protocol_idx % protocols.len()];
                    let p_num: u16 = port.parse().unwrap_or(1080);

                    let new_proxy = ProxyDisplay {
                        id: uuid::Uuid::new_v4().to_string(),
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
                        proxies.push(new_proxy);
                    }
                    state.add_log("INFO", format!("Registered new proxy: {}://{}:{}", proto, host, p_num));
                }
                *modal_guard = ModalState::None;
            }
            _ => {}
        },
        ModalState::None => {}
    }
}
