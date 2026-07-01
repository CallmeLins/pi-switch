use crate::config::{ModelEntry, ProviderProfile};
use crate::presets::Preset;

use super::i18n;
use super::text_edit::TextInput;

pub const API_CHOICES: [&str; 3] = [
    "openai-completions",
    "anthropic-messages",
    "google-generative-ai",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormMode {
    Add,
    Edit(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormFocus {
    Templates,
    Fields,
    JsonPreview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Name,
    Api,
    BaseUrl,
    ApiKey,
    Models,
}

impl FieldKind {
    pub const ALL: [FieldKind; 5] = [
        FieldKind::Name,
        FieldKind::Api,
        FieldKind::BaseUrl,
        FieldKind::ApiKey,
        FieldKind::Models,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            FieldKind::Name => i18n::field_name(),
            FieldKind::Api => i18n::field_api(),
            FieldKind::BaseUrl => i18n::field_base_url(),
            FieldKind::ApiKey => i18n::field_api_key(),
            FieldKind::Models => i18n::field_models(),
        }
    }

    pub fn is_text(&self) -> bool {
        !matches!(self, FieldKind::Api)
    }
}

pub struct ProviderFormState {
    pub mode: FormMode,
    pub focus: FormFocus,
    /// 0 = Custom, 1.. = preset index + 1 (Add mode only)
    pub template_idx: usize,
    pub field_idx: usize,
    pub editing: bool,
    pub name: TextInput,
    pub api_idx: usize,
    pub base_url: TextInput,
    pub api_key: TextInput,
    pub models: TextInput,
    pub preset_id: Option<String>,
    pub json_edit: TextInput,
    pub json_editing: bool,
    pub json_scroll: u16,
    /// Profile the form started from; preserves fields the form does not edit
    /// (headers, compat, proxy, ...).
    base: ProviderProfile,
    baseline: String,
}

fn api_index(api: &str) -> usize {
    API_CHOICES.iter().position(|c| *c == api).unwrap_or(0)
}

fn models_text(models: &[ModelEntry]) -> String {
    models
        .iter()
        .map(|m| m.id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn empty_profile() -> ProviderProfile {
    ProviderProfile {
        api: API_CHOICES[0].to_string(),
        base_url: String::new(),
        api_key: String::new(),
        models: vec![],
        preset: None,
        headers: None,
        auth_header: None,
        compat: None,
        proxy: false,
        updated_at: None,
        model_map: None,
        exposed_models: vec![],
        spoof: None,
    }
}

impl ProviderFormState {
    pub fn new_add() -> Self {
        let mut form = Self {
            mode: FormMode::Add,
            focus: FormFocus::Fields,
            template_idx: 0,
            field_idx: 0,
            editing: false,
            name: TextInput::default(),
            api_idx: 0,
            base_url: TextInput::default(),
            api_key: TextInput::default(),
            models: TextInput::default(),
            preset_id: None,
            json_edit: TextInput::default(),
            json_editing: false,
            json_scroll: 0,
            base: empty_profile(),
            baseline: String::new(),
        };
        form.baseline = form.snapshot();
        form
    }

    pub fn from_existing(name: &str, profile: &ProviderProfile) -> Self {
        let mut form = Self {
            mode: FormMode::Edit(name.to_string()),
            focus: FormFocus::Fields,
            template_idx: 0,
            field_idx: 0,
            editing: false,
            name: TextInput::new(name),
            api_idx: api_index(&profile.api),
            base_url: TextInput::new(profile.base_url.clone()),
            api_key: TextInput::new(profile.api_key.clone()),
            models: TextInput::new(models_text(&profile.models)),
            preset_id: profile.preset.clone(),
            json_edit: TextInput::default(),
            json_editing: false,
            json_scroll: 0,
            base: profile.clone(),
            baseline: String::new(),
        };
        form.baseline = form.snapshot();
        form
    }

    pub fn apply_preset(&mut self, preset: &Preset) {
        self.api_idx = api_index(&preset.api);
        self.base_url.set(preset.base_url.clone());
        self.api_key.set(preset.api_key.clone());
        self.models.set(models_text(&preset.models));
        self.preset_id = Some(preset.id.clone());
        self.base.models = preset.models.clone();
        if self.name.value.is_empty() {
            self.name.set(preset.id.clone());
        }
    }

    pub fn clear_template(&mut self) {
        self.api_idx = 0;
        self.base_url.clear();
        self.api_key.clear();
        self.models.clear();
        self.preset_id = None;
        self.base = empty_profile();
    }

    pub fn current_field(&self) -> FieldKind {
        FieldKind::ALL[self.field_idx.min(FieldKind::ALL.len() - 1)]
    }

    pub fn current_input_mut(&mut self) -> Option<&mut TextInput> {
        match self.current_field() {
            FieldKind::Name => Some(&mut self.name),
            FieldKind::BaseUrl => Some(&mut self.base_url),
            FieldKind::ApiKey => Some(&mut self.api_key),
            FieldKind::Models => Some(&mut self.models),
            FieldKind::Api => None,
        }
    }

    pub fn field_value(&self, field: FieldKind) -> String {
        match field {
            FieldKind::Name => self.name.value.clone(),
            FieldKind::Api => API_CHOICES[self.api_idx].to_string(),
            FieldKind::BaseUrl => self.base_url.value.clone(),
            FieldKind::ApiKey => self.api_key.value.clone(),
            FieldKind::Models => self.models.value.clone(),
        }
    }

    pub fn cycle_api(&mut self, forward: bool) {
        let len = API_CHOICES.len();
        self.api_idx = if forward {
            (self.api_idx + 1) % len
        } else {
            (self.api_idx + len - 1) % len
        };
    }

    pub fn next_focus(&mut self) {
        self.focus = match (self.focus, &self.mode) {
            (FormFocus::Templates, _) => FormFocus::Fields,
            (FormFocus::Fields, _) => FormFocus::JsonPreview,
            (FormFocus::JsonPreview, FormMode::Add) => FormFocus::Templates,
            (FormFocus::JsonPreview, FormMode::Edit(_)) => FormFocus::Fields,
        };
    }

    fn parsed_models(&self) -> Vec<ModelEntry> {
        self.models
            .value
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| {
                self.base
                    .models
                    .iter()
                    .find(|m| m.id == id)
                    .cloned()
                    .unwrap_or_else(|| ModelEntry {
                        id: id.to_string(),
                        ..Default::default()
                    })
            })
            .collect()
    }

    fn build_profile(&self) -> ProviderProfile {
        let mut profile = self.base.clone();
        profile.api = API_CHOICES[self.api_idx].to_string();
        profile.base_url = self.base_url.value.trim().to_string();
        profile.api_key = self.api_key.value.trim().to_string();
        profile.models = self.parsed_models();
        profile.preset = self.preset_id.clone();
        profile
    }

    pub fn profile_name(&self) -> String {
        self.name.value.trim().to_string()
    }

    pub fn validate(&self) -> Result<ProviderProfile, String> {
        if self.profile_name().is_empty() {
            return Err(if i18n::is_zh() { "名称为必填项".into() } else { "Name is required".into() });
        }
        let profile = self.build_profile();
        if profile.base_url.is_empty() {
            return Err(if i18n::is_zh() { "API 地址为必填项".into() } else { "Base URL is required".into() });
        }
        if profile.api_key.is_empty() {
            return Err(if i18n::is_zh() { "API 密钥为必填项".into() } else { "API Key is required".into() });
        }
        if profile.models.is_empty() {
            return Err(if i18n::is_zh() { "至少需要一个模型".into() } else { "At least one model is required".into() });
        }
        Ok(profile)
    }

    /// Try to apply edited JSON back to form fields. Returns error message on failure.
    pub fn apply_json_edit(&mut self) -> Result<(), String> {
        let json_str = self.json_edit.value.trim();
        if json_str.is_empty() {
            return Err(if i18n::is_zh() { "JSON 不能为空".to_string() } else { "JSON cannot be empty".to_string() });
        }
        let parsed: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("{}", e))?;
        let obj = parsed.as_object()
            .ok_or_else(|| if i18n::is_zh() { "JSON 必须是对象格式".to_string() } else { "JSON must be an object".to_string() })?;
        let profile: ProviderProfile = serde_json::from_value(serde_json::Value::Object(obj.clone()))
            .map_err(|e| format!("{}", e))?;

        self.name.set(if let Some(key) = obj.keys().next() { key.clone() } else { self.name.value.clone() });
        self.api_idx = api_index(&profile.api);
        self.base_url.set(profile.base_url.clone());
        self.api_key.set(profile.api_key.clone());
        self.models.set(models_text(&profile.models));
        self.base.models = profile.models.clone();
        self.json_editing = false;
        Ok(())
    }

    pub fn json_preview(&self) -> String {
        let mut profile = self.build_profile();
        profile.updated_at = None;
        let mut map = serde_json::Map::new();
        map.insert(
            self.profile_name(),
            serde_json::to_value(&profile).unwrap_or_default(),
        );
        serde_json::to_string_pretty(&serde_json::Value::Object(map)).unwrap_or_default()
    }

    fn snapshot(&self) -> String {
        format!("{}\n{}", self.profile_name(), self.json_preview())
    }

    pub fn is_dirty(&self) -> bool {
        self.snapshot() != self.baseline
    }
}
