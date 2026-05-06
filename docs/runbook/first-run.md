# First Run

Goal: Bring a fresh macOS Voxit install to the point where sign-in, permissions, and
paste work end to end.

Read this when: You are launching Voxit for the first time, validating a fresh install,
or re-checking onboarding after macOS permission resets.

Preconditions:

- A macOS machine with Voxit built or installed.
- Network access for ChatGPT sign-in.
- Access to macOS System Settings.

Depends on:

- [`../spec/runtime.md`](../spec/runtime.md) for the normative auth, permissions, and
  paste contract.
- `Makefile.toml` when you need the repository task entrypoints for formatting, linting,
  or tests before packaging.

Verification:

- Voxit shows signed-in status.
- Microphone, Accessibility, and Input Monitoring permissions are granted.
- A short dictation run pastes text back into the app that was frontmost at start.

## 1. Launch Voxit

- Start the app from `Voxit.app` or a local debug build.
- If you are building from source, run `./scripts/build_and_run.sh run` from the
  repository root to build, stage, and launch the Swift native host.

## 2. Sign in

- Open the auth controls in the Voxit window.
- Use the default ChatGPT device-code OAuth flow.
- Complete authorization in the browser using the visible code, then return to the app.

## 3. Grant required macOS permissions

- Open the onboarding or preferences surface in Voxit.
- Grant permissions in this order:
  1. Microphone
  2. Accessibility
  3. Input Monitoring
- After each grant, re-check the status in Voxit before moving to the next permission.

## 4. Confirm runtime configuration

- Open **Settings...** from the menu bar menu or press `Cmd+,` to confirm shell
  preferences and permission shortcuts are available.
- Check the config file at:

```text
$HOME/Library/Application Support/voxit/config.toml
```

- Confirm the default runtime hotkey and audio device settings look reasonable for the
  machine.
- If you need an explicit microphone, refresh the device list and select it before the
  first real dictation run.

## 5. Verify paste flow

- Put focus on a target app that accepts text input.
- Start a short dictation run.
- Stop recording and wait for finalize and optional rewrite to finish.
- Confirm the result pastes back into the same app that was frontmost when recording
  started.

## 6. Failure handling

- If sign-in stalls, reopen the auth surface and retry the device-code flow.
- If a permission does not update, grant it in macOS System Settings and then re-check
  from Voxit.
- If paste fails, verify Accessibility and Input Monitoring first before debugging the
  clipboard or target-app path.
