# Contextual Voice Specification

Purpose: Define the product contract for context-aware voice input, prompt profile
routing, interaction tiers, and host/core ownership.

Status: normative

Read this when: You need the authoritative contract for app-specific voice behavior,
prompt profile selection, reasoning effort, output policy, or the Rust versus Swift
boundary for contextual dictation.

Not this document: Step-by-step onboarding, visual design rationale, or provider API
integration details.

Defines:

- contextual voice input as the product layer above raw transcription
- the three user-facing interaction tiers
- `FocusedAppContext -> VoiceSessionPlan` routing ownership
- prompt profile, reasoning effort, and output policy contracts
- Swift host responsibilities versus Rust Core responsibilities

## 1) Product Contract

Voxit is a contextual voice input layer. The app should not treat speech-to-text as
the final product boundary. A session must produce output that is shaped by the app
where the user started dictation, the configured prompt profile, and the selected
output policy.

The durable product pipeline is:

```text
audio input
-> live transcript or semantic speech turns
-> focused app context snapshot
-> prompt profile selection
-> voice session plan
-> final text or action proposal
-> guarded insert, paste, or confirmation
```

The pipeline may use a transcription-only model, a text rewrite model, a realtime
reasoning voice model, or a combination. Provider selection is an implementation
detail; the contract is the context-aware session plan.

## 2) Interaction Tiers

### Fast Dictation

Fast Dictation is the lowest-friction path. It is used when the user needs clean text
quickly and there is no strong app-specific transformation requirement.

Required behavior:

- minimize latency
- default to direct insertion or paste
- avoid broad expansion
- preserve spoken content unless a configured cleanup rule applies

### Context Rewrite

Context Rewrite is the primary differentiator from basic ASR. It uses the focused app
context and a prompt profile to turn speech into the form expected by the destination.

Required behavior:

- select or confirm an app-specific prompt profile before final output
- shape tone, structure, vocabulary, and formatting for the destination app
- preserve high-precision entities unless the user explicitly asks to change them
- expose enough state for the HUD to show which profile is active

Examples:

- Linear or GitHub: issue, comment, review, or acceptance-criteria style
- Slack or Discord: concise conversational text
- Mail: complete but restrained email prose
- Code editors: code-editing instructions that preserve identifiers and file names
- Terminal: command proposals, with confirmation before execution-oriented output

### Voice Intent

Voice Intent is used when the user asks Voxit to produce an action proposal, structured
artifact, or multi-step transformation rather than plain text.

Required behavior:

- prefer preview or confirmation before externally visible or destructive outcomes
- separate the proposed output from any future execution step
- use stronger reasoning only when the workflow needs it
- make the selected output policy explicit in the session plan

## 3) Focused App Context

`FocusedAppContext` is the host-collected input to Rust-owned routing. It may contain:

- bundle id
- app name
- window title
- URL domain
- focused element role
- selected-text presence

Hosts should collect only the least sensitive data needed for routing. Full selected
text or document contents are not part of the default context contract.

## 4) Prompt Profiles

A `PromptProfile` defines how speech should become output for a destination. Profiles
belong to Rust Core so platform hosts share the same behavior.

Profile contracts include:

- stable profile id
- display title
- prompt directive used for model-specific prompt construction
- interaction tier
- default reasoning effort
- default output policy

The native host may set a manual built-in profile override for the current session.
When no override is active, built-in profile routing remains deterministic and
testable from focused app context.

User glossary terms are not routing rules. They are rewrite prompt inputs that help
preserve preferred spellings, names, and domain terms after a session plan has already
selected the profile.

## 5) Reasoning Effort

Reasoning effort is a session-planning property, not a UI preference alone.

Default policy:

- `minimal` for Fast Dictation
- `low` for common Context Rewrite
- `medium` for Voice Intent or high-precision routing
- `high` only when deeper reasoning materially improves the result

Latency-sensitive paths should use the lowest reasoning effort that satisfies the
workflow.

## 6) Output Policy

Output policy defines what the app may do with the final output:

- `insert_text`: insert or paste final text directly
- `preview_before_insert`: show the output before insertion
- `confirm_before_action`: require confirmation before action-like or risky output

Terminal and future automation surfaces must not skip confirmation for action-like
output.

## 7) Ownership Boundary

Rust Core owns:

- focused-context data contracts
- prompt profile definitions
- deterministic routing from context to profile
- voice session planning
- reasoning effort and output policy selection
- provider orchestration and model-specific prompt construction
- applying manual built-in profile overrides exposed by host UI
- applying glossary terms to rewrite prompt construction

Swift hosts own:

- menu bar, HUD, main window, and Settings presentation
- macOS-specific context capture
- permission panes and native controls
- rendering Rust-owned snapshots and session plans
- user confirmation UX

Swift must not become the durable source of contextual voice rules. If a rule affects
which profile, prompt, reasoning effort, or output policy applies, the rule belongs in
Rust Core.
