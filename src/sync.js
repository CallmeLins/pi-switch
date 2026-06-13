import { createCipheriv, createDecipheriv, randomBytes, createHash } from "node:crypto";
import { readFile, writeFile, mkdir, access } from "node:fs/promises";
import { join, basename } from "node:path";
import { homedir } from "node:os";
import { execSync } from "node:child_process";
import { CONFIG_PATH, CONFIG_DIR, loadConfig, saveConfig, backupFile, exists } from "./core.js";

const EXPORT_DIR = join(homedir(), ".pi-switch", "exports");
const ALGORITHM = "aes-256-cbc";

// ---- Encryption helpers ----

function deriveKey(passphrase) {
  return createHash("sha256").update(passphrase).digest();
}

function encrypt(text, passphrase) {
  const key = deriveKey(passphrase);
  const iv = randomBytes(16);
  const cipher = createCipheriv(ALGORITHM, key, iv);
  const encrypted = Buffer.concat([cipher.update(text, "utf8"), cipher.final()]);
  return JSON.stringify({
    v: 1,
    iv: iv.toString("base64"),
    data: encrypted.toString("base64"),
  });
}

function decrypt(encryptedJson, passphrase) {
  const { v, iv, data } = JSON.parse(encryptedJson);
  if (v !== 1) throw new Error("unsupported encryption version");
  const key = deriveKey(passphrase);
  const decipher = createDecipheriv(ALGORITHM, key, Buffer.from(iv, "base64"));
  const decrypted = Buffer.concat([decipher.update(Buffer.from(data, "base64")), decipher.final()]);
  return decrypted.toString("utf8");
}

// ---- Export ----

export async function exportConfig(passphrase) {
  if (!passphrase) throw new Error("passphrase required for encrypted export");
  if (passphrase.length < 8) throw new Error("passphrase must be at least 8 characters");

  if (!(await exists(CONFIG_PATH))) {
    throw new Error(`No config found at ${CONFIG_PATH}. Run pi-switch init first.`);
  }

  const configText = await readFile(CONFIG_PATH, "utf8");
  const encrypted = encrypt(configText, passphrase);

  await mkdir(EXPORT_DIR, { recursive: true });
  const ts = new Date().toISOString().replaceAll(":", "-").replaceAll(".", "-");
  const exportPath = join(EXPORT_DIR, `pi-switch-export-${ts}.json`);
  await writeFile(exportPath, encrypted + "\n", "utf8");

  return {
    path: exportPath,
    size: Buffer.byteLength(encrypted, "utf8"),
    message: `Config exported (encrypted) to ${exportPath}`,
  };
}

// ---- Import ----

export async function importConfig(filePath, passphrase) {
  if (!passphrase) throw new Error("passphrase required for encrypted import");
  if (!(await exists(filePath))) throw new Error(`file not found: ${filePath}`);

  const encryptedText = await readFile(filePath, "utf8");
  let configText;
  try {
    configText = decrypt(encryptedText, passphrase);
  } catch {
    throw new Error("decryption failed — wrong passphrase or corrupted file");
  }

  let newConfig;
  try {
    newConfig = JSON.parse(configText);
  } catch {
    throw new Error("decrypted data is not valid JSON");
  }

  // Backup existing config before overwriting
  const backupPath = await backupFile(CONFIG_PATH, "pre-import");

  // Sanitize API keys — never import raw keys from untrusted exports
  let sanitizedCount = 0;
  if (newConfig.profiles) {
    for (const [name, profile] of Object.entries(newConfig.profiles)) {
      if (profile.apiKey && !profile.apiKey.startsWith("$")) {
        // If it looks like a raw key, replace with env var placeholder
        const envName = `PI_SWITCH_${name.toUpperCase().replace(/-/g, "_")}_API_KEY`;
        profile.apiKey = `$${envName}`;
        sanitizedCount++;
      }
    }
  }

  await saveConfig(newConfig);

  return {
    backup: backupPath,
    profiles: Object.keys(newConfig.profiles || {}).length,
    sanitizedKeys: sanitizedCount,
    message: `Imported ${Object.keys(newConfig.profiles || {}).length} profile(s) from ${basename(filePath)}`,
  };
}

// ---- Deep link ----

export function getProviderUrl(name) {
  const presetUrls = {
    openrouter: "https://openrouter.ai/settings/keys",
    anthropic: "https://console.anthropic.com/settings/keys",
    deepseek: "https://platform.deepseek.com/api_keys",
    siliconflow: "https://cloud.siliconflow.cn/account/ak",
    openai: "https://platform.openai.com/api-keys",
    google: "https://aistudio.google.com/app/apikey",
  };

  if (presetUrls[name]) {
    return { url: presetUrls[name], label: `${name} API keys` };
  }

  return null;
}

export async function getProfileInfo(name) {
  const config = await loadConfig();
  const profile = config.profiles?.[name];
  if (!profile) throw new Error(`unknown profile '${name}'`);

  const presetLink = profile.preset ? getProviderUrl(profile.preset) : null;

  return {
    name,
    api: profile.api,
    baseUrl: profile.baseUrl,
    apiKey: profile.apiKey,
    isCurrent: config.current === name,
    isProxy: Boolean(profile.proxy),
    models: (profile.models || []).map((m) => m.id),
    links: {
      baseUrl: profile.baseUrl,
      ...(presetLink ? { manageKeys: presetLink.url } : {}),
      docs: profile.api === "openai-completions"
        ? "https://platform.openai.com/docs/api-reference/chat"
        : "https://docs.anthropic.com/en/api/messages",
    },
  };
}

export async function openProvider(name) {
  const link = getProviderUrl(name);
  if (!link) {
    const profile = await getProfileInfo(name);
    if (profile.baseUrl) {
      // Open the base URL if no preset link
      const cmd = process.platform === "darwin" ? "open"
        : process.platform === "win32" ? "start"
        : "xdg-open";
      try {
        execSync(`${cmd} "${profile.baseUrl}"`, { stdio: "ignore" });
        return { opened: profile.baseUrl, label: name };
      } catch {
        return { url: profile.baseUrl, label: name, note: "Could not open browser. Visit this URL manually." };
      }
    }
    return null;
  }

  const cmd = process.platform === "darwin" ? "open"
    : process.platform === "win32" ? "start"
    : "xdg-open";
  try {
    execSync(`${cmd} "${link.url}"`, { stdio: "ignore" });
    return { opened: link.url, label: link.label };
  } catch {
    return { url: link.url, label: link.label, note: "Could not open browser. Visit this URL manually." };
  }
}
