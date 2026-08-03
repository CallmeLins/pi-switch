import { test } from "node:test";
import assert from "node:assert/strict";
import { injectConversationId, makeBeforeProviderHeadersHandler, type SessionIdProvider } from "./conversation-id-inject.ts";

test("injects a non-empty session id and overrides an existing header", () => {
  const headers = { "x-conversation-id": "stale", authorization: "Bearer x" };
  const result = injectConversationId(headers, "abc-123");
  assert.equal(result["x-conversation-id"], "abc-123");
  assert.equal(result.authorization, "Bearer x");
});

test("skips injection when the session id is empty or blank", () => {
  const headers = { "x-conversation-id": "existing" };
  assert.equal(injectConversationId(headers, "")["x-conversation-id"], "existing");
  assert.equal(injectConversationId(headers, "   ")["x-conversation-id"], "existing");
  assert.equal(injectConversationId(headers, undefined)["x-conversation-id"], "existing");
});

test("leaves other headers untouched and returns a new object", () => {
  const headers = { authorization: "Bearer x", "x-custom": "v" };
  const result = injectConversationId(headers, "abc");
  assert.deepEqual(result, {
    authorization: "Bearer x",
    "x-custom": "v",
    "x-conversation-id": "abc",
  });
  assert.notEqual(result, headers, "must not mutate the caller's object");
  assert.deepEqual(headers, { authorization: "Bearer x", "x-custom": "v" });
});

test("handler wires the session id provider into the headers", () => {
  const handler = makeBeforeProviderHeadersHandler((ctx) => ctx.sessionManager.getSessionId());
  const event = { headers: { authorization: "Bearer x" } };
  handler(event, { sessionManager: { getSessionId: () => "uuid-9" } });
  assert.equal(event.headers["x-conversation-id"], "uuid-9");
  assert.equal(event.headers.authorization, "Bearer x");
});

test("handler skips injection when the provider yields no session id", () => {
  const handler = makeBeforeProviderHeadersHandler(() => undefined);
  const event = { headers: { authorization: "Bearer x" } };
  const ctx: SessionIdProvider = { sessionManager: { getSessionId: () => undefined } };
  handler(event, ctx);
  assert.deepEqual(event.headers, { authorization: "Bearer x" });
});
