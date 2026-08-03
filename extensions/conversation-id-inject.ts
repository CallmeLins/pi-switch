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
 * Pure injection logic for the conversation display name: return a new
 * headers object with `x-conversation-name` set to the current session
 * name, overriding any existing value. Blank names are not injected. The
 * caller's headers object is never mutated.
 */
export function injectConversationName(
  headers: RequestHeaders,
  sessionName: string | undefined,
): RequestHeaders {
  if (sessionName == null || sessionName.trim() === "") {
    return { ...headers };
  }
  return { ...headers, "x-conversation-name": sessionName };
}

export type ProviderHeadersEvent = { headers: RequestHeaders };

/**
 * The minimal session-manager surface this extension reads. Defined as a
 * concrete shape (not a generic) so the wiring stays trivially typed while
 * remaining decoupled from the full pi ExtensionContext.
 */
export type SessionIdProvider = {
  sessionManager: {
    getSessionId(): string | undefined;
    getSessionName(): string | undefined;
  };
};

export type SessionInfo = {
  id?: string;
  name?: string;
};

/**
 * Build the `before_provider_headers` handler: it merges the injected headers
 * back into the event's headers in place (pi's contract for this hook) while
 * the pure functions stay non-mutating. The session-info provider is injected
 * so the handler is testable without a live pi session.
 */
export function makeBeforeProviderHeadersHandler(
  getSession: (ctx: SessionIdProvider) => SessionInfo,
): (event: ProviderHeadersEvent, ctx: SessionIdProvider) => void {
  return (event, ctx) => {
    const { id, name } = getSession(ctx);
    Object.assign(event.headers, injectConversationId(event.headers, id));
    Object.assign(event.headers, injectConversationName(event.headers, name));
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
