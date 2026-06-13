export const PRESETS = {
  openrouter: {
    name: "OpenRouter",
    description: "OpenAI-compatible gateway with many hosted models",
    websiteUrl: "https://openrouter.ai",
    api: "openai-completions",
    baseUrl: "https://openrouter.ai/api/v1",
    apiKey: "$OPENROUTER_API_KEY",
    models: [
      { id: "anthropic/claude-sonnet-4.5", name: "Claude Sonnet 4.5 (OpenRouter)", reasoning: true, contextWindow: 200000, maxTokens: 32000 },
      { id: "openai/gpt-5-mini", name: "GPT-5 Mini (OpenRouter)", reasoning: true, contextWindow: 400000, maxTokens: 32000 }
    ]
  },
  anthropic: {
    name: "Anthropic Official",
    description: "Official Anthropic Messages API",
    websiteUrl: "https://console.anthropic.com",
    api: "anthropic-messages",
    baseUrl: "https://api.anthropic.com",
    apiKey: "$ANTHROPIC_API_KEY",
    models: [
      { id: "claude-sonnet-4-5", name: "Claude Sonnet 4.5", reasoning: true, contextWindow: 200000, maxTokens: 32000 }
    ]
  },
  deepseek: {
    name: "DeepSeek",
    description: "OpenAI-compatible DeepSeek API",
    websiteUrl: "https://platform.deepseek.com",
    api: "openai-completions",
    baseUrl: "https://api.deepseek.com/v1",
    apiKey: "$DEEPSEEK_API_KEY",
    models: [
      { id: "deepseek-chat", name: "DeepSeek Chat", contextWindow: 128000, maxTokens: 8192 },
      { id: "deepseek-reasoner", name: "DeepSeek Reasoner", reasoning: true, contextWindow: 128000, maxTokens: 8192 }
    ]
  },
  siliconflow: {
    name: "SiliconFlow",
    description: "OpenAI-compatible SiliconFlow API",
    websiteUrl: "https://cloud.siliconflow.cn",
    api: "openai-completions",
    baseUrl: "https://api.siliconflow.cn/v1",
    apiKey: "$SILICONFLOW_API_KEY",
    models: [
      { id: "deepseek-ai/DeepSeek-V3", name: "DeepSeek V3 (SiliconFlow)", contextWindow: 128000, maxTokens: 8192 },
      { id: "deepseek-ai/DeepSeek-R1", name: "DeepSeek R1 (SiliconFlow)", reasoning: true, contextWindow: 128000, maxTokens: 8192 }
    ]
  },
  openai: {
    name: "OpenAI Official",
    description: "Official OpenAI Chat Completions API",
    websiteUrl: "https://platform.openai.com",
    api: "openai-completions",
    baseUrl: "https://api.openai.com/v1",
    apiKey: "$OPENAI_API_KEY",
    models: [
      { id: "gpt-5", name: "GPT-5", reasoning: true, contextWindow: 400000, maxTokens: 32000 },
      { id: "gpt-5-mini", name: "GPT-5 Mini", reasoning: true, contextWindow: 400000, maxTokens: 32000 }
    ]
  }
};

export function getPreset(id) {
  return PRESETS[id] || null;
}

export function listPresets() {
  return Object.entries(PRESETS).map(([id, preset]) => ({ id, ...preset }));
}

export function presetToProfile(preset, overrides = {}) {
  return {
    api: overrides.api || preset.api,
    baseUrl: overrides.baseUrl || preset.baseUrl,
    apiKey: overrides.apiKey || preset.apiKey,
    models: overrides.models?.length ? overrides.models : preset.models,
    preset: overrides.presetId,
    updatedAt: new Date().toISOString()
  };
}
