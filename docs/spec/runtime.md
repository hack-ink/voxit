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
- Context-aware voice behavior is governed by
  [`contextual-voice.md`](./contextual-voice.md). The runtime must treat raw
  transcription as one step in a broader contextual voice input pipeline.
- The staged app bundle is a menu bar utility (`LSUIElement = true`) with a SwiftUI
  `MenuBarExtra` and an on-demand Voxit window.
- The app supports English-first behavior and configuration defaults (`language = "en"`).
- No speech is injected into target apps while Pass1 is running; text is only pasted
  after Pass2 or Pass3 completion.

## 2) State Machine

The runtime state is user-visible through the Rust-owned native-host snapshot rendered
by Swift:

- `Ready to listen.`
- `Listening`
- `Stopped`
- `FinalizingPass2`
- `RewritingPass3`
- `Done`

State transitions:

- `Start Dictation` or menu shortcut start in **toggle** mode -> capture focused
  context, start recording, and enter `Listening`.
- `Stop Dictation` or hotkey release in **hold** mode -> stop capture, encode WAV, then
  `FinalizingPass2`.
- Pass2 completion:
  - if auto rewrite is enabled -> `RewritingPass3`
  - else -> set final output to raw transcript and `Done`
- Pass3 completion:
  - if guard passes -> set final output to rewritten result and `Done`
  - if skipped or rejected -> set final output to raw transcript and `Done`
- If the active output policy is `insert_text`, the runtime pastes final output into the
  captured target automatically. Preview and confirmation policies leave output in the
  HUD for explicit paste.

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
  - enforce `rewrite.max_output_chars`
  - apply `rewrite.style` and any user glossary terms to prompt construction
- Guarded outcomes:
  - `Applied`: paste rewritten text
  - `Rejected`: fallback to raw Pass2 and paste raw
  - `Skipped`: fallback to raw Pass2 and paste raw

## 7) Target App Capture and Paste

- Before starting recording, capture frontmost app metadata (pid, bundle id, name) if
  `lock_frontmost_app = true`.
- Focus context capture also records available window title, URL domain, focused element
  role, and selected-text presence for Rust-owned prompt routing.
- On paste:
  - attempt to reactivate captured target app with retries
  - copy to clipboard
  - dispatch `Cmd+V` (`Meta+V`) to simulate paste
- A dedicated test-paste action should validate the clipboard and paste injection path.

## 8) Hotkey and Tray Behavior

- Hotkey chord handling:
  - supported mode switch: toggle or hold
  - the menu command uses the configured `hotkey.chord` presentation
  - system-wide hotkey capture is not active yet
- Menu bar behavior:
  - `MenuBarExtra` exposes `Open Voxit` (`Cmd+O`), `Settings...` (`Cmd+,`),
    `Start Dictation`, `Stop Dictation`, `Refresh Status` (`Cmd+R`), and `Quit Voxit`
    (`Cmd+Q`).
  - `Start Dictation` and `Stop Dictation` call the Rust host FFI command surface.
  - `Settings...` opens a dedicated AppKit-hosted Settings window.
  - The Settings window handles `Cmd+W` to close and `Cmd+Q` to terminate.

## 9) UI and Onboarding Contract

- UI surfaces are split by responsibility:
  - menu bar: always-available status and control
  - recording HUD: live session state, transcript preview, active profile, and paste
    controls
  - Voxit control-center window: activity, app rules, profiles, glossary, prompt lab,
    and debug/evaluation surfaces
  - Settings window: app preferences, shortcuts, microphone, permissions, account
    defaults, privacy, logging, and notifications
- Onboarding checklist provides request actions for required macOS permissions. The UI
  prompts permission requests in order:
  - Microphone: probe-based request and retry loop when denied
  - Accessibility: system prompt request plus re-check
  - Input Monitoring: system prompt request plus re-check
- Grant each permission in macOS Privacy & Security settings when prompted, then
  re-check in Voxit before continuing.
- "Paste raw now" is always available when finalization or rewrite is active and should
  bypass Pass3.
- The Control Center exposes the current focused context, selected profile, profile
  override, glossary terms, and prompt lab sample state. Profile override and glossary
  terms are passed back through Rust FFI before model calls.
- The Swift native host must render platform-neutral Rust model snapshots from
  `packages/voxit-core/` through `packages/voxit-host-ffi/` instead of defining a
  separate UI state machine, contextual routing policy, or prompt profile registry.

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
- writes supported preferences through the Rust host FFI into `config.toml`

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

- System-wide global hotkey capture is not implemented yet; the configured shortcut is
  currently a Swift menu command.
- The native HUD does not yet render Pass1 realtime draft/committed transcript events;
  it shows active profile/state plus raw and final output after Pass2/Pass3.
- App-rule authoring is not implemented yet; users can refresh focus context and
  manually override the active built-in profile.
- The Swift Settings audio picker still exposes only System Default even though Rust can
  resolve configured CoreAudio input device ids.
- CPAL fallback capture is not implemented despite a configuration option; only the
  VoiceProcessingIO path is active.
