import { test } from "node:test";
import assert from "node:assert/strict";
import { injectConversationId, injectConversationName, makeBeforeProviderHeadersHandler, firstUserMessageText, resolveSessionName, TITLE_MAX_LEN, type SessionIdProvider } from "./conversation-id-inject.ts";

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
    sessionManager: { getSessionId: () => "uuid-9", getSessionName: () => undefined, getEntries: () => [] },
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
      getEntries: () => [],
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
    sessionManager: { getSessionId: () => undefined, getSessionName: () => undefined, getEntries: () => [] },
  };
  handler(event, ctx);
  assert.deepEqual(event.headers, { authorization: "Bearer x" });
});

test("handler falls back to the first user message when no explicit name", () => {
  const handler = makeBeforeProviderHeadersHandler((ctx) => ({
    id: ctx.sessionManager.getSessionId(),
    name: ctx.sessionManager.getSessionName(),
  }));
  const event = { headers: { authorization: "Bearer x" } };
  const ctx: SessionIdProvider = {
    sessionManager: {
      getSessionId: () => "uuid-9",
      getSessionName: () => undefined,
      getEntries: () => [{ role: "user", content: "帮我修复 cost 计算" }],
    },
  };
  handler(event, ctx);
  assert.equal(event.headers["x-conversation-id"], "uuid-9");
  assert.equal(event.headers["x-conversation-name"], "帮我修复 cost 计算");
  assert.equal(event.headers.authorization, "Bearer x");
});

test("handler keeps the existing name header when neither name nor title exists", () => {
  const handler = makeBeforeProviderHeadersHandler((ctx) => ({
    id: ctx.sessionManager.getSessionId(),
    name: ctx.sessionManager.getSessionName(),
  }));
  const event = { headers: { authorization: "Bearer x", "x-conversation-name": "existing" } };
  const ctx: SessionIdProvider = {
    sessionManager: {
      getSessionId: () => "uuid-9",
      getSessionName: () => undefined,
      getEntries: () => [],
    },
  };
  handler(event, ctx);
  assert.equal(event.headers["x-conversation-name"], "existing");
});

// ─── firstUserMessageText ─────────────────────────────────

test("returns the first non-empty user message text", () => {
  const entries = [
    { role: "user", content: "hello" },
    { role: "assistant", content: [{ type: "text", text: "hi" }] },
  ];
  assert.equal(firstUserMessageText(entries), "hello");
});

test("skips empty user messages and uses the next non-empty one", () => {
  const entries = [
    { role: "user", content: "   " },
    { role: "user", content: "real question" },
  ];
  assert.equal(firstUserMessageText(entries), "real question");
});

test("joins text blocks of an array content", () => {
  const entries = [
    {
      role: "user",
      content: [
        { type: "text", text: "first" },
        { type: "text", text: "second" },
      ],
    },
  ];
  assert.equal(firstUserMessageText(entries), "first second");
});

test("ignores non-text blocks", () => {
  const entries = [
    {
      role: "user",
      content: [
        { type: "image", data: "..." },
        { type: "text", text: "only text counts" },
      ],
    },
  ];
  assert.equal(firstUserMessageText(entries), "only text counts");
});

test("returns undefined when no user message has text", () => {
  assert.equal(firstUserMessageText([]), undefined);
  assert.equal(firstUserMessageText([{ role: "assistant", content: "hi" }]), undefined);
  assert.equal(
    firstUserMessageText([{ role: "user", content: [{ type: "image", data: "x" }] }]),
    undefined,
  );
});

// ─── resolveSessionName ──────────────────────────────────

test("prefers the explicit name over the first message", () => {
  assert.equal(resolveSessionName(" 我的会话 ", [{ role: "user", content: "hello" }]), " 我的会话 ");
});

test("falls back to the sanitized first message title", () => {
  assert.equal(resolveSessionName(undefined, [{ role: "user", content: "hello world" }]), "hello world");
  assert.equal(resolveSessionName("", [{ role: "user", content: "hello world" }]), "hello world");
  assert.equal(resolveSessionName("   ", [{ role: "user", content: "hello world" }]), "hello world");
});

test("sanitizes control characters and trims the title", () => {
  const title = resolveSessionName(undefined, [{ role: "user", content: "line1\nline2\tend" }]);
  assert.equal(title, "line1 line2 end");
});

test("truncates long titles to TITLE_MAX_LEN characters", () => {
  const long = "x".repeat(200);
  const title = resolveSessionName(undefined, [{ role: "user", content: long }]);
  assert.equal(title, "x".repeat(TITLE_MAX_LEN));
  assert.equal(title.length, TITLE_MAX_LEN);
});

test("returns undefined when nothing is available", () => {
  assert.equal(resolveSessionName(undefined, []), undefined);
  assert.equal(resolveSessionName(undefined, [{ role: "user", content: "   " }]), undefined);
  assert.equal(resolveSessionName(undefined, [{ role: "user", content: "\n\t" }]), undefined);
});
