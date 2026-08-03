import { test } from "node:test";
import assert from "node:assert/strict";
import { injectConversationId, injectConversationName, makeBeforeProviderHeadersHandler, type SessionIdProvider } from "./conversation-id-inject.ts";

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

test("injects a non-empty conversation name and overrides an existing header", () => {
  const headers = { "x-conversation-name": "stale", authorization: "Bearer x" };
  const result = injectConversationName(headers, "我的对话");
  assert.equal(result["x-conversation-name"], "我的对话");
  assert.equal(result.authorization, "Bearer x");
});

test("skips name injection when the conversation name is empty or blank", () => {
  const headers = { "x-conversation-name": "existing" };
  assert.equal(injectConversationName(headers, "")["x-conversation-name"], "existing");
  assert.equal(injectConversationName(headers, "   ")["x-conversation-name"], "existing");
  assert.equal(injectConversationName(headers, undefined)["x-conversation-name"], "existing");
});

test("name injection leaves other headers untouched and returns a new object", () => {
  const headers = { authorization: "Bearer x", "x-conversation-id": "abc" };
  const result = injectConversationName(headers, "对话A");
  assert.deepEqual(result, {
    authorization: "Bearer x",
    "x-conversation-id": "abc",
    "x-conversation-name": "对话A",
  });
  assert.notEqual(result, headers, "must not mutate the caller's object");
  assert.deepEqual(headers, { authorization: "Bearer x", "x-conversation-id": "abc" });
});

test("handler wires the session id provider into the headers", () => {
  const handler = makeBeforeProviderHeadersHandler((ctx) => ({ id: ctx.sessionManager.getSessionId() }));
  const event = { headers: { authorization: "Bearer x" } };
  const ctx: SessionIdProvider = {
    sessionManager: { getSessionId: () => "uuid-9", getSessionName: () => undefined },
  };
  handler(event, ctx);
  assert.equal(event.headers["x-conversation-id"], "uuid-9");
  assert.equal(event.headers.authorization, "Bearer x");
});

test("handler injects both conversation id and name from the provider", () => {
  const handler = makeBeforeProviderHeadersHandler((ctx) => ({
    id: ctx.sessionManager.getSessionId(),
    name: ctx.sessionManager.getSessionName(),
  }));
  const event = { headers: { authorization: "Bearer x" } };
  const ctx: SessionIdProvider = {
    sessionManager: {
      getSessionId: () => "uuid-9",
      getSessionName: () => "对话A",
    },
  };
  handler(event, ctx);
  assert.equal(event.headers["x-conversation-id"], "uuid-9");
  assert.equal(event.headers["x-conversation-name"], "对话A");
  assert.equal(event.headers.authorization, "Bearer x");
});

test("handler skips injection when the provider yields no session id", () => {
  const handler = makeBeforeProviderHeadersHandler(() => ({}));
  const event = { headers: { authorization: "Bearer x" } };
  const ctx: SessionIdProvider = {
    sessionManager: { getSessionId: () => undefined, getSessionName: () => undefined },
  };
  handler(event, ctx);
  assert.deepEqual(event.headers, { authorization: "Bearer x" });
});
