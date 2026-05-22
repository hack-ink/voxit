# Contextual Voice Layer

Status: accepted

Date: 2026-05-08

Question: What product shape should Voxit use as it grows beyond basic speech-to-text?

Decision: Voxit is a menu bar-first contextual voice input layer for macOS. It should
feel like an input utility that works inside the user's current app, not like a
standalone voice chat app. The menu bar owns always-available control and status, a
recording HUD owns the active dictation moment, the main Voxit window owns user-facing
work assets such as profiles and prompt routing, and Settings owns app preferences.

Consequences:

- The primary product action happens in the focused app, with Voxit capturing context,
  transforming speech, and inserting output back into that app.
- Voxit differentiates from basic ASR by selecting a prompt profile from the current
  app context before final output is produced.
- The main Voxit window is a control center for activity, app rules, profiles,
  glossary, prompt experiments, and debug/evaluation surfaces.
- The Settings window stays separate and limited to app preferences such as startup,
  shortcuts, model choices, microphone, permissions, account defaults, privacy, logging,
  and notifications.
- Swift owns the native macOS presentation layer and UI glue. Rust owns durable product
  logic, context classification, prompt profile selection, voice session planning,
  output policy, and provider orchestration.
- Platform-specific hosts may add their own UI surfaces, but they must consume
  Rust-owned contracts instead of redefining contextual voice behavior in each host.
