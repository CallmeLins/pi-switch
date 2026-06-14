use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

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
    Loading(LoadingKind),
}

impl Overlay {
    pub fn is_active(&self) -> bool {
        !matches!(self, Overlay::None)
    }
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
    pub presets_idx: usize,
    pub proxy_idx: usize,
    pub backups_idx: usize,
    pub settings_lang_idx: usize,
    pub settings_proxy_idx: usize,
    pub settings_editing_field: Option<usize>,
    pub settings_edit_input: TextInput,
    pub detail_scroll: u16,
    pub toast: Option<Toast>,
    pub tick: u64,
    pub should_quit: bool,
}

pub fn proxy_actions() -> [&'static str; 3] {
    [i18n::proxy_action_start(), i18n::proxy_action_stop(), i18n::proxy_action_status()]
}

impl App {
    pub fn new(data: UiData) -> Self {
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
            presets_idx: 0,
            proxy_idx: 0,
            backups_idx: 0,
            settings_lang_idx: if i18n::is_zh() { 1 } else { 0 },
            settings_proxy_idx: 0,
            settings_editing_field: None,
            settings_edit_input: TextInput::default(),
            detail_scroll: 0,
            toast: None,
            tick: 0,
            should_quit: false,
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
        self.presets_idx = self.presets_idx.min(self.data.presets.len().saturating_sub(1));
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
        if let Some(toast) = &mut self.toast {
            toast.remaining_ticks = toast.remaining_ticks.saturating_sub(1);
            if toast.remaining_ticks == 0 {
                self.toast = None;
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
            Route::Presets => self.on_presets_key(key),
            Route::Proxy => self.on_proxy_key(key),
            Route::Stats => self.on_stats_key(key),
            Route::Backups => self.on_backups_key(key),
            Route::Settings => self.on_settings_key(key),
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
            _ => {}
        }
    }

    fn on_presets_key(&mut self, key: KeyEvent) {
        if self.back_to_nav_on_esc(&key) {
            return;
        }
        let len = self.data.presets.len();
        match key.code {
            KeyCode::Up => {
                self.presets_idx = self.presets_idx.saturating_sub(1);
            }
            KeyCode::Down => {
                if len > 0 {
                    self.presets_idx = (self.presets_idx + 1).min(len - 1);
                }
            }
            KeyCode::Enter => {
                if let Some(preset) = self.data.presets.get(self.presets_idx) {
                    self.form = Some(ProviderFormState::from_preset(
                        preset,
                        self.presets_idx + 1,
                    ));
                    self.form_return = Route::Presets;
                    self.route = Route::Form;
                }
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
                        self.push_toast(ToastKind::Success, result.message);
                        self.refresh();
                    }
                    Err(e) => self.push_toast(ToastKind::Error, e),
                },
                1 => match daemon_stop() {
                    Ok(result) => {
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
        if key.code == KeyCode::Char('r') {
            self.refresh();
            self.push_toast(ToastKind::Info, i18n::toast_refreshed());
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

        let row_count = 5; // Language + 4 proxy fields
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
                }
            }
            KeyCode::Enter => {
                if self.settings_proxy_idx > 0 {
                    // Start editing a proxy field
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
            3 => proxy.target.clone().unwrap_or_default(),
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
                config.settings.proxy.target = if value.is_empty() { None } else { Some(value) };
            }
            4 => {
                config.settings.proxy.failover = if value.is_empty() {
                    vec![]
                } else {
                    value.split(',').map(|s| s.trim().to_string()).collect()
                };
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
        if matches!(key.code, KeyCode::Enter) {
            form.json_edit = TextInput::new(form.json_preview());
            form.json_editing = true;
        }
    }
}
