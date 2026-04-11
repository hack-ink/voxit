# Documentation Index

Purpose: Route agents to the smallest correct repository surface for the current task.

Audience: All documentation in this repository is written for AI agents and LLM
workflows. The split below is by question type, not by human-versus-agent audience.

## Read order

- Read `README.md` first when you need the repository scope, platform target, or
  top-level runtime summary.
- Read `docs/policy.md` for document contracts, placement rules, and naming rules.
- Use `cargo make` whenever an equivalent repo task exists. Inspect `Makefile.toml`
  directly when task names or execution entrypoints matter.
- Then choose one primary lane:
  - `docs/spec/index.md` when the question is "what must be true?"
  - `docs/runbook/index.md` when the question is "which sequence should I execute?"
  - `docs/reference/index.md` when the question is "how is it currently organized or
    implemented?"
  - `docs/decisions/index.md` when the question is "why is it shaped this way?"

## Routing matrix

- Need contracts, invariants, schemas, enums, state machines, or required behavior ->
  `docs/spec/`
- Need runbooks, onboarding, validation steps, troubleshooting, or operational sequences
  -> `docs/runbook/`
- Need current repository layout, ownership boundaries, or implementation surface maps ->
  `docs/reference/`
- Need durable rationale, tradeoffs, or historical consequences -> `docs/decisions/`
- Need repo task names or automation entrypoints -> `Makefile.toml`
- Need documentation placement or authoring rules -> `docs/policy.md`

## Retrieval rules

- Optimize for agent routing and execution, not narrative flow.
- Keep one authoritative document per topic. Link instead of copying.
- Runtime and behavior authority lives in code plus `docs/spec/`. Runbook, reference,
  and decision docs explain usage, current state, and rationale, but do not override the
  governing spec.
- Start each document with a short routing header that says what the document is for,
  when to read it, and what it does not cover.
- Keep links explicit and stable.
