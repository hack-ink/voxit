# Voxit Runtime Specification (macOS, English)

Purpose: Define the normative runtime, auth, capture, paste, configuration, and release
contract for Voxit in this repository.

Status: normative

Read this when: You need the authoritative contract for Voxit runtime behavior, state
transitions, authentication, audio capture, paste flow, configuration keys, or release
scope.

Not this document: Step-by-step operational guidance, design rationale, or workflow
instructions.

Defines:

- macOS-first runtime scope and platform boundaries
- user-visible state machine and transcript lifecycle
- authentication, storage, audio capture, finalize, rewrite, and paste contracts
- onboarding, configuration, CI, release, observability, and known-gap expectations

## 1) Runtime Scope and Boundaries

- Build entrypoint is the SwiftPM native macOS host under `native/macos-host/`.
- Voxit uses Rust Core plus a platform host: shared runtime and platform-neutral model
  contracts stay in Rust crates, while Swift owns the macOS UI.
- The staged app bundle is a menu bar utility (`LSUIElement = true`) with a SwiftUI
  `MenuBarExtra` and an on-demand Voxit window.
- The app supports English-first behavior and configuration defaults (`language = "en"`).
- No speech is injected into target apps while Pass1 is running; text is only pasted
  after Pass2 or Pass3 completion.

## 2) State Machine

The runtime state is user-visible in `self.state` and UI status labels:

- `Ready to listen.`
- `Listening`
- `Stopped`
- `FinalizingPass2`
- `RewritingPass3`
- `Done`

State transitions:

- `Start Dictation` or hotkey start in **toggle** mode -> `Listening`.
- `Stop Dictation` or hotkey release in **hold** mode -> stop capture, encode WAV, then
  `FinalizingPass2`.
- Pass2 completion:
  - if auto rewrite is enabled -> `RewritingPass3`
  - else -> paste raw final transcript and `Done`
- Pass3 completion:
  - if guard passes -> paste rewritten result and `Done`
  - if skipped or rejected -> paste raw result and `Done`
- `Paste raw now (skip rewrite)` during Pass2 or Pass3 forces raw paste and sets `Done`.

## 3) Authentication Contract

- Default login is ChatGPT OAuth via device-code authorization.
- Browser callback OAuth is not part of the active V1 login surface.
- Token acquisition flow:
  - show the device code and verification URL
  - poll until ChatGPT authorization completes or fails
  - exchange the authorized device session and persist auth locally
  - fallback path uses `OPENAI_API_KEY` only when no OAuth token exists
- Storage:
  - preferred: keyring
  - fallback: local `auth.json`
- On startup:
  - read status as "signed in" when unexpired token or session metadata exists
  - otherwise show "Not signed in."

## 4) Audio Capture and Streaming Contract

### 4.1 Capture

- Default recorder is macOS CoreAudio VoiceProcessingIO.
- The active recorder input is resolved at session start from `audio.input_device_id`.
  - `0` means system default.
  - non-zero uses the requested CoreAudio input device id from config.
  - if the requested device is missing or unusable, Voxit falls back to system default
    before capture starts.
- Capture should be continuous while in `Listening`, producing in-memory PCM sample
  buffers and metadata (`sample_rate`, `channels`, `frames`).
- Raw audio must not be persisted by default.

### 4.2 Device picker lifecycle

- On startup, the app refreshes available input-capable devices and caches the result.
- A manual **Refresh microphones** action is available in the UI to repopulate the
  picker.
- Picker values map to:
  - **System default** (`audio.input_device_id = 0`)
  - an explicit input device id and name pair from a discovered device list
- Selection changes persist `audio.input_device_name` and `audio.input_device_id` to
  config.
- If a configured device id is invalid or stale when starting recording, the runtime
  falls back to system default and reports fallback in status or logs.

### 4.3 Pass1 transport

- For each chunk, send `input_audio_buffer.append` payload frames to OpenAI Realtime.
- Realtime session must be configured with:
  - `audio.input.format`: `audio/pcm` with sample rate from config (default `24000`)
  - `audio.input.noise_reduction`: configured profile (default `near_field`)
  - `audio.input.transcription.model`: Pass1 model
  - `audio.input.turn_detection.type`: `server_vad`
- Realtime events consumed by the UI:
  - `conversation.item.input_audio_transcription.delta` (draft)
  - `conversation.item.input_audio_transcription.completed` (committed)

### 4.4 Transcript composition

- Draft and committed must be separated in UI:
  - committed = finalized turns from completed events
  - draft = latest in-flight text fragment
- Ordering for committed text is deterministic by `item_id` and `previous_item_id`
  chain; out-of-order completed events must still render in chain order.

## 5) Pass2 Finalization Contract

- On stop, stop capture and upload full WAV to `/v1/audio/transcriptions`.
- Use the configured finalize model.
- Final transcript (`Pass2`) becomes baseline output for:
  - paste when rewrite is disabled or skipped
  - rewrite input when enabled
  - final output display

## 6) Pass3 Rewrite Contract

- Auto-run rewrite only when:
  - raw Pass2 transcript exists
  - rewrite is enabled in runtime preference
  - rewrite auto flag is enabled
- If disabled for this run, skip and paste raw final transcript.
- Rewriter output contract:
  - keep meaning
  - preserve numeric, date, and currency tokens
  - reject rewrite when the protected token multiset changes
- Guarded outcomes:
  - `Applied`: paste rewritten text
  - `Rejected`: fallback to raw Pass2 and paste raw
  - `Skipped`: fallback to raw Pass2 and paste raw

## 7) Target App Capture and Paste

- Before starting recording, capture frontmost app metadata (pid, bundle id, name) if
  `lock_frontmost_app = true`.
- On paste:
  - attempt to reactivate captured target app with retries
  - copy to clipboard
  - dispatch `Cmd+V` (`Meta+V`) to simulate paste
- A dedicated test-paste action should validate the clipboard and paste injection path.

## 8) Hotkey and Tray Behavior

- Hotkey chord handling:
  - supported mode switch: toggle or hold
  - currently recognized physical combo: `Ctrl+Shift+Space`
  - configuration exposes `hotkey.chord` for future use
- Menu bar behavior:
  - `MenuBarExtra` exposes `Open Voxit` (`Cmd+O`), `Settings...` (`Cmd+,`),
    `Refresh Status` (`Cmd+R`), and `Quit Voxit` (`Cmd+Q`).
  - `Start Dictation` displays the configured dictation shortcut presentation, but
    remains disabled until the Swift menu action is wired to the Rust runtime command.
  - `Settings...` opens a dedicated AppKit-hosted Settings window.
  - The Settings window handles `Cmd+W` to close and `Cmd+Q` to terminate.

## 9) UI and Onboarding Contract

- UI contains:
  - auth status and sign-in actions
  - runtime controls (start/stop, rewrite toggle, hotkey mode)
  - live stream sections (committed plus draft)
  - final transcript sections
  - onboarding checklist statuses for microphone, accessibility, and input monitoring
- Onboarding checklist provides request actions for required macOS permissions. The UI
  prompts permission requests in order:
  - Microphone: probe-based request and retry loop when denied
  - Accessibility: system prompt request plus re-check
  - Input Monitoring: system prompt request plus re-check
- Grant each permission in macOS Privacy & Security settings when prompted, then
  re-check in Voxit before continuing.
- "Paste raw now" is always available when finalization or rewrite is active and should
  bypass Pass3.
- The Swift native host must render platform-neutral Rust model snapshots from
  `packages/voxit-core/` through `packages/voxit-host-ffi/` instead of defining a
  separate UI state machine.

## 10) Configuration Contract

Config file location:

- `${Application Support}/voxit/config.toml` via `ProjectDirs`

Supported sections and keys:

- `ui.start_hidden`, `ui.panel_width_px`, `ui.panel_height_px`
- `hotkey.chord`, `hotkey.mode` (`toggle` or `hold`)
- `audio.backend`, `audio.input_sample_rate_hz`, `audio.input_device_name`,
  `audio.input_device_id`, `audio.realtime_target_rate_hz`
- `openai.api_base_url`, `openai.realtime_model`, `openai.finalize_model`,
  `openai.rewrite_model`, `openai.language`
- `openai.realtime.noise_reduction`
- `rewrite.enabled`, `rewrite.auto`, `rewrite.guard_numbers`,
  `rewrite.max_output_chars`, `rewrite.style`
- `paste.lock_frontmost_app`, `paste.method`

On load:

- parse file when present
- defaults are used when missing or invalid entries are encountered
- `audio.input_device_id = 0` is treated as system default
- non-zero `audio.input_device_id` requests that device; if unavailable at startup,
  Voxit falls back to default input

Current Swift Settings window:

- persists shell preferences in macOS `UserDefaults`
- does not yet write those preferences through the Rust `config.toml` path

## 11) CI and Release

- `language.yml` is macOS-only for lint, format, and test checks.
- Release packaging matrix is restricted to `aarch64-apple-darwin` and comments out
  Linux and Windows jobs.
- Packaging uses `scripts/build_and_run.sh stage` to build `packages/voxit-host-ffi`,
  build the SwiftPM host, stage `target/voxit-native-host/Voxit.app`, and zip it as
  `voxit-<target>.zip`.

## 12) Observability and Logs

- Runtime logs are written via rotating file appender under the data directory.
- User-facing state is mirrored by status strings for troubleshooting.
- Error states must avoid hard-crash behavior and should return to a user-actionable
  status.

## 13) Known Gaps

- Swift Settings write-through to `config.toml` is not implemented yet.
- Menu-driven start/stop dictation is visible but not wired to the Rust runtime command
  yet.
- Configured hotkey chord string is not yet mapped; current hardcoded gesture is
  `Ctrl+Shift+Space` only.
- CPAL fallback capture is not implemented despite a configuration option; only the
  VoiceProcessingIO path is active.
- `rewrite.max_output_chars` and `rewrite.style` are persisted but not strictly
  enforced in the rewrite prompt yet.
- No explicit audio resampling step to 24 kHz is implemented in the current path.
