use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use std::sync::mpsc;

use crate::daemon::{daemon_start, daemon_stop};
use crate::ops;

use super::data::{ProfileRow, UiData};
use super::i18n;
use super::form::{FieldKind, FormFocus, FormMode, ProviderFormState};
use super::route::{NavItem, Route};
use super::text_edit::TextInput;
use super::theme::{theme, Theme};

pub const TOAST_TICKS: u16 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Nav,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    pub remaining_ticks: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum LoadingKind {
    DaemonStart,
    DaemonStop,
    Saving,
}

impl LoadingKind {
    pub fn message(&self) -> &'static str {
        match self {
            LoadingKind::DaemonStart => "Starting daemon...",
            LoadingKind::DaemonStop => "Stopping daemon...",
            LoadingKind::Saving => "Saving...",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    Quit,
    DeleteProfile(String),
    FormSaveBeforeClose,
}

pub struct ConfirmOverlay {
    pub title: String,
    pub message: String,
    pub action: ConfirmAction,
}

pub enum Overlay {
    None,
    Help,
    Confirm(ConfirmOverlay),
    #[allow(dead_code)]
    Loading(LoadingKind),
}

impl Overlay {
    pub fn is_active(&self) -> bool {
        !matches!(self, Overlay::None)
    }
}

pub enum FetchModelsMessage {
    Success(Vec<String>),
    Error(String),
}

pub struct App {
    pub theme: Theme,
    pub data: UiData,
    pub focus: Focus,
    pub nav_idx: usize,
    pub route: Route,
    pub overlay: Overlay,
    pub filter: TextInput,
    pub filter_active: bool,
    pub form: Option<ProviderFormState>,
    pub form_return: Route,
    pub profiles_idx: usize,
    pub proxy_idx: usize,
    pub backups_idx: usize,
    pub stats_scroll: u16,
    pub settings_lang_idx: usize,
    pub settings_proxy_idx: usize,
    pub settings_user_agent_idx: usize,  // 0=custom, 1=claude-code, 2=codex, 3=gemini
    pub settings_editing_field: Option<usize>,
    pub settings_edit_input: TextInput,
    pub detail_scroll: u16,
    pub toast: Option<Toast>,
    pub tick: u64,
    pub should_quit: bool,
    /// True only when this TUI session spawned the proxy daemon itself (not when the
    /// daemon was already running, e.g. started from the command line). Used to decide
    /// whether to auto-stop the daemon on TUI exit.
    pub proxy_started_by_tui: bool,
    // Model selection state
    pub model_selection_list: Vec<(String, bool)>, // (model_id, selected)
    pub model_selection_idx: usize,
    pub model_selection_loading: bool,
    pub fetch_rx: Option<mpsc::Receiver<FetchModelsMessage>>,
    // Failover editor state
    pub failover_list: Vec<(String, bool)>,  // (provider_name, selected)
    pub failover_idx: usize,
}

pub fn proxy_actions() -> [&'static str; 3] {
    [i18n::proxy_action_start(), i18n::proxy_action_stop(), i18n::proxy_action_status()]
}

pub fn user_agent_presets() -> [&'static str; 4] {
    ["Custom", "Claude Code", "Codex", "Gemini"]
}

pub fn user_agent_preset_value(idx: usize) -> Option<&'static str> {
    match idx {
        0 => None,  // Custom
        1 => Some("Claude Code/1.0 (https://claude.ai)"),
        2 => Some("Codex/1.0"),
        3 => Some("Gemini/1.0 (https://gemini.google.com)"),
        _ => None,
    }
}

impl App {
    pub fn new(data: UiData) -> Self {
        // Detect User-Agent preset index
        let user_agent_idx = match data.config.settings.proxy.user_agent.as_deref() {
            Some("Claude Code/1.0 (https://claude.ai)") => 1,
            Some("Codex/1.0") => 2,
            Some("Gemini/1.0 (https://gemini.google.com)") => 3,
            _ => 0,  // Custom or None
        };

        Self {
            theme: theme(),
            data,
            focus: Focus::Nav,
            nav_idx: 0,
            route: Route::Home,
            overlay: Overlay::None,
            filter: TextInput::default(),
            filter_active: false,
            form: None,
            form_return: Route::Profiles,
            profiles_idx: 0,
            proxy_idx: 0,
            backups_idx: 0,
            stats_scroll: 0,
            settings_lang_idx: if i18n::is_zh() { 1 } else { 0 },
            settings_proxy_idx: 0,
            settings_user_agent_idx: user_agent_idx,
            settings_editing_field: None,
            settings_edit_input: TextInput::default(),
            detail_scroll: 0,
            toast: None,
            tick: 0,
            should_quit: false,
            proxy_started_by_tui: false,
            model_selection_list: vec![],
            model_selection_idx: 0,
            model_selection_loading: false,
            fetch_rx: None,
            failover_list: vec![],
            failover_idx: 0,
        }
    }

    // ─── Derived state ────────────────────────────────────

    pub fn visible_profiles(&self) -> Vec<&ProfileRow> {
        let query = self.filter.value.trim().to_lowercase();
        self.data
            .profiles
            .iter()
            .filter(|row| {
                if query.is_empty() {
                    return true;
                }
                row.name.to_lowercase().contains(&query)
                    || row.api.to_lowercase().contains(&query)
                    || row.base_url.to_lowercase().contains(&query)
                    || row
                        .models
                        .iter()
                        .any(|model| model.to_lowercase().contains(&query))
            })
            .collect()
    }

    pub fn selected_profile_name(&self) -> Option<String> {
        self.visible_profiles()
            .get(self.profiles_idx)
            .map(|row| row.name.clone())
    }

    fn clamp_indices(&mut self) {
        let visible = self.visible_profiles().len();
        self.profiles_idx = self.profiles_idx.min(visible.saturating_sub(1));
        self.proxy_idx = self.proxy_idx.min(proxy_actions().len() - 1);
        self.backups_idx = self.backups_idx.min(self.data.backups.len().saturating_sub(1));
    }

    pub fn push_toast(&mut self, kind: ToastKind, message: impl Into<String>) {
        self.toast = Some(Toast {
            message: message.into(),
            kind,
            remaining_ticks: TOAST_TICKS,
        });
    }

    fn refresh(&mut self) {
        self.data.refresh();
        self.clamp_indices();
    }

    // ─── Tick ─────────────────────────────────────────────

    pub fn on_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);

        // Refresh pi's current model (~every 2s) so the proxy page stays informative.
        if self.tick % 10 == 0 {
            self.data.refresh_pi_model();
        }

        if let Some(toast) = &mut self.toast {
            toast.remaining_ticks = toast.remaining_ticks.saturating_sub(1);
            if toast.remaining_ticks == 0 {
                self.toast = None;
            }
        }

        // Check for async fetch completion
        if let Some(rx) = &self.fetch_rx {
            if let Ok(msg) = rx.try_recv() {
                self.model_selection_loading = false;
                match msg {
                    FetchModelsMessage::Success(fetched_models) => {
                        // Check if we're in form mode (Route::FetchModels with "_temp_form")
                        let is_form_mode = matches!(&self.route, Route::FetchModels(name) if name == "_temp_form");

                        if is_form_mode {
                            // Form mode: update selection list with manual models pre-selected
                            let manual_models: std::collections::HashSet<_> = self.model_selection_list
                                .iter()
                                .filter_map(|(id, selected)| if *selected { Some(id.clone()) } else { None })
                                .collect();

                            self.model_selection_list = fetched_models
                                .into_iter()
                                .map(|id| {
                                    let selected = manual_models.contains(&id);
                                    (id, selected)
                                })
                                .collect();

                            self.push_toast(ToastKind::Success, i18n::toast_models_fetched(self.model_selection_list.len()));
                        } else {
                            // Model selection mode: merge preserving selection state
                            let existing: std::collections::HashSet<_> = self.model_selection_list
                                .iter()
                                .filter_map(|(id, selected)| if *selected { Some(id.clone()) } else { None })
                                .collect();

                            self.model_selection_list = fetched_models
                                .into_iter()
                                .map(|id| {
                                    let selected = existing.contains(&id);
                                    (id, selected)
                                })
                                .collect();

                            self.push_toast(ToastKind::Success, i18n::toast_models_fetched(self.model_selection_list.len()));
                        }
                    }
                    FetchModelsMessage::Error(err) => {
                        self.push_toast(ToastKind::Error, format!("Fetch failed: {}", err));
                    }
                }
                self.fetch_rx = None;
            }
        }
    }

    // ─── Mouse ────────────────────────────────────────────

    pub fn on_mouse(&mut self, mouse: MouseEvent) {
        let key = match mouse.kind {
            MouseEventKind::ScrollUp => KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            MouseEventKind::ScrollDown => KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            _ => return,
        };
        self.on_key(key);
    }

    // ─── Keys ─────────────────────────────────────────────

    /// Normalize vim-style hjkl into arrow keys for non-editing contexts.
    fn normalize_key(key: KeyEvent) -> KeyEvent {
        if key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            return key;
        }
        let code = match key.code {
            KeyCode::Char('h') => KeyCode::Left,
            KeyCode::Char('j') => KeyCode::Down,
            KeyCode::Char('k') => KeyCode::Up,
            KeyCode::Char('l') => KeyCode::Right,
            other => other,
        };
        KeyEvent::new(code, key.modifiers)
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'C'))
        {
            self.should_quit = true;
            return;
        }

        if self.overlay.is_active() {
            self.on_overlay_key(key);
            return;
        }

        if self.route == Route::Form {
            self.on_form_key(key);
            return;
        }

        if self.filter_active {
            self.on_filter_key(key);
            return;
        }

        // Global keys
        match key.code {
            KeyCode::Char('?') => {
                self.overlay = Overlay::Help;
                return;
            }
            KeyCode::Char('/') if self.route == Route::Profiles => {
                self.filter_active = true;
                self.focus = Focus::Content;
                return;
            }
            KeyCode::Char('q') => {
                if let Route::ProfileDetail(_) = self.route {
                    self.route = Route::Profiles;
                } else {
                    self.request_quit();
                }
                return;
            }
            _ => {}
        }

        let key = Self::normalize_key(key);

        match key.code {
            KeyCode::Left if self.focus == Focus::Content => {
                self.focus = Focus::Nav;
                return;
            }
            KeyCode::Right if self.focus == Focus::Nav => {
                self.focus = Focus::Content;
                return;
            }
            _ => {}
        }

        match self.focus {
            Focus::Nav => self.on_nav_key(key),
            Focus::Content => self.on_content_key(key),
        }
    }

    fn request_quit(&mut self) {
        self.overlay = Overlay::Confirm(ConfirmOverlay {
            title: i18n::confirm_exit_title().into(),
            message: i18n::confirm_exit_msg().into(),
            action: ConfirmAction::Quit,
        });
    }

    // ─── Overlay keys ─────────────────────────────────────

    fn on_overlay_key(&mut self, key: KeyEvent) {
        match &self.overlay {
            Overlay::Help => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Enter
                ) {
                    self.overlay = Overlay::None;
                }
            }
            Overlay::Confirm(confirm) => match key.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                    let action = confirm.action.clone();
                    self.overlay = Overlay::None;
                    self.execute_confirm(action);
                }
                KeyCode::Char('n' | 'N') => {
                    let action = confirm.action.clone();
                    self.overlay = Overlay::None;
                    if action == ConfirmAction::FormSaveBeforeClose {
                        self.close_form();
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.overlay = Overlay::None;
                }
                _ => {}
            },
            Overlay::Loading(_) => {}
            Overlay::None => {}
        }
    }

    fn execute_confirm(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::Quit => {
                self.should_quit = true;
            }
            ConfirmAction::DeleteProfile(name) => match ops::remove_profile(&name) {
                Ok(_) => {
                    self.push_toast(ToastKind::Success, i18n::toast_deleted(&name));
                    self.refresh();
                    if self.route == Route::ProfileDetail(name) {
                        self.route = Route::Profiles;
                    }
                }
                Err(e) => self.push_toast(ToastKind::Error, e.to_string()),
            },
            ConfirmAction::FormSaveBeforeClose => {
                if self.save_form() {
                    self.close_form();
                }
            }
        }
    }

    // ─── Filter keys ──────────────────────────────────────

    fn on_filter_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                self.filter_active = false;
                self.clamp_indices();
            }
            KeyCode::Esc => {
                self.filter.clear();
                self.filter_active = false;
                self.clamp_indices();
            }
            KeyCode::Up | KeyCode::Down => {
                self.filter_active = false;
                self.clamp_indices();
                self.on_profiles_key(key);
            }
            _ => {
                if self.filter.apply_key(key).is_some() {
                    self.profiles_idx = 0;
                }
            }
        }
    }

    // ─── Nav keys ─────────────────────────────────────────

    fn on_nav_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                self.nav_idx = self.nav_idx.saturating_sub(1);
            }
            KeyCode::Down => {
                self.nav_idx = (self.nav_idx + 1).min(NavItem::ALL.len() - 1);
            }
            KeyCode::Enter => {
                let item = NavItem::ALL[self.nav_idx];
                match item.to_route() {
                    Some(route) => {
                        self.route = route;
                        self.focus = Focus::Content;
                        self.detail_scroll = 0;
                        self.clamp_indices();
                    }
                    None => self.request_quit(),
                }
            }
            KeyCode::Esc => self.request_quit(),
            _ => {}
        }
    }

    // ─── Content keys ─────────────────────────────────────

    fn on_content_key(&mut self, key: KeyEvent) {
        match self.route.clone() {
            Route::Home => self.on_home_key(key),
            Route::Profiles => self.on_profiles_key(key),
            Route::ProfileDetail(name) => self.on_profile_detail_key(key, &name),
            Route::FetchModels(name) => self.on_fetch_models_key(key, &name),
            Route::ExposeModels(name) => self.on_expose_models_key(key, &name),
            Route::Proxy => self.on_proxy_key(key),
            Route::Stats => self.on_stats_key(key),
            Route::Backups => self.on_backups_key(key),
            Route::Settings => self.on_settings_key(key),
            Route::FailoverEditor => self.on_failover_editor_key(key),
            Route::Form => {}
        }
    }

    fn back_to_nav_on_esc(&mut self, key: &KeyEvent) -> bool {
        if key.code == KeyCode::Esc {
            self.focus = Focus::Nav;
            true
        } else {
            false
        }
    }

    fn on_home_key(&mut self, key: KeyEvent) {
        if self.back_to_nav_on_esc(&key) {
            return;
        }
        if key.code == KeyCode::Char('r') {
            self.refresh();
            self.push_toast(ToastKind::Info, i18n::toast_refreshed());
        }
    }

    fn on_profiles_key(&mut self, key: KeyEvent) {
        let visible_len = self.visible_profiles().len();
        match key.code {
            KeyCode::Esc => {
                if !self.filter.value.is_empty() {
                    self.filter.clear();
                    self.clamp_indices();
                } else {
                    self.focus = Focus::Nav;
                }
            }
            KeyCode::Up => {
                self.profiles_idx = self.profiles_idx.saturating_sub(1);
            }
            KeyCode::Down => {
                if visible_len > 0 {
                    self.profiles_idx = (self.profiles_idx + 1).min(visible_len - 1);
                }
            }
            KeyCode::Enter => {
                if let Some(name) = self.selected_profile_name() {
                    self.detail_scroll = 0;
                    self.route = Route::ProfileDetail(name);
                }
            }
            KeyCode::Char(' ') | KeyCode::Char('s') => {
                if let Some(name) = self.selected_profile_name() {
                    self.switch_profile(&name);
                }
            }
            KeyCode::Char('a') => self.open_add_form(),
            KeyCode::Char('e') => {
                if let Some(name) = self.selected_profile_name() {
                    self.open_edit_form(&name);
                }
            }
            KeyCode::Char('c') => {
                if let Some(name) = self.selected_profile_name() {
                    self.copy_profile(&name);
                }
            }
            KeyCode::Char('d') => {
                if let Some(name) = self.selected_profile_name() {
                    self.confirm_delete(&name);
                }
            }
            KeyCode::Char('r') => {
                self.refresh();
                self.push_toast(ToastKind::Info, i18n::toast_refreshed());
            }
            _ => {}
        }
    }

    fn on_profile_detail_key(&mut self, key: KeyEvent, name: &str) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.route = Route::Profiles;
            }
            KeyCode::Up => {
                self.detail_scroll = self.detail_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                self.detail_scroll = self.detail_scroll.saturating_add(1);
            }
            KeyCode::Char('e') => self.open_edit_form(name),
            KeyCode::Char(' ') | KeyCode::Char('s') => self.switch_profile(name),
            KeyCode::Char('d') => self.confirm_delete(name),
            KeyCode::Char('x') => {
                // Expose models selection
                self.route = Route::ExposeModels(name.to_string());
                self.load_expose_models_selection(name);
            }
            _ => {}
        }
    }

    fn on_proxy_key(&mut self, key: KeyEvent) {
        if self.back_to_nav_on_esc(&key) {
            return;
        }
        match key.code {
            KeyCode::Up => {
                self.proxy_idx = self.proxy_idx.saturating_sub(1);
            }
            KeyCode::Down => {
                self.proxy_idx = (self.proxy_idx + 1).min(proxy_actions().len() - 1);
            }
            KeyCode::Enter => match self.proxy_idx {
                0 => match daemon_start(None, None) {
                    Ok(result) => {
                        // started_at is Some only when we actually spawned the daemon
                        // (not when it was already running). Only then do we own it.
                        if result.started_at.is_some() {
                            self.proxy_started_by_tui = true;
                        }
                        self.push_toast(ToastKind::Success, result.message);
                        self.refresh();
                    }
                    Err(e) => self.push_toast(ToastKind::Error, e),
                },
                1 => match daemon_stop() {
                    Ok(result) => {
                        self.proxy_started_by_tui = false;
                        self.push_toast(ToastKind::Info, result.message);
                        self.refresh();
                    }
                    Err(e) => self.push_toast(ToastKind::Error, e),
                },
                _ => {
                    self.refresh();
                    self.push_toast(ToastKind::Info, i18n::toast_status_refreshed());
                }
            },
            _ => {}
        }
    }

    fn on_stats_key(&mut self, key: KeyEvent) {
        if self.back_to_nav_on_esc(&key) {
            return;
        }
        match key.code {
            KeyCode::Char('r') => {
                self.refresh();
                self.push_toast(ToastKind::Info, i18n::toast_refreshed());
            }
            KeyCode::Up => {
                self.stats_scroll = self.stats_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                self.stats_scroll = self.stats_scroll.saturating_add(1);
            }
            _ => {}
        }
    }

    fn on_backups_key(&mut self, key: KeyEvent) {
        if self.back_to_nav_on_esc(&key) {
            return;
        }
        let len = self.data.backups.len();
        match key.code {
            KeyCode::Up => {
                self.backups_idx = self.backups_idx.saturating_sub(1);
            }
            KeyCode::Down => {
                if len > 0 {
                    self.backups_idx = (self.backups_idx + 1).min(len - 1);
                }
            }
            _ => {}
        }
    }

    fn on_settings_key(&mut self, key: KeyEvent) {
        if let Some(field_idx) = self.settings_editing_field {
            // Editing a text field
            let key = Self::normalize_key(key);
            match key.code {
                KeyCode::Esc => {
                    self.settings_editing_field = None;
                }
                KeyCode::Enter => {
                    self.commit_settings_field(field_idx);
                    self.settings_editing_field = None;
                }
                _ => {
                    self.settings_edit_input.apply_key(key);
                }
            }
            return;
        }

        if self.back_to_nav_on_esc(&key) {
            return;
        }

        let row_count = 5; // Language + host + port + user-agent + failover
        match key.code {
            KeyCode::Up => {
                self.settings_proxy_idx = self.settings_proxy_idx.saturating_sub(1);
            }
            KeyCode::Down => {
                self.settings_proxy_idx = (self.settings_proxy_idx + 1).min(row_count - 1);
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') => {
                if self.settings_proxy_idx == 0 {
                    // Toggle language
                    let new_idx = 1 - self.settings_lang_idx;
                    let new_lang = if new_idx == 1 { i18n::Lang::Zh } else { i18n::Lang::En };
                    match i18n::set_lang(new_lang) {
                        Ok(()) => {
                            self.settings_lang_idx = new_idx;
                            self.push_toast(ToastKind::Success, i18n::toast_lang_set(new_lang.code()));
                        }
                        Err(e) => self.push_toast(ToastKind::Error, e),
                    }
                } else if self.settings_proxy_idx == 3 {
                    // Cycle User-Agent presets
                    let presets = user_agent_presets();
                    let direction = if matches!(key.code, KeyCode::Left) { -1i32 } else { 1i32 };
                    let new_idx = ((self.settings_user_agent_idx as i32 + direction)
                        .rem_euclid(presets.len() as i32)) as usize;
                    self.settings_user_agent_idx = new_idx;

                    // Apply preset value to config
                    if let Ok(mut config) = crate::config::load_config() {
                        config.settings.proxy.user_agent = user_agent_preset_value(new_idx).map(|s| s.to_string());
                        if let Ok(()) = crate::config::save_config(&config) {
                            self.refresh();
                            let preset_name = presets[new_idx];
                            self.push_toast(ToastKind::Success, format!("User-Agent: {}", preset_name));
                        }
                    }
                }
            }
            KeyCode::Enter => {
                if self.settings_proxy_idx == 4 {
                    // Open failover editor
                    self.open_failover_editor();
                } else if self.settings_proxy_idx == 3 && self.settings_user_agent_idx == 0 {
                    // Custom User-Agent - allow editing
                    let text = self.get_settings_field_value(self.settings_proxy_idx);
                    self.settings_edit_input = TextInput::new(text).with_policy(super::text_edit::TextInputPolicy::Any);
                    self.settings_editing_field = Some(self.settings_proxy_idx);
                } else if self.settings_proxy_idx == 1 || self.settings_proxy_idx == 2 {
                    // Host / port text fields
                    let text = self.get_settings_field_value(self.settings_proxy_idx);
                    let policy = if self.settings_proxy_idx == 2 {
                        super::text_edit::TextInputPolicy::Digits
                    } else {
                        super::text_edit::TextInputPolicy::Any
                    };
                    self.settings_edit_input = TextInput::new(text).with_policy(policy);
                    self.settings_editing_field = Some(self.settings_proxy_idx);
                }
            }
            _ => {}
        }
    }

    fn get_settings_field_value(&self, idx: usize) -> String {
        let proxy = &self.data.config.settings.proxy;
        match idx {
            0 => String::new(),
            1 => proxy.host.clone(),
            2 => proxy.port.to_string(),
            3 => proxy.user_agent.as_deref().unwrap_or("").to_string(),
            4 => if proxy.failover.is_empty() {
                String::new()
            } else {
                proxy.failover.join(", ")
            },
            _ => String::new(),
        }
    }

    fn commit_settings_field(&mut self, idx: usize) {
        let value = self.settings_edit_input.value.trim().to_string();
        let mut config = match crate::config::load_config() {
            Ok(c) => c,
            Err(e) => {
                self.push_toast(ToastKind::Error, e.to_string());
                return;
            }
        };

        match idx {
            1 => config.settings.proxy.host = value,
            2 => {
                if let Ok(port) = value.parse::<u16>() {
                    config.settings.proxy.port = port;
                } else {
                    self.push_toast(ToastKind::Error, "Invalid port number");
                    return;
                }
            }
            3 => {
                // User-Agent custom value
                config.settings.proxy.user_agent = if value.is_empty() { None } else { Some(value) };
                // Set to Custom mode when manually edited
                self.settings_user_agent_idx = 0;
            }
            _ => return,
        }

        match crate::config::save_config(&config) {
            Ok(()) => {
                self.refresh();
                self.push_toast(ToastKind::Success, i18n::settings_saved());
            }
            Err(e) => self.push_toast(ToastKind::Error, e.to_string()),
        }
    }

    fn open_failover_editor(&mut self) {
        // Load all non-proxy profiles as candidates
        let current_failover = &self.data.config.settings.proxy.failover;

        // Build list: start with selected (in order), then unselected
        let mut list = Vec::new();

        // First add selected ones in their current order
        for name in current_failover {
            if self.data.config.profiles.contains_key(name) {
                list.push((name.clone(), true));
            }
        }

        // Then add unselected ones (non-proxy profiles not in failover)
        for (name, profile) in &self.data.config.profiles {
            if current_failover.contains(name) {
                continue;  // Already added
            }
            let is_proxy = profile.get("proxy")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_proxy {
                list.push((name.clone(), false));
            }
        }

        self.failover_list = list;
        self.failover_idx = 0;
        self.route = Route::FailoverEditor;
    }

    fn on_failover_editor_key(&mut self, key: KeyEvent) {
        if self.back_to_nav_on_esc(&key) {
            self.route = Route::Settings;
            return;
        }

        let len = self.failover_list.len();
        if len == 0 {
            return;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.failover_idx = self.failover_idx.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.failover_idx = (self.failover_idx + 1).min(len - 1);
            }
            KeyCode::Char(' ') => {
                // Toggle selection
                self.failover_list[self.failover_idx].1 = !self.failover_list[self.failover_idx].1;
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Move item up
                if self.failover_idx > 0 {
                    self.failover_list.swap(self.failover_idx, self.failover_idx - 1);
                    self.failover_idx -= 1;
                }
            }
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Move item down
                if self.failover_idx < len - 1 {
                    self.failover_list.swap(self.failover_idx, self.failover_idx + 1);
                    self.failover_idx += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('s') => {
                // Save failover configuration
                self.save_failover_config();
            }
            _ => {}
        }
    }

    fn save_failover_config(&mut self) {
        let mut config = match crate::config::load_config() {
            Ok(c) => c,
            Err(e) => {
                self.push_toast(ToastKind::Error, e.to_string());
                return;
            }
        };

        // Collect selected items in order
        let failover: Vec<String> = self.failover_list.iter()
            .filter(|(_, selected)| *selected)
            .map(|(name, _)| name.clone())
            .collect();

        config.settings.proxy.failover = failover;

        match crate::config::save_config(&config) {
            Ok(()) => {
                self.refresh();
                self.route = Route::Settings;
                self.push_toast(ToastKind::Success, i18n::settings_saved());
            }
            Err(e) => self.push_toast(ToastKind::Error, e.to_string()),
        }
    }

    // ─── Profile actions ──────────────────────────────────

    fn switch_profile(&mut self, name: &str) {
        match ops::use_profile(name, None) {
            Ok(outcome) => {
                self.push_toast(
                    ToastKind::Success,
                    i18n::toast_switched(&outcome.name, &outcome.provider_id),
                );
                self.refresh();
            }
            Err(e) => self.push_toast(ToastKind::Error, e.to_string()),
        }
    }

    fn copy_profile(&mut self, src: &str) {
        let mut dst = format!("{src}-copy");
        let mut counter = 2;
        while self.data.config.profiles.contains_key(&dst) {
            dst = format!("{src}-copy{counter}");
            counter += 1;
        }
        match ops::duplicate_profile(src, &dst) {
            Ok(_) => {
                self.push_toast(ToastKind::Success, i18n::toast_copied(src, &dst));
                self.refresh();
            }
            Err(e) => self.push_toast(ToastKind::Error, e.to_string()),
        }
    }

    fn confirm_delete(&mut self, name: &str) {
        self.overlay = Overlay::Confirm(ConfirmOverlay {
            title: i18n::confirm_delete_title().into(),
            message: i18n::confirm_delete_msg(name),
            action: ConfirmAction::DeleteProfile(name.to_string()),
        });
    }

    // ─── Form ─────────────────────────────────────────────

    fn open_add_form(&mut self) {
        self.form = Some(ProviderFormState::new_add());
        self.form_return = Route::Profiles;
        self.route = Route::Form;
    }

    fn open_edit_form(&mut self, name: &str) {
        let Some(value) = self.data.config.profiles.get(name) else {
            return;
        };
        match serde_json::from_value(value.clone()) {
            Ok(profile) => {
                self.form = Some(ProviderFormState::from_existing(name, &profile));
                self.form_return = self.route.clone();
                self.route = Route::Form;
            }
            Err(e) => self.push_toast(ToastKind::Error, i18n::toast_invalid_json(&e.to_string())),
        }
    }

    fn close_form(&mut self) {
        self.form = None;
        self.route = self.form_return.clone();
        self.clamp_indices();
    }

    fn save_form(&mut self) -> bool {
        let Some(form) = &self.form else {
            return false;
        };
        let profile = match form.validate() {
            Ok(profile) => profile,
            Err(msg) => {
                self.push_toast(ToastKind::Warning, msg);
                return false;
            }
        };
        let name = form.profile_name();
        let rename_from = match &form.mode {
            FormMode::Edit(original) => Some(original.clone()),
            FormMode::Add => None,
        };

        let is_new_name = rename_from.as_deref() != Some(name.as_str());
        if is_new_name && self.data.config.profiles.contains_key(&name) {
            self.push_toast(
                ToastKind::Error,
                    if i18n::is_zh() { format!("供应商 '{name}' 已存在") } else { format!("Profile '{name}' already exists") },
            );
            return false;
        }

        let mut profile = profile;
        profile.updated_at = Some(chrono::Utc::now().to_rfc3339());

        match ops::upsert_profile(&name, &profile, rename_from.as_deref()) {
            Ok(_) => {
                self.push_toast(ToastKind::Success, i18n::toast_saved(&name));
                self.refresh();
                true
            }
            Err(e) => {
                self.push_toast(ToastKind::Error, e.to_string());
                false
            }
        }
    }

    fn request_close_form(&mut self) {
        let dirty = self.form.as_ref().is_some_and(|form| form.is_dirty());
        if dirty {
            self.overlay = Overlay::Confirm(ConfirmOverlay {
                title: i18n::confirm_discard_title().into(),
                message: i18n::confirm_discard_msg().into(),
                action: ConfirmAction::FormSaveBeforeClose,
            });
        } else {
            self.close_form();
        }
    }

    fn on_form_key(&mut self, key: KeyEvent) {
        let Some(form) = &mut self.form else {
            self.route = self.form_return.clone();
            return;
        };

        // JSON edit mode
        if form.json_editing {
            match key.code {
                KeyCode::Esc => {
                    form.json_editing = false;
                }
                KeyCode::Enter => {
                    match form.apply_json_edit() {
                        Ok(()) => {}
                        Err(e) => self.push_toast(ToastKind::Error, e),
                    }
                }
                KeyCode::Tab => {
                    if let Err(e) = form.apply_json_edit() {
                        self.push_toast(ToastKind::Error, e);
                    }
                }
                KeyCode::Up | KeyCode::Down => {
                    // Multi-line navigation: move cursor up/down by line
                    let lines: Vec<&str> = form.json_edit.value.lines().collect();
                    if lines.is_empty() {
                        return;
                    }

                    // Find current line and column
                    let mut cursor_pos = 0;
                    let mut current_line = 0;
                    let mut col_in_line = 0;
                    for (line_idx, line) in lines.iter().enumerate() {
                        let line_len = line.chars().count();
                        if cursor_pos + line_len >= form.json_edit.cursor {
                            current_line = line_idx;
                            col_in_line = form.json_edit.cursor - cursor_pos;
                            break;
                        }
                        cursor_pos += line_len + 1; // +1 for newline
                    }

                    // Move to target line
                    let target_line = match key.code {
                        KeyCode::Up => current_line.saturating_sub(1),
                        KeyCode::Down => (current_line + 1).min(lines.len() - 1),
                        _ => current_line,
                    };

                    if target_line != current_line {
                        // Calculate new cursor position
                        let mut new_cursor = 0;
                        for (idx, line) in lines.iter().enumerate() {
                            if idx == target_line {
                                let target_len = line.chars().count();
                                new_cursor += col_in_line.min(target_len);
                                break;
                            }
                            new_cursor += line.chars().count() + 1;
                        }
                        form.json_edit.cursor = new_cursor;

                        // Auto-scroll to keep cursor visible
                        let total_lines = lines.len();
                        if total_lines > 0 {
                            // Assuming area height of around 20 lines (will adjust in rendering)
                            let visible_lines = 15; // Conservative estimate
                            if target_line < form.json_scroll as usize {
                                form.json_scroll = target_line as u16;
                            } else if target_line >= (form.json_scroll as usize + visible_lines) {
                                form.json_scroll = target_line.saturating_sub(visible_lines - 1) as u16;
                            }
                        }
                    }
                }
                _ => {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(key.code, KeyCode::Char('s' | 'S'))
                    {
                        if let Err(e) = form.apply_json_edit() {
                            self.push_toast(ToastKind::Error, e);
                            return;
                        }
                        if self.save_form() {
                            self.close_form();
                        }
                        return;
                    }
                    form.json_edit.apply_key(key);
                }
            }
            return;
        }

        // Field edit mode
        if form.editing {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    form.editing = false;
                }
                KeyCode::Tab => {
                    form.editing = false;
                    form.next_focus();
                }
                _ => {
                    if let Some(input) = form.current_input_mut() {
                        input.apply_key(key);
                    }
                }
            }
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('s' | 'S'))
        {
            if self.save_form() {
                self.close_form();
            }
            return;
        }

        match key.code {
            KeyCode::Tab => {
                if let Some(form) = &mut self.form {
                    form.next_focus();
                }
            }
            KeyCode::Esc => self.request_close_form(),
            _ => {
                let focus = match &self.form {
                    Some(form) => form.focus,
                    None => return,
                };
                match focus {
                    FormFocus::Templates => self.on_form_templates_key(key),
                    FormFocus::Fields => self.on_form_fields_key(key),
                    FormFocus::JsonPreview => self.on_form_json_preview_key(key),
                }
            }
        }
    }

    fn on_form_templates_key(&mut self, key: KeyEvent) {
        let template_count = self.data.presets.len() + 1;
        let Some(form) = &mut self.form else {
            return;
        };
        let key = Self::normalize_key(key);
        match key.code {
            KeyCode::Left => {
                form.template_idx = form.template_idx.saturating_sub(1);
            }
            KeyCode::Right => {
                form.template_idx = (form.template_idx + 1).min(template_count - 1);
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if form.template_idx == 0 {
                    form.clear_template();
                } else if let Some(preset) = self.data.presets.get(form.template_idx - 1) {
                    form.apply_preset(preset);
                }
                form.focus = FormFocus::Fields;
            }
            KeyCode::Down => {
                form.focus = FormFocus::Fields;
            }
            _ => {}
        }
    }

    fn on_form_fields_key(&mut self, key: KeyEvent) {
        let Some(form) = &mut self.form else {
            return;
        };
        let key = Self::normalize_key(key);
        match key.code {
            KeyCode::Up => {
                if form.field_idx == 0 && matches!(form.mode, FormMode::Add) {
                    form.focus = FormFocus::Templates;
                } else {
                    form.field_idx = form.field_idx.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                form.field_idx = (form.field_idx + 1).min(FieldKind::ALL.len() - 1);
            }
            KeyCode::Enter => {
                if form.current_field().is_text() {
                    form.editing = true;
                } else {
                    form.cycle_api(true);
                }
            }
            KeyCode::Char(' ') => {
                if form.current_field() == FieldKind::Api {
                    form.cycle_api(true);
                }
            }
            KeyCode::Char('d') => {
                // Quick clear current field
                if let Some(input) = form.current_input_mut() {
                    input.clear();
                }
            }
            KeyCode::Char('f') => {
                // Enter fetch models selection page (always use temp form marker)
                self.route = Route::FetchModels("_temp_form".to_string());
                self.start_fetch_models_for_form();
            }
            KeyCode::Left => {
                if form.current_field() == FieldKind::Api {
                    form.cycle_api(false);
                }
            }
            KeyCode::Right => {
                if form.current_field() == FieldKind::Api {
                    form.cycle_api(true);
                }
            }
            _ => {}
        }
    }

    fn on_form_json_preview_key(&mut self, key: KeyEvent) {
        let Some(form) = &mut self.form else {
            return;
        };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                form.json_scroll = form.json_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                form.json_scroll = form.json_scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                form.json_scroll = form.json_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                form.json_scroll = form.json_scroll.saturating_add(10);
            }
            KeyCode::Home => {
                form.json_scroll = 0;
            }
            KeyCode::Enter => {
                form.json_edit = TextInput::new(form.json_preview());
                form.json_editing = true;
            }
            _ => {}
        }
    }

    // ─── Model Selection ──────────────────────────────────────

    fn start_fetch_models_for_form(&mut self) {
        let Some(form) = &self.form else { return };

        // Get manually entered models
        let manual_models: Vec<String> = form.models.value
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        // Build a temporary profile to fetch from
        let temp_profile = crate::config::ProviderProfile {
            api: super::form::API_CHOICES[form.api_idx].to_string(),
            base_url: form.base_url.value.trim().to_string(),
            api_key: form.api_key.value.trim().to_string(),
            models: vec![],
            exposed_models: vec![],
            ..Default::default()
        };

        if temp_profile.base_url.is_empty() || temp_profile.api_key.is_empty() {
            self.push_toast(ToastKind::Error, if i18n::is_zh() {
                "请先填写 API 地址和密钥"
            } else {
                "Please fill in Base URL and API Key first"
            });
            return;
        }

        // Show manual models immediately with all selected
        self.model_selection_list = manual_models.iter()
            .map(|id| (id.clone(), true))
            .collect();
        self.model_selection_idx = 0;
        self.model_selection_loading = true;

        // Spawn async fetch task
        let (tx, rx) = mpsc::channel();
        let manual_models_clone = manual_models.clone();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(FetchModelsMessage::Error(format!("Runtime error: {}", e)));
                    return;
                }
            };

            // Call fetch directly with the temp profile values
            let client = match reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(FetchModelsMessage::Error(e.to_string()));
                    return;
                }
            };

            let api_key = crate::config::resolve_env(&temp_profile.api_key);
            let candidate_urls = ops::build_model_fetch_urls(&temp_profile.base_url, &temp_profile.api);

            let result = rt.block_on(async {
                for url in candidate_urls {
                    let mut req = client.get(&url);
                    req = match temp_profile.api.as_str() {
                        "openai-completions" => req.header("Authorization", format!("Bearer {}", api_key)),
                        "anthropic-messages" => req.header("x-api-key", &api_key)
                            .header("anthropic-version", "2023-06-01"),
                        _ => req.header("Authorization", format!("Bearer {}", api_key)),
                    };

                    if let Ok(resp) = req.send().await {
                        if resp.status().is_success() {
                            if let Ok(payload) = resp.json::<serde_json::Value>().await {
                                let mut models = ops::parse_model_ids(&payload);
                                // Merge with manual models (keep manual ones at front)
                                for manual in &manual_models_clone {
                                    if !models.contains(manual) {
                                        models.insert(0, manual.clone());
                                    }
                                }
                                if !models.is_empty() {
                                    return Ok(models);
                                }
                            }
                        }
                    }
                }
                Err(crate::error::AppError::Message("No models found".into()))
            });

            match result {
                Ok(models) => {
                    let _ = tx.send(FetchModelsMessage::Success(models));
                }
                Err(e) => {
                    let _ = tx.send(FetchModelsMessage::Error(e.to_string()));
                }
            }
        });

        self.fetch_rx = Some(rx);
    }

    fn load_expose_models_selection(&mut self, name: &str) {
        self.model_selection_list.clear();
        self.model_selection_idx = 0;

        let profile_value = match self.data.config.profiles.get(name) {
            Some(p) => p,
            None => return,
        };

        let profile: crate::config::ProviderProfile = match serde_json::from_value(profile_value.clone()) {
            Ok(p) => p,
            Err(_) => return,
        };

        // Load all models with exposed state
        for model in &profile.models {
            let is_exposed = profile.exposed_models.contains(&model.id);
            self.model_selection_list.push((model.id.clone(), is_exposed));
        }
    }

    fn on_fetch_models_key(&mut self, key: KeyEvent, name: &str) {
        let is_form_mode = name == "_temp_form";

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if is_form_mode {
                    self.route = Route::Form;
                } else {
                    self.route = Route::ProfileDetail(name.to_string());
                }
            }
            KeyCode::Up => {
                self.model_selection_idx = self.model_selection_idx.saturating_sub(1);
            }
            KeyCode::Down => {
                if !self.model_selection_list.is_empty() {
                    self.model_selection_idx = (self.model_selection_idx + 1)
                        .min(self.model_selection_list.len() - 1);
                }
            }
            KeyCode::Char(' ') => {
                if let Some((_, selected)) = self.model_selection_list.get_mut(self.model_selection_idx) {
                    *selected = !*selected;
                }
            }
            KeyCode::Char('s') => {
                self.save_fetch_models_selection(name);
            }
            _ => {}
        }
    }

    fn on_expose_models_key(&mut self, key: KeyEvent, name: &str) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.route = Route::ProfileDetail(name.to_string());
            }
            KeyCode::Up => {
                self.model_selection_idx = self.model_selection_idx.saturating_sub(1);
            }
            KeyCode::Down => {
                if !self.model_selection_list.is_empty() {
                    self.model_selection_idx = (self.model_selection_idx + 1)
                        .min(self.model_selection_list.len() - 1);
                }
            }
            KeyCode::Char(' ') => {
                if let Some((_, selected)) = self.model_selection_list.get_mut(self.model_selection_idx) {
                    *selected = !*selected;
                }
            }
            KeyCode::Char('s') => {
                self.save_expose_models_selection(name);
            }
            _ => {}
        }
    }

    fn save_fetch_models_selection(&mut self, name: &str) {
        let is_form_mode = name == "_temp_form";

        if is_form_mode {
            // Form mode: update form's models field
            let selected_ids: Vec<String> = self.model_selection_list
                .iter()
                .filter(|(_, selected)| *selected)
                .map(|(id, _)| id.clone())
                .collect();

            if let Some(form) = &mut self.form {
                form.models.set(selected_ids.join(", "));
            }

            self.push_toast(ToastKind::Success, i18n::toast_models_fetched(selected_ids.len()));
            self.route = Route::Form;
        } else {
            // Profile mode: save to config
            let selected_models: Vec<crate::config::ModelEntry> = self.model_selection_list
                .iter()
                .filter(|(_, selected)| *selected)
                .map(|(id, _)| crate::config::ModelEntry {
                    id: id.clone(),
                    ..Default::default()
                })
                .collect();

            match ops::update_provider_models(name, selected_models) {
                Ok(_) => {
                    self.push_toast(ToastKind::Success, i18n::toast_models_updated(name));
                    self.refresh();
                    self.route = Route::ProfileDetail(name.to_string());
                }
                Err(e) => {
                    self.push_toast(ToastKind::Error, e.to_string());
                }
            }
        }
    }

    fn save_expose_models_selection(&mut self, name: &str) {
        let exposed_model_ids: Vec<String> = self.model_selection_list
            .iter()
            .filter(|(_, selected)| *selected)
            .map(|(id, _)| id.clone())
            .collect();

        match ops::update_exposed_models(name, exposed_model_ids) {
            Ok(_) => {
                self.push_toast(ToastKind::Success, i18n::toast_exposed_models_updated(name));
                self.refresh();
                self.route = Route::ProfileDetail(name.to_string());
            }
            Err(e) => {
                self.push_toast(ToastKind::Error, e.to_string());
            }
        }
    }
}
