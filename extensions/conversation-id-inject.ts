import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export type RequestHeaders = Record<string, string | null>;

/**
 * Pure injection logic: return a new headers object with `x-conversation-id`
 * set to the current session id, overriding any existing value. The caller's
 * headers object is never mutated.
 */
export function injectConversationId(
  headers: RequestHeaders,
  sessionId: string | undefined,
): RequestHeaders {
  if (sessionId == null || sessionId.trim() === "") {
    return { ...headers };
  }
  return { ...headers, "x-conversation-id": sessionId };
}

/**
 * Percent-encode every code point above Latin-1 (> 255) so the value stays
 * a valid ByteString for HTTP headers: undici's Headers rejects non-Latin1
 * characters with a TypeError and the request dies before being sent.
 * ASCII/Latin1 characters are kept as-is, so plain names round-trip
 * unchanged. The `u` flag matches astral characters (surrogate pairs) as
 * single code points so `encodeURIComponent` never sees an isolated surrogate.
 */
function encodeHeaderValue(value: string): string {
  return value.replace(/[^\x00-\xff]/gu, (ch) => encodeURIComponent(ch));
}

/**
 * Pure injection logic for the conversation display name: return a new
 * headers object with `x-conversation-name` set to the current session
 * name, overriding any existing value. Non-Latin1 characters are
 * percent-encoded so the header value stays HTTP-safe. Blank names are not
 * injected. The caller's headers object is never mutated.
 */
export function injectConversationName(
  headers: RequestHeaders,
  sessionName: string | undefined,
): RequestHeaders {
  if (sessionName == null || sessionName.trim() === "") {
    return { ...headers };
  }
  return { ...headers, "x-conversation-name": encodeHeaderValue(sessionName) };
}

export type ProviderHeadersEvent = { headers: RequestHeaders };

/**
 * The minimal session-manager surface this extension reads. Defined as a
 * concrete shape (not a generic) so the wiring stays trivially typed while
 * remaining decoupled from the full pi ExtensionContext.
 */
/**
 * The minimal session-manager surface this extension reads. Defined as a
 * concrete shape (not a generic) so the wiring stays trivially typed while
 * remaining decoupled from the full pi ExtensionContext.
 */
export type SessionIdProvider = {
  sessionManager: {
    getSessionId(): string | undefined;
    getSessionName(): string | undefined;
    getEntries(): SessionEntry[];
  };
};

/**
 * Loose shape of a session entry (pi's `AgentMessage`). Only the fields this
 * extension reads are typed, so it stays decoupled from pi internals.
 */
export type SessionEntryContent = string | Array<{ type?: string; text?: string }>;
export type SessionEntry = {
  type?: string;
  // Legacy flat shape (extension tests / early callers).
  role?: string;
  content?: SessionEntryContent;
  // Real pi SessionManager entry shape: role/content live under `message`.
  message?: {
    role?: string;
    content?: SessionEntryContent;
  };
};

export const TITLE_MAX_LEN = 60;

/**
 * Extract the text of a session entry's content: plain strings pass through;
 * block arrays contribute every `text` block (image/thinking/toolCall blocks
 * are ignored).
 */
function textOf(content: SessionEntryContent | undefined): string {
  if (typeof content === "string") {
    return content;
  }
  if (Array.isArray(content)) {
    return content
      .filter(
        (b): b is { type: string; text: string } =>
          b?.type === "text" && typeof b.text === "string",
      )
      .map((b) => b.text)
      .join(" ");
  }
  return "";
}

/**
 * Text of the first non-empty user message in a session, or `undefined` when
 * there is none. The returned text is trimmed but not sanitized/truncated —
 * callers decide how to present it.
 */
/**
 * Text of the first non-empty user message in a session, or `undefined` when
 * there is none. The returned text is trimmed but not sanitized/truncated —
 * callers decide how to present it. Handles both the legacy flat entry
 * (`role`/`content` at the top level) and the real pi SessionManager entry
 * shape (`role`/`content` nested under `message`).
 */
export function firstUserMessageText(entries: SessionEntry[]): string | undefined {
  for (const entry of entries) {
    const role = entry.message?.role ?? entry.role;
    if (role !== "user") {
      continue;
    }
    const text = textOf(entry.message?.content ?? entry.content).trim();
    if (text) {
      return text;
    }
  }
  return undefined;
}

/**
 * Resolve the conversation display name: an explicit non-blank name wins;
 * otherwise fall back to the first user message as a readable title, with
 * control characters collapsed to spaces and the result truncated to
 * `TITLE_MAX_LEN`. Returns `undefined` when neither source yields text.
 */
export function resolveSessionName(
  name: string | undefined,
  entries: SessionEntry[],
): string | undefined {
  if (name != null && name.trim() !== "") {
    return name;
  }
  const title = firstUserMessageText(entries);
  if (!title) {
    return undefined;
  }
  const sanitized = title.replace(/[\x00-\x1f\x7f]+/g, " ").trim();
  if (!sanitized) {
    return undefined;
  }
  return sanitized.slice(0, TITLE_MAX_LEN);
}
export type SessionInfo = {
  id?: string;
  name?: string;
};

/**
 * The subagent-folding env surface this extension reads. Subagent processes
 * are spawned by pi-subagents with `PI_SUBAGENT_DEPTH >= 1` and inherit the
 * parent's `PI_PARENT_SESSION_ID`; the parent process advertises its session
 * id through that variable so child requests fold into the same conversation.
 */
export type RequestEnv = {
  PI_SUBAGENT_DEPTH?: string;
  PI_PARENT_SESSION_ID?: string;
};

export type RequestInjection = {
  conversationId?: string;
  conversationName?: string;
};

/**
 * Decide what to inject for the current process: a subagent (depth > 0)
 * folds its requests into the parent conversation (parent id only, no name
 * so the aggregate label stays the parent's); the parent process injects its
 * own id and resolved name.
 */
export function resolveRequestInjection(
  ownId: string | undefined,
  ownName: string | undefined,
  env: RequestEnv,
): RequestInjection {
  const depth = Number.parseInt(env.PI_SUBAGENT_DEPTH ?? "0", 10) || 0;
  if (depth > 0) {
    return env.PI_PARENT_SESSION_ID
      ? { conversationId: env.PI_PARENT_SESSION_ID }
      : {};
  }
  return {
    ...(ownId ? { conversationId: ownId } : {}),
    ...(ownName ? { conversationName: ownName } : {}),
  };
}

/**
 * Build the `before_provider_headers` handler: it merges the injected headers
 * back into the event's headers in place (pi's contract for this hook) while
 * the pure functions stay non-mutating. The session-info provider is injected
 * so the handler is testable without a live pi session. The parent process
 * additionally advertises its session id via `PI_PARENT_SESSION_ID` so spawned
 * subagents (which inherit the env) fold their requests into it.
 */
export function makeBeforeProviderHeadersHandler(
  getSession: (ctx: SessionIdProvider) => SessionInfo,
): (event: ProviderHeadersEvent, ctx: SessionIdProvider) => void {
  return (event, ctx) => {
    const { id, name } = getSession(ctx);
    const sessionName = resolveSessionName(name, ctx.sessionManager.getEntries());
    const env = process.env as RequestEnv;
    const { conversationId, conversationName } = resolveRequestInjection(id, sessionName, env);
    const isParent = (Number.parseInt(env.PI_SUBAGENT_DEPTH ?? "0", 10) || 0) === 0;
    if (isParent && conversationId) {
      process.env.PI_PARENT_SESSION_ID = conversationId;
    }
    Object.assign(event.headers, injectConversationId(event.headers, conversationId));
    Object.assign(event.headers, injectConversationName(event.headers, conversationName));
  };
}

export default function conversationIdInjectExtension(pi: ExtensionAPI): void {
  pi.on(
    "before_provider_headers",
    makeBeforeProviderHeadersHandler((ctx) => ({
      id: ctx.sessionManager.getSessionId(),
      name: ctx.sessionManager.getSessionName(),
    })),
  );
}
