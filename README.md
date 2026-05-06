<div align="center">

# Voxit

AI dictation App for macOS (MVP scaffold).

[![License](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Docs](https://img.shields.io/docsrs/voxit)](https://docs.rs/voxit)
[![Language Checks](https://github.com/hack-ink/voxit/actions/workflows/language.yml/badge.svg?branch=main)](https://github.com/hack-ink/voxit/actions/workflows/language.yml)
[![Release](https://github.com/hack-ink/voxit/actions/workflows/release.yml/badge.svg)](https://github.com/hack-ink/voxit/actions/workflows/release.yml)
[![GitHub tag (latest by date)](https://img.shields.io/github/v/tag/hack-ink/voxit)](https://github.com/hack-ink/voxit/tags)
[![GitHub last commit](https://img.shields.io/github/last-commit/hack-ink/voxit?color=red&style=plastic)](https://github.com/hack-ink/voxit)
[![GitHub code lines](https://tokei.rs/b1/github/hack-ink/voxit)](https://github.com/hack-ink/voxit)

</div>

## Feature Highlights

### What is implemented in v1

- Swift menu bar dictation app on macOS with start/stop hotkey control.
- ChatGPT login flow through OAuth device-code authorization.
- Real-time pass-1 transcription from mic with committed/draft streaming assembly.
- Pass-2 finalize pass using `gpt-4o-transcribe` for better punctuation and stability.
- Optional Pass-3 rewrite for cleaner English output with numeric/proper noun protection.
- Auto-paste into the app that was frontmost when recording began.
- Configurable behavior and models via `config.toml`.

For the normative product contract, constraints, and gaps, see the
[Runtime Spec](docs/spec/runtime.md).

## Status

V1 target is **macOS-first** and aligned to the English-only voice input design.

- Status: ✅ Core MVP loop is implemented (record → stream preview → finalize → optional rewrite → paste).
- Scope: ✅ Native macOS mic capture + OpenAI model pipeline only.
- Limitation: ✅ Linux/Windows build is intentionally disabled.
- Limitation: ⚠️ Known gaps are documented in the
  [Runtime Spec](docs/spec/runtime.md) (runtime action wiring, config write-through,
  CPAL fallback robustness, and rollout cleanup items).

## Usage

### Installation

#### Build from Source

```sh
# Clone the repository.
git clone https://github.com/hack-ink/voxit
cd voxit

# To install Rust on macOS and Unix, run the following command.
#
# To install Rust on Windows, download and run the installer from `https://rustup.rs`.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain stable

# Install the necessary dependencies. (Unix only)
# Using Ubuntu as an example, this really depends on your distribution.
sudo apt-get update
sudo apt-get install <DEPENDENCIES>

# Build the Swift native host and stage `Voxit.app`.
./scripts/build_and_run.sh stage

# Or build, stage, and launch the local app bundle.
./scripts/build_and_run.sh run
```

#### Download Pre-built Binary

- **macOS**
    - Download the latest pre-built binary from [GitHub Releases](https://github.com/hack-ink/voxit/releases/latest).
- **Windows / Linux**
    - Not included in V1 release target (macOS-only).

### Configuration

Voxit stores settings in:

```
$HOME/Library/Application Support/voxit/config.toml
```

Current supported keys are:

```toml
[ui]
start_hidden = true
panel_width_px = 420
panel_height_px = 260

[hotkey]
chord = "ctrl+shift+space"
mode = "toggle" # toggle | hold

[audio]
backend = "voice_processing" # voice_processing | cpal
input_sample_rate_hz = 48000
input_device_name = ""
input_device_id = 0
realtime_target_rate_hz = 24000

[openai]
api_base_url = "https://api.openai.com/v1"
realtime_model = "gpt-4o-mini-transcribe"
finalize_model = "gpt-4o-transcribe"
rewrite_model = "gpt-5.2-mini"
language = "en"

[openai.realtime]
noise_reduction = "near_field" # near_field | far_field | off

[rewrite]
enabled = true
auto = true
guard_numbers = true
max_output_chars = 8000
style = "clean" # clean | formal | concise

[paste]
lock_frontmost_app = true
method = "clipboard_cmd_v"
```

First-run onboarding checklist:

- Sign in with ChatGPT.
- Microphone permission in **System Settings → Privacy & Security → Microphone**.
- Accessibility permission in **Privacy & Security → Accessibility** (for Cmd+V fallback).
- Input Monitoring permission in **Privacy & Security → Input Monitoring** (for global hotkey hooks).
- Voxit uses request buttons to guide you through the permission prompts in sequence (Microphone → Accessibility → Input Monitoring); grant each permission and re-check when prompted.
- Verify paste flow after permission grant and restart the app if needed.

For the full guided sequence, see [First Run](docs/runbook/first-run.md).

Runtime configuration remains sourced from `config.toml`. The current Swift Settings
window persists shell preferences in macOS `UserDefaults`; writing those settings back
through the Rust config path is a tracked runtime gap.

### Interaction

### Runtime behavior

- Start recording: press the configured hotkey (default `Ctrl+Shift+Space`) to toggle.
- While listening: panel shows live draft text and committed segments.
- Stop recording: toggle key again or release key in hold mode.
- Finalize: Pass-2 runs automatically; rewrite runs by default unless disabled in settings.
- Microphone input selection is persisted in config as `audio.input_device_id` and `audio.input_device_name`.
- Refresh workflow: the picker list is refreshed at startup and via the **Refresh microphones** control before choosing from a list of input-capable devices.
- Runtime fallback: if a saved explicit device id is unavailable, Voxit falls back to the system default input device and continues recording.
- Paste behavior: by default paste rewritten text after finalize, or paste raw transcript via available controls.
- Output target: text is pasted into the app that was frontmost when dictation started.

## Update

### Changelog

- Track versioned behavior changes in [GitHub Releases](https://github.com/hack-ink/voxit/releases).

## Development

### Architecture

### Implementation snapshot

- Current app: Swift/SwiftUI menu bar host under `native/macos-host/`.
- Rust Core remains the runtime owner, exposed to Swift through C ABI glue under
  `packages/voxit-host-ffi/`.
- Dedicated auth/session/config/rewrite/paste pipeline and typed application state.
- macOS frontmost-app capture + clipboard/command-paste integration.

### Docs

- [Documentation Index](docs/index.md) routes to spec, runbook, reference, and decision docs.
- [Runtime Spec](docs/spec/runtime.md) is the normative runtime contract.
- [First Run](docs/runbook/first-run.md) covers sign-in, permission grants, and paste validation.
- [Repository Layout](docs/reference/repository-layout.md) maps the current repo surfaces.

## Support Me

If you find this project helpful and would like to support its development, you can buy me a coffee!

Your support is greatly appreciated and motivates me to keep improving this project.

- **Fiat**
    - [Ko-fi](https://ko-fi.com/hack_ink)
    - [Afdian](https://afdian.com/a/hack_ink)
- **Crypto**
    - **Bitcoin**
        - `bc1pedlrf67ss52md29qqkzr2avma6ghyrt4jx9ecp9457qsl75x247sqcp43c`
    - **Ethereum**
        - `0x3e25247CfF03F99a7D83b28F207112234feE73a6`
    - **Polkadot**
        - `156HGo9setPcU2qhFMVWLkcmtCEGySLwNqa3DaEiYSWtte4Y`

Thank you for your support!

## Appreciation

We would like to extend our heartfelt gratitude to the following projects and contributors:

- The Rust community for their continuous support and development of the Rust ecosystem.

## Additional Acknowledgements

- Not yet populated.

<div align="right">

### License

<sup>Licensed under [GPL-3.0](LICENSE).</sup>

</div>
