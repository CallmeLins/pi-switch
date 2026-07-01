use crate::config::{ModelEntry, ProviderProfile};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub website_url: String,
    pub api: String,
    pub base_url: String,
    pub api_key: String,
    pub models: Vec<ModelEntry>,
}

pub fn all_presets() -> Vec<Preset> {
    vec![
        Preset {
            id: "openrouter".into(),
            name: "OpenRouter".into(),
            description: "OpenAI-compatible gateway with many hosted models".into(),
            website_url: "https://openrouter.ai".into(),
            api: "openai-completions".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            api_key: "$OPENROUTER_API_KEY".into(),
            models: vec![
                ModelEntry {
                    id: "anthropic/claude-sonnet-4.5".into(),
                    name: Some("Claude Sonnet 4.5 (OpenRouter)".into()),
                    context_window: 200000,
                    max_tokens: 32000,
                    ..Default::default()
                },
                ModelEntry {
                    id: "openai/gpt-5-mini".into(),
                    name: Some("GPT-5 Mini (OpenRouter)".into()),
                    context_window: 400000,
                    max_tokens: 32000,
                    ..Default::default()
                },
            ],
        },
        Preset {
            id: "anthropic".into(),
            name: "Anthropic Official".into(),
            description: "Official Anthropic Messages API".into(),
            website_url: "https://console.anthropic.com".into(),
            api: "anthropic-messages".into(),
            base_url: "https://api.anthropic.com".into(),
            api_key: "$ANTHROPIC_API_KEY".into(),
            models: vec![
                ModelEntry {
                    id: "claude-sonnet-4-5".into(),
                    name: Some("Claude Sonnet 4.5".into()),
                    context_window: 200000,
                    max_tokens: 32000,
                    ..Default::default()
                },
            ],
        },
        Preset {
            id: "deepseek".into(),
            name: "DeepSeek".into(),
            description: "OpenAI-compatible DeepSeek API".into(),
            website_url: "https://platform.deepseek.com".into(),
            api: "openai-completions".into(),
            base_url: "https://api.deepseek.com/v1".into(),
            api_key: "$DEEPSEEK_API_KEY".into(),
            models: vec![
                ModelEntry {
                    id: "deepseek-chat".into(),
                    name: Some("DeepSeek Chat".into()),
                    context_window: 128000,
                    max_tokens: 8192,
                    ..Default::default()
                },
                ModelEntry {
                    id: "deepseek-reasoner".into(),
                    name: Some("DeepSeek Reasoner".into()),
                    context_window: 128000,
                    max_tokens: 8192,
                    ..Default::default()
                },
            ],
        },
        Preset {
            id: "siliconflow".into(),
            name: "SiliconFlow".into(),
            description: "OpenAI-compatible SiliconFlow API".into(),
            website_url: "https://cloud.siliconflow.cn".into(),
            api: "openai-completions".into(),
            base_url: "https://api.siliconflow.cn/v1".into(),
            api_key: "$SILICONFLOW_API_KEY".into(),
            models: vec![
                ModelEntry {
                    id: "deepseek-ai/DeepSeek-V3".into(),
                    name: Some("DeepSeek V3 (SiliconFlow)".into()),
                    context_window: 128000,
                    max_tokens: 8192,
                    ..Default::default()
                },
                ModelEntry {
                    id: "deepseek-ai/DeepSeek-R1".into(),
                    name: Some("DeepSeek R1 (SiliconFlow)".into()),
                    context_window: 128000,
                    max_tokens: 8192,
                    ..Default::default()
                },
            ],
        },
        Preset {
            id: "openai".into(),
            name: "OpenAI Official".into(),
            description: "Official OpenAI Chat Completions API".into(),
            website_url: "https://platform.openai.com".into(),
            api: "openai-completions".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: "$OPENAI_API_KEY".into(),
            models: vec![
                ModelEntry {
                    id: "gpt-5".into(),
                    name: Some("GPT-5".into()),
                    context_window: 400000,
                    max_tokens: 32000,
                    ..Default::default()
                },
                ModelEntry {
                    id: "gpt-5-mini".into(),
                    name: Some("GPT-5 Mini".into()),
                    context_window: 400000,
                    max_tokens: 32000,
                    ..Default::default()
                },
            ],
        },
    ]
}

pub fn get_preset(id: &str) -> Option<Preset> {
    all_presets().into_iter().find(|p| p.id == id)
}

pub fn preset_to_profile(preset: &Preset, api_key: Option<&str>, models: Option<Vec<ModelEntry>>) -> ProviderProfile {
    ProviderProfile {
        api: preset.api.clone(),
        base_url: preset.base_url.clone(),
        api_key: api_key.unwrap_or(&preset.api_key).to_string(),
        models: models.unwrap_or_else(|| preset.models.clone()),
        preset: Some(preset.id.clone()),
        headers: None,
        auth_header: None,
        compat: None,
        proxy: false,
        updated_at: Some(chrono::Utc::now().to_rfc3339()),
        model_map: None,
        exposed_models: vec![],
        spoof: None,
    }
}
