import type { ProviderProfile, ResponsesMode } from "../types";

export function effectiveResponsesMode(profile: Pick<ProviderProfile, "api" | "responsesMode">): ResponsesMode {
  if (profile.responsesMode !== "auto" && profile.responsesMode) return profile.responsesMode;
  if (profile.api === "openai-responses") return "passthrough";
  if (profile.api === "openai-completions") return "convert";
  return "auto";
}

export function responsesModeError(api: string, mode: ResponsesMode): string | null {
  if (mode === "passthrough" && api !== "openai-responses") {
    return "passthrough requires openai-responses";
  }
  if (mode === "convert" && api !== "openai-completions") {
    return "convert requires openai-completions";
  }
  return null;
}
