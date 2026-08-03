# Standards Review — conversation-id-inject (WIP vs bfeb6ec)

## src-rust/proxy.rs

**(a) Documented-standard violations**
- Violates `spec.md` Out of Scope: "pi-switch proxy / stats / WebUI / TUI 的任何改动" and the Solution claim "故无需 pi-switch 侧任何改动". The WIP adds the `x-opencode-session` fallback + a test to `conversation_id_of` — a pi-switch proxy change the spec explicitly excluded.
- The spec's premise for "零改动" is false: it states "ADR-0002 已确立探测优先级", but `docs/adr/0002-conversation-boundary-from-client-id.md` lists only `x-conversation-id` header then body `conversation_id` — no `x-opencode-session`. The new code and CONTEXT.md (3 sources) now agree, leaving ADR-0002 stale; spec.md's "无新增 ADR" leaves that contradiction unrecorded.

**(b) Smells / judgement calls**
- New test's "both headers" case re-asserts precedence already covered by `conversation_id_prefers_header_over_body` (overlap is small but real).
- Header-name knowledge (`x-conversation-id` / `x-opencode-session`) now lives in three places — extension, proxy, CONTEXT.md — a mild Divergent Change if the name ever shifts.

## extensions/conversation-id-inject.ts

**(a) Documented-standard violations**
- None found vs spec.md; doc comment matches the `extensions.md` contract ("Handlers mutate `event.headers` in place"; retries reuse same headers).

**(b) Smells / judgement calls**
- `makeBeforeProviderHeadersHandler<TContext>` + explicit cast at wiring site: `makeBeforeProviderHeadersHandler<{ sessionManager: { getSessionId(): string | undefined } }>`. Structurally sound only because the real `ExtensionContext` (per docs example) satisfies that shape — but it is an *unchecked* cast: the repo has no typecheck step (no tsconfig/tsc script) and `@earendil-works/pi-coding-agent` is not a declared dependency, so `import type { ExtensionAPI }` would not resolve under `tsc`. Type safety is asserted, never verified.
- Duplicated Code: the `{ sessionManager: { getSessionId(): string | undefined } }` shape literal appears twice (wiring site and test annotation).
- Speculative Generality: `TContext` has exactly one instantiation; a concrete minimal ctx type (cast once at `pi.on`) would be simpler.
- `Object.assign(event.headers, injectConversationId(...))` copies then merges back — indirect but defensible for keeping the pure fn non-mutating.

## extensions/conversation-id-inject.test.ts

**(a) Documented-standard violations**
- None; tests match spec.md Testing Decisions (behavioral, no pi runtime).

**(b) Smells / judgement calls**
- `handler(event, {} as never)` — `as never` type escape; passes only because the handler never touches `ctx`. Fragile if the handler later reads ctx.
- Handler tests re-assert what the pure-function tests already cover (mild redundancy, acceptable as wiring coverage). No tautological tests; non-mutation and blank/undefined cases are well covered.

## package.json

**(a) Documented-standard violations**
- `engines: "node": ">=20"` contradicts the test toolchain: `node --test "extensions/**/*.test.ts"` runs `.ts` via type stripping, which spec.md itself pins at "node ≥23.6". No `--experimental-strip-types` flag and no engines bump → `npm test` breaks on node 20–22.
- `pi.extensions` registration order is fine: `index.ts` registers no header hooks, so no interaction with `before_provider_headers`.

**(b) Smells**
- None beyond the above.

## Verdict
Implementation is clean and well-tested; the blocking standard gap is the proxy.rs change contradicting spec.md's explicit Out-of-Scope / "零改动" premise (itself built on a misreading of ADR-0002), plus the engines-vs-toolchain mismatch. The `TContext` cast is sound-but-unverifiable given no typechecking pipeline.
