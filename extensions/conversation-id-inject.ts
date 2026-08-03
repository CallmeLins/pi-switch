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

export type ProviderHeadersEvent = { headers: RequestHeaders };

/**
 * The minimal session-manager surface this extension reads. Defined as a
 * concrete shape (not a generic) so the wiring stays trivially typed while
 * remaining decoupled from the full pi ExtensionContext.
 */
export type SessionIdProvider = {
  sessionManager: { getSessionId(): string | undefined };
};

/**
 * Build the `before_provider_headers` handler: it merges the injected headers
 * back into the event's headers in place (pi's contract for this hook) while
 * the pure function stays non-mutating. The session-id provider is injected
 * so the handler is testable without a live pi session.
 */
export function makeBeforeProviderHeadersHandler(
  getSessionId: (ctx: SessionIdProvider) => string | undefined,
): (event: ProviderHeadersEvent, ctx: SessionIdProvider) => void {
  return (event, ctx) => {
    Object.assign(event.headers, injectConversationId(event.headers, getSessionId(ctx)));
  };
}

export default function conversationIdInjectExtension(pi: ExtensionAPI): void {
  pi.on(
    "before_provider_headers",
    makeBeforeProviderHeadersHandler((ctx) => ctx.sessionManager.getSessionId()),
  );
}
