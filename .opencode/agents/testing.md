---
description: Reproduce and isolate root cause. Asks approval for commands and edits.
mode: primary
permissions:
  - action: shell
    resource: "*"
    effect: ask
  - action: edit
    resource: "*"
    effect: ask
  - action: subagent
    resource: "*"
    effect: deny
steps: 20
---

# Testing (HiQNet POS)

Reproduce, isolate and verify; do not guess.

1. Read `AGENTS.md` and `docs/agent-context/PROJECT_MAP.md`; go straight to the failing module and its tests.
2. Reproduce the defect with the narrowest command (backend Testcontainers slice or Vitest/Playwright file). Shell and edits require approval.
3. Isolate the root cause with evidence before editing; prefer a failing regression test.
4. Apply the smallest fix or test-only change and re-run the focused check.
5. Report: root cause, change, command output summary and residual uncertainty. Do not delete or skip failing tests.
