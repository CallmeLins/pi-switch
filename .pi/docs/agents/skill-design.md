# Skill Design Spec

The design rules every skill in this repo must obey. Apply them when writing a new skill or editing an existing one. Terms are defined once in [`CONTEXT.md`](../../../.opencode/CONTEXT.md) — reference them there, never restate the definition.

This spec exists because of a real incident: a long-horizon skill run on a flash-class model stopped its turn at "announce the next step" points four times in one session (see `DIAGNOSIS-tdd-implement-stuck.md`). The three rules below are the preventive measures that came out of that diagnosis. They are repo rules, not advice.

## Rule 1 — Turn Continuity

Every **Long-Horizon Skill** must carry a positive **Turn Continuity** rule of its own: the consecutive actions of a stage (red → green → typecheck → next seam) are executed serially **within one turn**, until the stage's exit condition is met. Do not end the turn at "announce the next step" points, and do not wait for the user to say "continue".

- State it **positively** (per the negation principle in `writing-great-skills`): describe the target behaviour, never the banned one.
- It must be **self-contained** — the skill cannot rely on the harness `/goal` line, because no `/goal` exists when the user does not activate one.
- Every stage ends on a checkable exit condition; reaching it is the only thing that ends the turn.
- A sub-step going green (e.g. one seam) is not a stage exit — a stage ends only when all of its seams are complete. Progress output does not itself end the turn: output, then keep executing until one of the three endpoints (compliance checkpoint, external blocker, stage exit) is reached.
- Canonical example: the 回合连续性 rule in [`.agents/skills/tdd-implement/references/stages.md`](../../skills/tdd-implement/references/stages.md) stage ③.

## Rule 2 — Model Selection

Flash-class models are markedly more likely to stop prematurely on long-horizon agentic work. For critical long tasks, prefer a stronger model or `/goal` mode. This is a runtime choice, not something a skill text can enforce — record it here so skill authors and session runners share the same guidance.

## Rule 3 — Progress Chunking

Giant turns — a single `write` of a large file, or a batch `replace` of a hundred-plus lines — hit output caps and get truncated mid-work. Chunk the work into small, individually verifiable steps:

- A single `write` over ~150 lines: write the skeleton first, then fill in batches.
- A batch of more than ~5 `replace`s: split into batches and verify after each batch.

These thresholds are experience defaults; adjust them as practice shows better values.

## Long-horizon skills inventory

Skills currently classified as Long-Horizon, to be evolved against these rules as they are touched: `tdd-implement` (fixed), `diagnose-fix` (fixed — new orchestration skill for diagnosis + TDD fix, carries its own Turn Continuity rule), `diagnosing-bugs`, `improve-codebase-architecture`, `wayfinder`, `grill-to-spec`, `to-spec`. Backfilling existing skill texts is out of scope for now — these rules bind new and edited skills going forward.
