#!/usr/bin/env bash
# Deterministic secret scan for commit-check check ③.
# The grep patterns are fragile to freehand — run this instead of re-writing
# the scan every time. Two confidence tiers:
#   FAIL  — structured secrets (KEY=value assignments, private key blocks)
#   WARN  — bare keywords (may legitimately appear in docs that mention them)
# Checks the staged diff (fail) and, unless --staged-only is passed, reports
# matches in the unstaged diff (warn).
#
# Usage:
#   scripts/scan-sensitive.sh            # staged (fail) + unstaged (warn)
#   scripts/scan-sensitive.sh --staged-only
set -euo pipefail

fail_patterns='(api[_-]?key|secret|token|passwd|password)[[:space:]]*[=:][[:space:]]*[^[:space:]]{8,}|BEGIN (RSA|OPENSSH|EC|DSA) PRIVATE KEY'
warn_patterns='(api[_-]?key|secret|token|passwd|password|\.env)'

fail=0

if git diff --cached -U0 | grep -inE "$fail_patterns"; then
  echo "❌ Structured secrets found in STAGED diff — remove them before committing." >&2
  fail=1
else
  echo "✅ No structured secrets in staged diff."
  if git diff --cached -U0 | grep -inE "$warn_patterns"; then
    echo "⚠  Keyword matches in STAGED diff — eyeball whether they are real secrets." >&2
  fi
fi

if [[ "${1:-}" != "--staged-only" ]]; then
  if git diff -U0 | grep -inE "$fail_patterns"; then
    echo "⚠  Structured-secret matches in UNSTAGED diff — decide whether they belong in the commit." >&2
  fi
fi

exit "$fail"
