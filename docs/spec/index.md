# Spec Index

Purpose: Route agents to normative documents that define repository truth.

Question this index answers: "what must remain true?"

## Use this index when

- You need an invariant, contract, schema, enum, state model, interface, or required
  behavior.
- You are deciding whether code or data is correct.
- A runbook says "see the governing spec" and you need the authoritative source.

## Do not use this index when

- You need step-by-step instructions, maintenance actions, migrations, or incident
  response.
- You need durable rationale rather than the final contract; read
  `docs/decisions/index.md`.
- You want rationale only, without an authoritative contract.
- You need current layout or implementation boundaries; read `docs/reference/index.md`.

## What belongs in `docs/spec/`

- Contracts and invariants.
- Data shapes, canonical field names, enums, defaults, units, and limits.
- State transitions and protocol rules.
- Behavior that tests, code, or operators should treat as authoritative.

## Spec document contract

Start each spec with a compact routing header:

- `Purpose`
- `Status: normative`
- `Read this when`
- `Not this document`
- `Defines`

Then keep the body explicit:

- Prefer concrete nouns over pronouns.
- Separate facts from rationale.
- Include canonical names exactly as code or data uses them.
- Include a small example when it removes ambiguity.
- Link to related runbooks instead of embedding procedures.

## Structure policy

- Prefer shallow paths while the spec set is small.
- Add subfolders only when they mirror stable system boundaries or materially reduce
  ambiguity.
- Choose names for topic clarity and retrieval quality, not visual uniformity.
- If a runbook depends on a spec, the runbook links back to the governing spec.

## Current governing specs

- [`runtime.md`](./runtime.md) defines the runtime scope, auth flow, audio capture,
  transcript pipeline, paste behavior, configuration keys, and known gaps.
- [`contextual-voice.md`](./contextual-voice.md) defines context-aware voice input,
  prompt profile routing, interaction tiers, reasoning effort, output policy, and the
  host/core ownership boundary.
