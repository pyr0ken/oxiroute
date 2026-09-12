pub mod state;
pub mod ui;

pub use state::{
    AccountDisplay, ActiveTab, ComboDisplay, ModalState, ProxyDisplay, TuiState, COMBO_STRATEGIES,
    KNOWN_MODELS, PROVIDER_TYPES,
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

pub fn run_tui(
    state: Arc<TuiState>,
    auth_mgr: Arc<oxiroute_auth::AntigravityAuthManager>,
) -> Result<(), Box<dyn std::error::Error>> {
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
                                        models: vec!["antigravity/claude-sonnet-4-6".to_string()],
                                        proxy_idx: 0,
                                        active_field: 0,
                                    });
                                }
                                ActiveTab::Providers => {
                                    state.open_modal(ModalState::AddProvider {
                                        provider_type_idx: 0,
                                        name: String::new(),
                                        api_key: String::new(),
                                        priority: 0,
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
                        KeyCode::Enter | KeyCode::Char('e') => {
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

                                        let proxies_guard = state.proxies.read().unwrap();
                                        let p_idx = if let Some(ref pname) = combo.proxy_name {
                                            proxies_guard
                                                .iter()
                                                .position(|p| p.name == *pname)
                                                .map(|i| i + 1)
                                                .unwrap_or(0)
                                        } else {
                                            0
                                        };

                                        state.open_modal(ModalState::EditCombo {
                                            combo_id: combo.id.clone(),
                                            name: combo.name.clone(),
                                            strategy_idx: strat_idx,
                                            models: combo.models.clone(),
                                            proxy_idx: p_idx,
                                            active_field: 0,
                                        });
                                    }
                                }
                                ActiveTab::Providers => {
                                    let accounts_guard = state.accounts.read().unwrap();
                                    if let Some(acc) = accounts_guard.get(sel_idx) {
                                        let p_idx = PROVIDER_TYPES
                                            .iter()
                                            .position(|(p, _)| *p == acc.provider)
                                            .unwrap_or(0);
                                        let prio = (acc.priority.saturating_sub(1)) as usize % 3;

                                        state.open_modal(ModalState::EditProvider {
                                            account_id: acc.id.clone(),
                                            provider_type_idx: p_idx,
                                            name: acc.name.clone(),
                                            api_key: String::new(),
                                            priority: prio,
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
                        KeyCode::Char('t') => {
                            let tab = *state.active_tab.read().unwrap();
                            if tab == ActiveTab::Combos {
                                let sel_idx = *state.selected_index.read().unwrap();
                                let target_combo = {
                                    let mut combos_guard = state.combos.write().unwrap();
                                    if let Some(c) = combos_guard.get_mut(sel_idx) {
                                        c.status_text = Some("Testing... ⏳".to_string());
                                        Some((c.id.clone(), c.name.clone(), c.models.clone(), c.proxy_name.clone()))
                                    } else {
                                        None
                                    }
                                };

                                if let Some((cid, cname, models, p_name)) = target_combo {
                                    let state_clone = state.clone();
                                    let auth_clone = auth_mgr.clone();
                                    tokio::spawn(async move {
                                        // Resolve proxy URL for this combo:
                                        // 1. Combo specific assigned proxy from proxy_registry
                                        // 2. Global --proxy arg fallback
                                        let resolved_proxy = if let Some(ref pname) = p_name {
                                            let proxies_guard = state_clone.proxies.read().unwrap();
                                            proxies_guard
                                                .iter()
                                                .find(|p| p.name == *pname || p.id == *pname)
                                                .map(|p| format!("{}://{}:{}", p.protocol, p.host, p.port))
                                                .or_else(|| state_clone.proxy.clone())
                                        } else {
                                            state_clone.proxy.clone()
                                        };

                                        state_clone.add_log(
                                            "INFO",
                                            format!(
                                                "Testing combo '{}' (models: {}, proxy: {})",
                                                cname,
                                                models.join(", "),
                                                resolved_proxy.as_deref().unwrap_or("Direct")
                                            ),
                                        );

                                        // Test the first active Google/Antigravity account with the combo's proxy
                                        let first_acc = auth_clone.list_accounts().iter().find(|a| a.is_active).cloned();
                                        if let Some(acc) = first_acc {
                                            match oxiroute_auth::AntigravityAuthManager::test_account_connection(
                                                &acc,
                                                resolved_proxy.as_deref(),
                                            )
                                            .await
                                            {
                                                Ok(latency) => {
                                                    if let Ok(mut combos_guard) = state_clone.combos.write() {
                                                        if let Some(c) = combos_guard.iter_mut().find(|c| c.id == cid) {
                                                            c.status_text = Some(format!("Healthy ● ({}ms)", latency));
                                                        }
                                                    }
                                                    state_clone.add_log(
                                                        "INFO",
                                                        format!(
                                                            "Combo '{}' verified healthy ({}ms latency) via {}",
                                                            cname,
                                                            latency,
                                                            resolved_proxy.as_deref().unwrap_or("Direct")
                                                        ),
                                                    );
                                                }
                                                Err(err) => {
                                                    if let Ok(mut combos_guard) = state_clone.combos.write() {
                                                        if let Some(c) = combos_guard.iter_mut().find(|c| c.id == cid) {
                                                            c.status_text = Some("Failed ✕".to_string());
                                                        }
                                                    }
                                                    state_clone.add_log(
                                                        "ERROR",
                                                        format!("Combo '{}' test failed: {}", cname, err),
                                                    );
                                                }
                                            }
                                        } else {
                                            if let Ok(mut combos_guard) = state_clone.combos.write() {
                                                if let Some(c) = combos_guard.iter_mut().find(|c| c.id == cid) {
                                                    c.status_text = Some("No Accounts ✕".to_string());
                                                }
                                            }
                                            state_clone.add_log(
                                                "WARN",
                                                format!("No active accounts available to test combo '{}'", cname),
                                            );
                                        }
                                    });
                                }
                            } else if tab == ActiveTab::Providers {
                                let sel_idx = *state.selected_index.read().unwrap();
                                let target_account = {
                                    let mut accounts_guard = state.accounts.write().unwrap();
                                    if let Some(acc) = accounts_guard.get_mut(sel_idx) {
                                        acc.status_text = "Testing... ⏳".to_string();
                                        Some((acc.id.clone(), acc.email.clone(), acc.provider.clone()))
                                    } else {
                                        None
                                    }
                                };

                                if let Some((acc_id, email, provider)) = target_account {
                                    let state_clone = state.clone();
                                    let auth_clone = auth_mgr.clone();
                                    tokio::spawn(async move {
                                        state_clone.add_log(
                                            "INFO",
                                            format!(
                                                "Dispatching live network probe for provider '{}' [{}] (Proxy: {})",
                                                email,
                                                provider,
                                                state_clone.proxy.as_deref().unwrap_or("Direct")
                                            ),
                                        );

                                        let is_google = provider == "antigravity" || provider == "agy";
                                        let acc_opt = auth_clone
                                            .list_accounts()
                                            .iter()
                                            .find(|a| a.id == acc_id || a.email == email)
                                            .cloned();

                                        if is_google && acc_opt.is_some() {
                                            let acc = acc_opt.unwrap();
                                            match oxiroute_auth::AntigravityAuthManager::test_account_connection(
                                                &acc,
                                                state_clone.proxy.as_deref(),
                                            )
                                            .await
                                            {
                                                Ok(latency) => {
                                                    if let Ok(mut accounts_guard) = state_clone.accounts.write() {
                                                        if let Some(a) = accounts_guard.iter_mut().find(|a| a.id == acc_id) {
                                                            a.status_text = format!("Verified ● ({}ms)", latency);
                                                        }
                                                    }
                                                    state_clone.add_log(
                                                        "INFO",
                                                        format!(
                                                            "Real probe success: '{}' -> 200 OK ({}ms latency)",
                                                            email, latency
                                                        ),
                                                    );
                                                }
                                                Err(err) => {
                                                    if let Ok(mut accounts_guard) = state_clone.accounts.write() {
                                                        if let Some(a) = accounts_guard.iter_mut().find(|a| a.id == acc_id) {
                                                            a.status_text = "Failed ✕".to_string();
                                                        }
                                                    }
                                                    state_clone.add_log(
                                                        "ERROR",
                                                        format!("Real probe failed for '{}': {}", email, err),
                                                    );
                                                }
                                            }
                                        } else {
                                            // Non-OAuth provider or API key probe: test endpoint reachability via configured proxy
                                            let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(8));
                                            if let Some(ref p) = state_clone.proxy {
                                                if let Ok(proxy) = reqwest::Proxy::all(p) {
                                                    builder = builder.proxy(proxy);
                                                }
                                            }
                                            let client = builder.build().unwrap_or_default();
                                            let test_url = match provider.as_str() {
                                                "openrouter" => "https://openrouter.ai/api/v1/models",
                                                "deepseek" => "https://api.deepseek.com/models",
                                                "groq" => "https://api.groq.com/openai/v1/models",
                                                "openai" => "https://api.openai.com/v1/models",
                                                "anthropic" => "https://api.anthropic.com/v1/models",
                                                _ => "https://httpbin.org/status/200",
                                            };

                                            let start = std::time::Instant::now();
                                            match client.get(test_url).send().await {
                                                Ok(res) => {
                                                    let latency = start.elapsed().as_millis() as u64;
                                                    let status = res.status().as_u16();
                                                    let is_ok = res.status().is_success() || status == 401; // 401 means server reached and responding
                                                    if let Ok(mut accounts_guard) = state_clone.accounts.write() {
                                                        if let Some(a) = accounts_guard.iter_mut().find(|a| a.id == acc_id) {
                                                            a.status_text = if is_ok {
                                                                format!("Verified ● ({}ms)", latency)
                                                            } else {
                                                                format!("Status {}", status)
                                                            };
                                                        }
                                                    }
                                                    state_clone.add_log(
                                                        "INFO",
                                                        format!("Network probe for '{}' reached {} in {}ms", email, test_url, latency),
                                                    );
                                                }
                                                Err(e) => {
                                                    if let Ok(mut accounts_guard) = state_clone.accounts.write() {
                                                        if let Some(a) = accounts_guard.iter_mut().find(|a| a.id == acc_id) {
                                                            a.status_text = "Failed ✕".to_string();
                                                        }
                                                    }
                                                    state_clone.add_log(
                                                        "ERROR",
                                                        format!("Probe network error for '{}': {}", email, e),
                                                    );
                                                }
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Char('r') => {
                            let tab = *state.active_tab.read().unwrap();
                            if tab == ActiveTab::Providers {
                                let sel_idx = *state.selected_index.read().unwrap();
                                let target_email = {
                                    let mut accounts_guard = state.accounts.write().unwrap();
                                    if let Some(acc) = accounts_guard.get_mut(sel_idx) {
                                        acc.status_text = "Refreshing... ⏳".to_string();
                                        Some(acc.email.clone())
                                    } else {
                                        None
                                    }
                                };

                                if let Some(email) = target_email {
                                    let state_clone = state.clone();
                                    let auth_clone = auth_mgr.clone();
                                    tokio::spawn(async move {
                                        let acc_opt = auth_clone
                                            .list_accounts()
                                            .iter()
                                            .find(|a| a.email == email)
                                            .cloned();
                                        if let Some(acc) = acc_opt {
                                            let mut builder = reqwest::Client::builder();
                                            if let Some(ref p) = state_clone.proxy {
                                                if let Ok(proxy) = reqwest::Proxy::all(p) {
                                                    builder = builder.proxy(proxy);
                                                }
                                            }
                                            let client = builder.build().unwrap_or_default();
                                            state_clone.add_log(
                                                "INFO",
                                                format!("Dispatching live OAuth refresh for: {}", email),
                                            );
                                            match oxiroute_auth::AntigravityAuthManager::refresh_account_token(
                                                &acc,
                                                &client,
                                            )
                                            .await
                                            {
                                                Ok(_) => {
                                                    if let Ok(mut accounts_guard) = state_clone.accounts.write() {
                                                        if let Some(a) = accounts_guard.iter_mut().find(|a| a.email == email) {
                                                            a.status_text = "Refreshed ✓".to_string();
                                                        }
                                                    }
                                                    state_clone.add_log(
                                                        "INFO",
                                                        format!(
                                                            "Real OAuth refresh successful for '{}' (Valid 3600s)",
                                                            email
                                                        ),
                                                    );
                                                }
                                                Err(err) => {
                                                    if let Ok(mut accounts_guard) = state_clone.accounts.write() {
                                                        if let Some(a) = accounts_guard.iter_mut().find(|a| a.email == email) {
                                                            a.status_text = "Refresh Fail ✕".to_string();
                                                        }
                                                    }
                                                    state_clone.add_log(
                                                        "ERROR",
                                                        format!("Real OAuth refresh failed for '{}': {}", email, err),
                                                    );
                                                }
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Char(' ') => {
                            let tab = *state.active_tab.read().unwrap();
                            if tab == ActiveTab::Providers {
                                let sel_idx = *state.selected_index.read().unwrap();
                                let mut accounts_guard = state.accounts.write().unwrap();
                                if let Some(acc) = accounts_guard.get_mut(sel_idx) {
                                    acc.is_active = !acc.is_active;
                                    acc.status_text = if acc.is_active {
                                        "Active ●".to_string()
                                    } else {
                                        "Disabled ○".to_string()
                                    };
                                    let id = acc.id.clone();
                                    let name = acc.name.clone();
                                    let is_active = acc.is_active;
                                    drop(accounts_guard);

                                    if let Some(ref storage) = state.storage {
                                        let _ = storage.update_provider_active(&id, is_active);
                                    }
                                    state.add_log(
                                        "INFO",
                                        format!(
                                            "Toggled provider '{}' -> {} (Saved 💾)",
                                            name,
                                            if is_active { "Active" } else { "Disabled" }
                                        ),
                                    );
                                }
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
                                        if let Some(ref storage) = state.storage {
                                            let _ = storage.delete_combo(&removed.id);
                                        }
                                    }
                                }
                                ActiveTab::Providers => {
                                    let mut accounts_guard = state.accounts.write().unwrap();
                                    if sel_idx < accounts_guard.len() {
                                        let removed = accounts_guard.remove(sel_idx);
                                        state.add_log(
                                            "WARN",
                                            format!("Removed provider connection: '{}'", removed.name),
                                        );
                                        if let Some(ref storage) = state.storage {
                                            let _ = storage.delete_provider_connection(&removed.id);
                                        }
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
                                        if let Some(ref storage) = state.storage {
                                            let _ = storage.delete_proxy(&removed.id);
                                        }
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
            mut models,
            mut proxy_idx,
            mut active_field,
        } => handle_combo_step(
            state,
            None,
            &mut name,
            &mut strategy_idx,
            &mut models,
            &mut proxy_idx,
            &mut active_field,
            code,
        ),
        ModalState::EditCombo {
            combo_id,
            mut name,
            mut strategy_idx,
            mut models,
            mut proxy_idx,
            mut active_field,
        } => handle_combo_step(
            state,
            Some(combo_id),
            &mut name,
            &mut strategy_idx,
            &mut models,
            &mut proxy_idx,
            &mut active_field,
            code,
        ),
        ModalState::ModelPicker {
            return_to_edit,
            combo_id,
            combo_name,
            strategy_idx,
            proxy_idx,
            mut search_query,
            mut selected_models,
            mut cursor_idx,
        } => handle_model_picker_step(
            return_to_edit,
            combo_id,
            combo_name,
            strategy_idx,
            proxy_idx,
            &mut search_query,
            &mut selected_models,
            &mut cursor_idx,
            code,
        ),
        ModalState::AddProvider {
            mut provider_type_idx,
            mut name,
            mut api_key,
            mut priority,
            mut active_field,
        } => handle_provider_step(
            state,
            None,
            &mut provider_type_idx,
            &mut name,
            &mut api_key,
            &mut priority,
            &mut active_field,
            code,
        ),
        ModalState::EditProvider {
            account_id,
            mut provider_type_idx,
            mut name,
            mut api_key,
            mut priority,
            mut active_field,
        } => handle_provider_step(
            state,
            Some(account_id),
            &mut provider_type_idx,
            &mut name,
            &mut api_key,
            &mut priority,
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
    models: &mut Vec<String>,
    proxy_idx: &mut usize,
    active_field: &mut usize,
    code: KeyCode,
) -> ModalState {
    let num_proxies = {
        let guard = state.proxies.read().unwrap();
        guard.len()
    };
    let total_proxy_choices = num_proxies + 1; // 0 = None, 1..=num_proxies

    match code {
        KeyCode::Esc => ModalState::None,
        KeyCode::Tab | KeyCode::Down => {
            *active_field = (*active_field + 1) % 4;
            wrap_combo_modal(combo_id, name, *strategy_idx, models, *proxy_idx, *active_field)
        }
        KeyCode::BackTab | KeyCode::Up => {
            *active_field = (*active_field + 3) % 4;
            wrap_combo_modal(combo_id, name, *strategy_idx, models, *proxy_idx, *active_field)
        }
        KeyCode::Right => {
            if *active_field == 1 {
                *strategy_idx = (*strategy_idx + 1) % COMBO_STRATEGIES.len();
            } else if *active_field == 3 {
                *proxy_idx = (*proxy_idx + 1) % total_proxy_choices;
            }
            wrap_combo_modal(combo_id, name, *strategy_idx, models, *proxy_idx, *active_field)
        }
        KeyCode::Left => {
            if *active_field == 1 {
                *strategy_idx = if *strategy_idx == 0 {
                    COMBO_STRATEGIES.len() - 1
                } else {
                    *strategy_idx - 1
                };
            } else if *active_field == 3 {
                *proxy_idx = if *proxy_idx == 0 {
                    total_proxy_choices - 1
                } else {
                    *proxy_idx - 1
                };
            }
            wrap_combo_modal(combo_id, name, *strategy_idx, models, *proxy_idx, *active_field)
        }
        KeyCode::Char(' ') | KeyCode::Char('m') if *active_field == 2 => {
            // Open searchable ModelPicker
            ModalState::ModelPicker {
                return_to_edit: combo_id.is_some(),
                combo_id,
                combo_name: name.clone(),
                strategy_idx: *strategy_idx,
                proxy_idx: *proxy_idx,
                search_query: String::new(),
                selected_models: models.clone(),
                cursor_idx: 0,
            }
        }
        KeyCode::Char(c) if *active_field == 0 => {
            name.push(c);
            wrap_combo_modal(combo_id, name, *strategy_idx, models, *proxy_idx, *active_field)
        }
        KeyCode::Backspace if *active_field == 0 => {
            name.pop();
            wrap_combo_modal(combo_id, name, *strategy_idx, models, *proxy_idx, *active_field)
        }
        KeyCode::Enter => {
            if *active_field == 2 {
                // If enter on field 2, open model picker
                return ModalState::ModelPicker {
                    return_to_edit: combo_id.is_some(),
                    combo_id,
                    combo_name: name.clone(),
                    strategy_idx: *strategy_idx,
                    proxy_idx: *proxy_idx,
                    search_query: String::new(),
                    selected_models: models.clone(),
                    cursor_idx: 0,
                };
            }

            if !name.trim().is_empty() {
                let strat_name = COMBO_STRATEGIES[*strategy_idx % COMBO_STRATEGIES.len()].0;
                let final_id = combo_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                let selected_proxy_name = if *proxy_idx == 0 {
                    None
                } else {
                    let proxies_guard = state.proxies.read().unwrap();
                    proxies_guard.get(*proxy_idx - 1).map(|p| p.name.clone())
                };

                let new_combo = ComboDisplay {
                    id: final_id.clone(),
                    name: name.trim().to_string(),
                    strategy: strat_name.to_string(),
                    models: if models.is_empty() {
                        vec!["antigravity/claude-sonnet-4-6".to_string()]
                    } else {
                        models.clone()
                    },
                    proxy_name: selected_proxy_name,
                    status_text: None,
                };

                if let Some(ref storage) = state.storage {
                    let rec = oxiroute_auth::ComboRecord {
                        id: new_combo.id.clone(),
                        name: new_combo.name.clone(),
                        strategy: new_combo.strategy.clone(),
                        models: new_combo.models.clone(),
                        proxy_name: new_combo.proxy_name.clone(),
                    };
                    if let Err(e) = storage.save_combo(&rec) {
                        state.add_log("ERROR", format!("Failed to save combo to disk: {}", e));
                    }
                }

                if let Ok(mut combos) = state.combos.write() {
                    if let Some(pos) = combos.iter().position(|c| c.id == final_id) {
                        combos[pos] = new_combo;
                        state.add_log(
                            "INFO",
                            format!("Updated combo: '{}' [{}] (Saved 💾)", name, strat_name),
                        );
                    } else {
                        combos.push(new_combo);
                        state.add_log(
                            "INFO",
                            format!("Created new combo: '{}' [{}] (Saved 💾)", name, strat_name),
                        );
                    }
                }
            }
            ModalState::None
        }
        _ => wrap_combo_modal(combo_id, name, *strategy_idx, models, *proxy_idx, *active_field),
    }
}

fn wrap_combo_modal(
    combo_id: Option<String>,
    name: &str,
    strategy_idx: usize,
    models: &[String],
    proxy_idx: usize,
    active_field: usize,
) -> ModalState {
    if let Some(id) = combo_id {
        ModalState::EditCombo {
            combo_id: id,
            name: name.to_string(),
            strategy_idx,
            models: models.to_vec(),
            proxy_idx,
            active_field,
        }
    } else {
        ModalState::AddCombo {
            name: name.to_string(),
            strategy_idx,
            models: models.to_vec(),
            proxy_idx,
            active_field,
        }
    }
}

fn handle_model_picker_step(
    return_to_edit: bool,
    combo_id: Option<String>,
    combo_name: String,
    strategy_idx: usize,
    proxy_idx: usize,
    search_query: &mut String,
    selected_models: &mut Vec<String>,
    cursor_idx: &mut usize,
    code: KeyCode,
) -> ModalState {
    let query_lower = search_query.to_lowercase();
    let filtered_models: Vec<&str> = KNOWN_MODELS
        .iter()
        .filter(|(m, desc)| {
            if query_lower.is_empty() {
                true
            } else {
                m.to_lowercase().contains(&query_lower) || desc.to_lowercase().contains(&query_lower)
            }
        })
        .map(|(m, _)| *m)
        .collect();

    match code {
        KeyCode::Esc => {
            // Return to combo modal without saving changes
            wrap_combo_modal(combo_id, &combo_name, strategy_idx, selected_models, proxy_idx, 2)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !filtered_models.is_empty() {
                *cursor_idx = (*cursor_idx + 1) % filtered_models.len();
            }
            ModalState::ModelPicker {
                return_to_edit,
                combo_id,
                combo_name,
                strategy_idx,
                proxy_idx,
                search_query: search_query.clone(),
                selected_models: selected_models.clone(),
                cursor_idx: *cursor_idx,
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if !filtered_models.is_empty() {
                *cursor_idx = if *cursor_idx == 0 {
                    filtered_models.len() - 1
                } else {
                    *cursor_idx - 1
                };
            }
            ModalState::ModelPicker {
                return_to_edit,
                combo_id,
                combo_name,
                strategy_idx,
                proxy_idx,
                search_query: search_query.clone(),
                selected_models: selected_models.clone(),
                cursor_idx: *cursor_idx,
            }
        }
        KeyCode::Char(' ') => {
            // Toggle model selection
            if let Some(model_at_cursor) = filtered_models.get(*cursor_idx) {
                let m_str = model_at_cursor.to_string();
                if let Some(pos) = selected_models.iter().position(|m| *m == m_str) {
                    selected_models.remove(pos);
                } else {
                    selected_models.push(m_str);
                }
            }
            ModalState::ModelPicker {
                return_to_edit,
                combo_id,
                combo_name,
                strategy_idx,
                proxy_idx,
                search_query: search_query.clone(),
                selected_models: selected_models.clone(),
                cursor_idx: *cursor_idx,
            }
        }
        KeyCode::Char(c) => {
            search_query.push(c);
            *cursor_idx = 0;
            ModalState::ModelPicker {
                return_to_edit,
                combo_id,
                combo_name,
                strategy_idx,
                proxy_idx,
                search_query: search_query.clone(),
                selected_models: selected_models.clone(),
                cursor_idx: 0,
            }
        }
        KeyCode::Backspace => {
            search_query.pop();
            *cursor_idx = 0;
            ModalState::ModelPicker {
                return_to_edit,
                combo_id,
                combo_name,
                strategy_idx,
                proxy_idx,
                search_query: search_query.clone(),
                selected_models: selected_models.clone(),
                cursor_idx: 0,
            }
        }
        KeyCode::Enter => {
            // Confirm and return to combo modal with selected models
            wrap_combo_modal(combo_id, &combo_name, strategy_idx, selected_models, proxy_idx, 2)
        }
        _ => ModalState::ModelPicker {
            return_to_edit,
            combo_id,
            combo_name,
            strategy_idx,
            proxy_idx,
            search_query: search_query.clone(),
            selected_models: selected_models.clone(),
            cursor_idx: *cursor_idx,
        },
    }
}

fn handle_provider_step(
    state: &TuiState,
    account_id: Option<String>,
    provider_type_idx: &mut usize,
    name: &mut String,
    api_key: &mut String,
    priority: &mut usize,
    active_field: &mut usize,
    code: KeyCode,
) -> ModalState {
    match code {
        KeyCode::Esc => ModalState::None,
        KeyCode::Tab | KeyCode::Down => {
            *active_field = (*active_field + 1) % 4;
            wrap_provider_modal(account_id, *provider_type_idx, name, api_key, *priority, *active_field)
        }
        KeyCode::BackTab | KeyCode::Up => {
            *active_field = (*active_field + 3) % 4;
            wrap_provider_modal(account_id, *provider_type_idx, name, api_key, *priority, *active_field)
        }
        KeyCode::Right => {
            if *active_field == 0 {
                *provider_type_idx = (*provider_type_idx + 1) % PROVIDER_TYPES.len();
            } else if *active_field == 3 {
                *priority = (*priority + 1) % 3;
            }
            wrap_provider_modal(account_id, *provider_type_idx, name, api_key, *priority, *active_field)
        }
        KeyCode::Left => {
            if *active_field == 0 {
                *provider_type_idx = if *provider_type_idx == 0 {
                    PROVIDER_TYPES.len() - 1
                } else {
                    *provider_type_idx - 1
                };
            } else if *active_field == 3 {
                *priority = if *priority == 0 { 2 } else { *priority - 1 };
            }
            wrap_provider_modal(account_id, *provider_type_idx, name, api_key, *priority, *active_field)
        }
        KeyCode::Char(c) => {
            match *active_field {
                1 => name.push(c),
                2 => api_key.push(c),
                _ => {}
            }
            wrap_provider_modal(account_id, *provider_type_idx, name, api_key, *priority, *active_field)
        }
        KeyCode::Backspace => {
            match *active_field {
                1 => {
                    name.pop();
                }
                2 => {
                    api_key.pop();
                }
                _ => {}
            }
            wrap_provider_modal(account_id, *provider_type_idx, name, api_key, *priority, *active_field)
        }
        KeyCode::Enter => {
            let ptype = PROVIDER_TYPES[*provider_type_idx % PROVIDER_TYPES.len()].0;
            let display_name = if name.trim().is_empty() {
                format!("{} Main", ptype)
            } else {
                name.trim().to_string()
            };
            let final_id = account_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let mut enc_token = None;
            if !api_key.trim().is_empty() {
                if let Some(ref storage) = state.storage {
                    if let Ok(k) = storage.get_encryption_key() {
                        enc_token = oxiroute_auth::crypto::encrypt_field(api_key.trim(), &k).ok();
                    }
                }
                if enc_token.is_none() {
                    enc_token = Some(api_key.trim().to_string());
                }
            }

            let now_str = chrono::Utc::now().to_rfc3339();
            let is_oauth = ptype == "antigravity" || ptype == "agy";
            let auth_type = if is_oauth { "oauth" } else { "apikey" };

            let record = oxiroute_auth::ProviderRecord {
                id: final_id.clone(),
                provider: ptype.to_string(),
                auth_type: auth_type.to_string(),
                name: Some(display_name.clone()),
                email: Some(display_name.clone()),
                priority: (*priority + 1) as u32,
                is_active: true,
                access_token: enc_token.clone(),
                refresh_token: if is_oauth { enc_token } else { None },
                project_id: if is_oauth { Some("aicode-consumers".to_string()) } else { None },
                expires_at: None,
                scope: None,
                provider_specific_data: None,
                created_at: Some(now_str.clone()),
                updated_at: Some(now_str),
            };

            if let Some(ref storage) = state.storage {
                if let Err(e) = storage.save_provider_connection(&record) {
                    state.add_log("ERROR", format!("Failed to save provider to disk: {}", e));
                }
            }

            let new_acc = AccountDisplay {
                id: final_id.clone(),
                provider: ptype.to_string(),
                name: display_name.clone(),
                email: if api_key.len() > 8 {
                    format!("{}****", &api_key[..4])
                } else if !api_key.is_empty() {
                    "****".to_string()
                } else {
                    display_name.clone()
                },
                priority: (*priority + 1) as u32,
                is_active: true,
                status_text: "Active ●".to_string(),
            };

            if let Ok(mut accounts) = state.accounts.write() {
                if let Some(pos) = accounts.iter().position(|a| a.id == final_id) {
                    accounts[pos] = new_acc;
                    state.add_log(
                        "INFO",
                        format!(
                            "Updated provider connection: '{}' [{}] (P{}) (Saved 💾)",
                            display_name,
                            ptype,
                            *priority + 1
                        ),
                    );
                } else {
                    accounts.push(new_acc);
                    state.add_log(
                        "INFO",
                        format!(
                            "Added provider connection: '{}' [{}] (P{}) (Saved 💾)",
                            display_name,
                            ptype,
                            *priority + 1
                        ),
                    );
                }
            }
            ModalState::None
        }
        _ => wrap_provider_modal(account_id, *provider_type_idx, name, api_key, *priority, *active_field),
    }
}

fn wrap_provider_modal(
    account_id: Option<String>,
    provider_type_idx: usize,
    name: &str,
    api_key: &str,
    priority: usize,
    active_field: usize,
) -> ModalState {
    if let Some(id) = account_id {
        ModalState::EditProvider {
            account_id: id,
            provider_type_idx,
            name: name.to_string(),
            api_key: api_key.to_string(),
            priority,
            active_field,
        }
    } else {
        ModalState::AddProvider {
            provider_type_idx,
            name: name.to_string(),
            api_key: api_key.to_string(),
            priority,
            active_field,
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

                if let Some(ref storage) = state.storage {
                    let rec = oxiroute_auth::ProxyRecord {
                        id: new_proxy.id.clone(),
                        name: Some(new_proxy.name.clone()),
                        protocol: new_proxy.protocol.clone(),
                        host: new_proxy.host.clone(),
                        port: new_proxy.port,
                        username: None,
                        password: None,
                        status: "active".to_string(),
                    };
                    if let Err(e) = storage.save_proxy(&rec) {
                        state.add_log("ERROR", format!("Failed to save proxy to disk: {}", e));
                    }
                }

                if let Ok(mut proxies) = state.proxies.write() {
                    if let Some(pos) = proxies.iter().position(|p| p.id == final_id) {
                        proxies[pos] = new_proxy;
                        state.add_log(
                            "INFO",
                            format!("Updated proxy: {}://{}:{} (Saved 💾)", proto, host, p_num),
                        );
                    } else {
                        proxies.push(new_proxy);
                        state.add_log(
                            "INFO",
                            format!("Registered proxy: {}://{}:{} (Saved 💾)", proto, host, p_num),
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


