//! Thin C ABI bridge for native platform hosts.
//!
//! The ABI intentionally starts with only a session handle and a copy-out UI snapshot.
//! This gives the Swift host a stable Rust-owned model without moving audio, auth, or
//! inference orchestration across FFI before those boundaries are ready.

#[cfg(target_os = "macos")] use std::sync::mpsc;
use std::{
	ffi::{CStr, c_char},
	ptr::{self, NonNull},
	sync::mpsc::{Receiver, TryRecvError},
};

#[cfg(target_os = "macos")] use voxit_audio::Recorder;
#[cfg(target_os = "macos")] use voxit_core::RealtimeSessionConfig;
#[cfg(target_os = "macos")] use voxit_core::RewriteSettings;
use voxit_core::{
	self, Config, ContextualVoiceRouter, FocusedAppContext, NativeHostSnapshot, PlatformHost,
	RealtimeEvent, RealtimeSession, TranscriptAssembler, VoiceSessionPlan,
	contextual::{
		PromptProfileKind, VoiceInteractionTier, VoiceOutputPolicy, VoiceReasoningEffort,
	},
	ui_model::{AuthMethod, AuthSurfaceState, DictationSurfaceState, HotkeySurfaceMode},
};
#[cfg(target_os = "macos")] use voxit_macos::TargetApp;

/// ABI version exported by the thin C host bridge.
pub const VOXIT_HOST_FFI_ABI_VERSION: u32 = 6;

/// Opaque session handle owned by the native host through the C ABI.
pub struct VoxitHostSessionHandle {
	config: Config,
	snapshot: NativeHostSnapshot,
	focused_context: FocusedAppContext,
	profile_override: Option<PromptProfileKind>,
	voice_plan: VoiceSessionPlan,
	glossary_terms: String,
	transcript_assembler: TranscriptAssembler,
	pass1_committed_transcript: String,
	pass1_draft_transcript: String,
	last_raw_transcript: String,
	last_final_output: String,
	last_error: String,
	recording_duration_ms: u64,
	realtime_session: Option<RealtimeSession>,
	realtime_event_rx: Option<Receiver<RealtimeEvent>>,
	#[cfg(target_os = "macos")]
	recorder: Option<Recorder>,
	#[cfg(target_os = "macos")]
	target_app: Option<TargetApp>,
}

/// Result code returned by FFI entry points.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxitStatus {
	/// The operation succeeded.
	Ok = 0,
	/// The provided session handle was null.
	NullHandle = 1,
	/// The provided output pointer was null.
	NullOutput = 2,
	/// The provided input payload was invalid.
	InvalidInput = 3,
}

/// FFI-safe platform tag.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxitPlatformTag {
	/// Native macOS host.
	MacOS = 0,
	/// Unsupported or test-only host.
	Unsupported = 1,
}

/// FFI-safe auth method.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxitAuthMethod {
	/// ChatGPT OAuth device-code authorization.
	ChatGptDeviceCode = 0,
}

/// FFI-safe auth state.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxitAuthState {
	/// The host has not completed an auth status read yet.
	Checking = 0,
	/// No usable ChatGPT session is present.
	SignedOut = 1,
	/// A usable ChatGPT session is present.
	SignedIn = 2,
	/// An auth flow is currently in progress.
	Busy = 3,
}

/// FFI-safe dictation state.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxitDictationState {
	/// No active recording is in progress.
	Idle = 0,
	/// Microphone capture and pass-1 streaming are active.
	Listening = 1,
	/// Pass-2 finalization is running.
	Finalizing = 2,
	/// Pass-3 rewrite is running.
	Rewriting = 3,
	/// The latest dictation cycle has completed.
	Done = 4,
}

/// FFI-safe hotkey mode.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxitHotkeyMode {
	/// Press once to start and press again to stop.
	Toggle = 0,
	/// Hold to record and release to stop.
	Hold = 1,
}

/// FFI-safe built-in prompt profile kind.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxitPromptProfileKind {
	/// Default low-latency dictation profile.
	FastDictation = 0,
	/// Messaging profile for conversational destinations.
	Messaging = 1,
	/// Mail profile for complete email prose.
	Mail = 2,
	/// Code editor profile for programming-related dictation.
	CodeEditor = 3,
	/// Terminal profile for command-like proposals.
	Terminal = 4,
	/// Work tracker profile for issue, review, and planning destinations.
	WorkTracker = 5,
}

/// FFI-safe interaction tier.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxitVoiceInteractionTier {
	/// Lowest-latency speech-to-clean-text path.
	FastDictation = 0,
	/// App-aware rewrite path.
	ContextRewrite = 1,
	/// Intent-oriented path.
	VoiceIntent = 2,
}

/// FFI-safe reasoning effort.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxitVoiceReasoningEffort {
	/// Fastest viable reasoning path.
	Minimal = 0,
	/// Light reasoning for common contextual rewrites.
	Low = 1,
	/// Deeper reasoning for multi-step output.
	Medium = 2,
	/// Strongest reasoning for constrained output.
	High = 3,
}

/// FFI-safe output policy.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxitVoiceOutputPolicy {
	/// Insert or paste final text directly.
	InsertText = 0,
	/// Show the output before insertion.
	PreviewBeforeInsert = 1,
	/// Require confirmation before action-like output.
	ConfirmBeforeAction = 2,
}

/// FFI-safe string fields exposed through copy-out buffers.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxitHostStringField {
	/// Focused app bundle id.
	FocusedBundleId = 0,
	/// Focused app display name.
	FocusedAppName = 1,
	/// Focused window title.
	FocusedWindowTitle = 2,
	/// Focused URL domain.
	FocusedUrlDomain = 3,
	/// Focused accessibility element role.
	FocusedElementRole = 4,
	/// Selected prompt profile id.
	PromptProfileId = 5,
	/// Selected prompt directive.
	PromptDirective = 6,
	/// Latest raw Pass2 transcript.
	RawTranscript = 7,
	/// Latest final output after rewrite or fallback.
	FinalOutput = 8,
	/// Latest user-actionable error.
	LastError = 9,
	/// Latest committed realtime Pass1 transcript.
	Pass1CommittedTranscript = 10,
	/// Latest in-flight realtime Pass1 draft transcript.
	Pass1DraftTranscript = 11,
}

/// FFI-safe session configuration.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VoxitHostConfig {
	/// Platform family that owns the host.
	pub platform: VoxitPlatformTag,
}

/// FFI-safe user preference payload written through Rust config.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VoxitHostPreferences {
	/// Non-zero when the app should start hidden/menu-bar first.
	pub start_hidden: u8,
	/// Hotkey mode.
	pub hotkey_mode: VoxitHotkeyMode,
	/// Non-zero when final output should paste automatically when policy allows it.
	pub paste_after_transcription: u8,
	/// Non-zero when pass-3 rewrite is enabled.
	pub rewrite_after_transcription: u8,
}

/// FFI-safe native-host snapshot.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VoxitHostSnapshot {
	/// Visible platform shell.
	pub platform: VoxitPlatformTag,
	/// Auth method for the first native UI.
	pub auth_method: VoxitAuthMethod,
	/// Current auth status.
	pub auth_state: VoxitAuthState,
	/// Current dictation status.
	pub dictation_state: VoxitDictationState,
	/// Current hotkey mode.
	pub hotkey_mode: VoxitHotkeyMode,
	/// Suggested panel width.
	pub panel_width_px: u32,
	/// Suggested panel height.
	pub panel_height_px: u32,
	/// Non-zero when pass-3 rewrite is enabled.
	pub rewrite_enabled: u8,
	/// Non-zero when focused app context has at least one routing signal.
	pub has_focused_context: u8,
	/// Non-zero when selected text was present at context capture time.
	pub selected_text_present: u8,
	/// Non-zero when a raw Pass2 transcript is available.
	pub has_raw_transcript: u8,
	/// Non-zero when realtime Pass1 committed transcript text is available.
	pub has_pass1_committed_transcript: u8,
	/// Non-zero when realtime Pass1 draft transcript text is available.
	pub has_pass1_draft_transcript: u8,
	/// Non-zero when a final output is available.
	pub has_final_output: u8,
	/// Non-zero when the last command failed or produced a warning.
	pub has_error: u8,
	/// Last recording duration reported by audio capture.
	pub recording_duration_ms: u64,
	/// Selected prompt profile kind.
	pub prompt_profile_kind: VoxitPromptProfileKind,
	/// Selected voice interaction tier.
	pub voice_tier: VoxitVoiceInteractionTier,
	/// Selected reasoning effort.
	pub reasoning_effort: VoxitVoiceReasoningEffort,
	/// Selected output policy.
	pub output_policy: VoxitVoiceOutputPolicy,
}
impl Default for VoxitHostSnapshot {
	fn default() -> Self {
		Self {
			platform: VoxitPlatformTag::Unsupported,
			auth_method: VoxitAuthMethod::ChatGptDeviceCode,
			auth_state: VoxitAuthState::Checking,
			dictation_state: VoxitDictationState::Idle,
			hotkey_mode: VoxitHotkeyMode::Toggle,
			panel_width_px: 0,
			panel_height_px: 0,
			rewrite_enabled: 0,
			has_focused_context: 0,
			selected_text_present: 0,
			has_raw_transcript: 0,
			has_pass1_committed_transcript: 0,
			has_pass1_draft_transcript: 0,
			has_final_output: 0,
			has_error: 0,
			recording_duration_ms: 0,
			prompt_profile_kind: VoxitPromptProfileKind::FastDictation,
			voice_tier: VoxitVoiceInteractionTier::FastDictation,
			reasoning_effort: VoxitVoiceReasoningEffort::Minimal,
			output_policy: VoxitVoiceOutputPolicy::InsertText,
		}
	}
}

/// Returns the ABI version exported by this bridge.
#[unsafe(no_mangle)]
pub extern "C" fn voxit_host_ffi_abi_version() -> u32 {
	VOXIT_HOST_FFI_ABI_VERSION
}

/// Creates a Rust-owned native-host session.
#[unsafe(no_mangle)]
pub extern "C" fn voxit_host_session_create(
	config: VoxitHostConfig,
) -> *mut VoxitHostSessionHandle {
	let platform = match config.platform {
		VoxitPlatformTag::MacOS => PlatformHost::MacOS,
		VoxitPlatformTag::Unsupported => PlatformHost::Unsupported,
	};
	let config = Config::load().unwrap_or_else(|_| Config::default());
	let snapshot = NativeHostSnapshot::initial(platform, &config);
	let focused_context = FocusedAppContext::new();
	let voice_plan = ContextualVoiceRouter.plan_for_context(&focused_context);

	Box::into_raw(Box::new(VoxitHostSessionHandle {
		config,
		snapshot,
		focused_context,
		profile_override: None,
		voice_plan,
		glossary_terms: String::new(),
		transcript_assembler: TranscriptAssembler::new(),
		pass1_committed_transcript: String::new(),
		pass1_draft_transcript: String::new(),
		last_raw_transcript: String::new(),
		last_final_output: String::new(),
		last_error: String::new(),
		recording_duration_ms: 0,
		realtime_session: None,
		realtime_event_rx: None,
		#[cfg(target_os = "macos")]
		recorder: None,
		#[cfg(target_os = "macos")]
		target_app: None,
	}))
}

/// Destroys a Rust-owned native-host session.
///
/// # Safety
///
/// `handle` must be either null or a pointer previously returned by
/// [`voxit_host_session_create`] that has not already been destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn voxit_host_session_destroy(handle: *mut VoxitHostSessionHandle) {
	if let Some(handle) = NonNull::new(handle) {
		let mut handle = unsafe { Box::from_raw(handle.as_ptr()) };

		stop_realtime_preview(&mut handle);
	}
}

/// Copies the current Rust-owned host snapshot into caller-owned memory.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by [`voxit_host_session_create`]. `out`
/// must point to writable memory for one [`VoxitHostSnapshot`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn voxit_host_session_copy_snapshot(
	handle: *mut VoxitHostSessionHandle,
	out: *mut VoxitHostSnapshot,
) -> VoxitStatus {
	let Some(mut handle) = NonNull::new(handle) else {
		return VoxitStatus::NullHandle;
	};
	let Some(out) = NonNull::new(out) else {
		return VoxitStatus::NullOutput;
	};
	let handle_ref = unsafe { handle.as_mut() };

	drain_realtime_events(handle_ref);

	let snapshot = &handle_ref.snapshot;
	let focused_context = &handle_ref.focused_context;
	let voice_plan = &handle_ref.voice_plan;

	unsafe {
		out.as_ptr().write(encode_snapshot_with_context(
			handle_ref,
			snapshot,
			focused_context,
			voice_plan,
		))
	};

	VoxitStatus::Ok
}

/// Refreshes focused app context and recomputes the Rust-owned voice session plan.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by [`voxit_host_session_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn voxit_host_session_refresh_focused_context(
	handle: *mut VoxitHostSessionHandle,
) -> VoxitStatus {
	let Some(mut handle) = NonNull::new(handle) else {
		return VoxitStatus::NullHandle;
	};
	let handle = unsafe { handle.as_mut() };

	refresh_focused_context(handle);
	update_voice_plan(handle);

	VoxitStatus::Ok
}

/// Starts a native dictation capture session.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by [`voxit_host_session_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn voxit_host_session_start_dictation(
	handle: *mut VoxitHostSessionHandle,
) -> VoxitStatus {
	let Some(mut handle) = NonNull::new(handle) else {
		return VoxitStatus::NullHandle;
	};
	let handle = unsafe { handle.as_mut() };

	start_dictation(handle)
}

/// Stops capture, finalizes transcription, optionally rewrites, and applies output policy.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by [`voxit_host_session_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn voxit_host_session_stop_dictation(
	handle: *mut VoxitHostSessionHandle,
) -> VoxitStatus {
	let Some(mut handle) = NonNull::new(handle) else {
		return VoxitStatus::NullHandle;
	};
	let handle = unsafe { handle.as_mut() };

	stop_dictation(handle)
}

/// Pastes the latest final output into the captured target app.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by [`voxit_host_session_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn voxit_host_session_paste_final_output(
	handle: *mut VoxitHostSessionHandle,
) -> VoxitStatus {
	let Some(mut handle) = NonNull::new(handle) else {
		return VoxitStatus::NullHandle;
	};
	let handle = unsafe { handle.as_mut() };

	paste_final_output(handle)
}

/// Saves host preferences through the Rust-owned config file.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by [`voxit_host_session_create`].
/// `hotkey_chord` must point to a null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn voxit_host_session_save_preferences(
	handle: *mut VoxitHostSessionHandle,
	preferences: VoxitHostPreferences,
	hotkey_chord: *const c_char,
) -> VoxitStatus {
	let Some(mut handle) = NonNull::new(handle) else {
		return VoxitStatus::NullHandle;
	};
	let Some(hotkey_chord) = NonNull::new(hotkey_chord.cast_mut()) else {
		return VoxitStatus::InvalidInput;
	};
	let handle = unsafe { handle.as_mut() };
	let hotkey_chord = unsafe { CStr::from_ptr(hotkey_chord.as_ptr()) };
	let Ok(hotkey_chord) = hotkey_chord.to_str() else {
		set_error(handle, "hotkey chord is not valid UTF-8");

		return VoxitStatus::Ok;
	};

	save_preferences(handle, preferences, hotkey_chord)
}

/// Saves OpenAI model preferences through the Rust-owned config file.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by [`voxit_host_session_create`]. Model
/// pointers must point to null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn voxit_host_session_save_model_preferences(
	handle: *mut VoxitHostSessionHandle,
	realtime_model: *const c_char,
	realtime_transcription_model: *const c_char,
	finalize_model: *const c_char,
	rewrite_model: *const c_char,
) -> VoxitStatus {
	let Some(mut handle) = NonNull::new(handle) else {
		return VoxitStatus::NullHandle;
	};
	let handle = unsafe { handle.as_mut() };
	let realtime_model = match read_required_c_string(handle, realtime_model, "realtime model") {
		Ok(value) => value,
		Err(status) => return status,
	};
	let realtime_transcription_model = match read_required_c_string(
		handle,
		realtime_transcription_model,
		"realtime transcription model",
	) {
		Ok(value) => value,
		Err(status) => return status,
	};
	let finalize_model = match read_required_c_string(handle, finalize_model, "finalize model") {
		Ok(value) => value,
		Err(status) => return status,
	};
	let rewrite_model = match read_required_c_string(handle, rewrite_model, "rewrite model") {
		Ok(value) => value,
		Err(status) => return status,
	};

	save_model_preferences(
		handle,
		realtime_model,
		realtime_transcription_model,
		finalize_model,
		rewrite_model,
	)
}

/// Sets a manual prompt-profile override for the current host session.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by [`voxit_host_session_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn voxit_host_session_set_profile_override(
	handle: *mut VoxitHostSessionHandle,
	profile_kind: VoxitPromptProfileKind,
) -> VoxitStatus {
	let Some(mut handle) = NonNull::new(handle) else {
		return VoxitStatus::NullHandle;
	};
	let handle = unsafe { handle.as_mut() };

	handle.profile_override = Some(decode_prompt_profile_kind(profile_kind));

	update_voice_plan(handle);

	VoxitStatus::Ok
}

/// Clears any manual prompt-profile override for the current host session.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by [`voxit_host_session_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn voxit_host_session_clear_profile_override(
	handle: *mut VoxitHostSessionHandle,
) -> VoxitStatus {
	let Some(mut handle) = NonNull::new(handle) else {
		return VoxitStatus::NullHandle;
	};
	let handle = unsafe { handle.as_mut() };

	handle.profile_override = None;

	update_voice_plan(handle);

	VoxitStatus::Ok
}

/// Sets newline-separated glossary terms for contextual rewrite prompts.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by [`voxit_host_session_create`].
/// `glossary_terms` must point to a null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn voxit_host_session_set_glossary(
	handle: *mut VoxitHostSessionHandle,
	glossary_terms: *const c_char,
) -> VoxitStatus {
	let Some(mut handle) = NonNull::new(handle) else {
		return VoxitStatus::NullHandle;
	};
	let Some(glossary_terms) = NonNull::new(glossary_terms.cast_mut()) else {
		return VoxitStatus::InvalidInput;
	};
	let handle = unsafe { handle.as_mut() };
	let glossary_terms = unsafe { CStr::from_ptr(glossary_terms.as_ptr()) };
	let Ok(glossary_terms) = glossary_terms.to_str() else {
		set_error(handle, "glossary terms are not valid UTF-8");

		return VoxitStatus::Ok;
	};

	handle.glossary_terms = glossary_terms.to_string();

	VoxitStatus::Ok
}

/// Copies a Rust-owned string field into caller-owned memory.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by [`voxit_host_session_create`].
/// `out` must point to writable memory for `out_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn voxit_host_session_copy_string(
	handle: *mut VoxitHostSessionHandle,
	field: VoxitHostStringField,
	out: *mut c_char,
	out_len: usize,
) -> VoxitStatus {
	let Some(mut handle) = NonNull::new(handle) else {
		return VoxitStatus::NullHandle;
	};
	let Some(out) = NonNull::new(out) else {
		return VoxitStatus::NullOutput;
	};

	if out_len == 0 {
		return VoxitStatus::InvalidInput;
	}

	let handle = unsafe { handle.as_mut() };

	drain_realtime_events(handle);

	let value = string_field_value(handle, field);

	write_c_string(out, out_len, value)
}

fn start_dictation(handle: &mut VoxitHostSessionHandle) -> VoxitStatus {
	clear_run_output(handle);
	refresh_focused_context(handle);
	update_voice_plan(handle);

	#[cfg(target_os = "macos")]
	{
		if handle.recorder.is_some() {
			set_error(handle, "recording is already active");

			return VoxitStatus::Ok;
		}

		let preferred_device_id = (handle.config.audio.input_device_id != 0)
			.then_some(handle.config.audio.input_device_id);

		match voxit_audio::start_recording_with_stream(
			64,
			preferred_device_id,
			handle.config.audio.realtime_target_rate_hz,
			1,
		) {
			Ok((recorder, chunk_rx, selection)) => {
				let (event_tx, event_rx) = mpsc::channel();

				match voxit_core::start_realtime_session(
					realtime_session_config(handle),
					chunk_rx,
					event_tx,
				) {
					Ok(session) => {
						handle.realtime_session = Some(session);
						handle.realtime_event_rx = Some(event_rx);
					},
					Err(err) => {
						handle.realtime_session = None;
						handle.realtime_event_rx = None;
						handle.last_error = format!("realtime preview unavailable: {err}");
					},
				}

				handle.recorder = Some(recorder);
				handle.snapshot.dictation_state = DictationSurfaceState::Listening;
				handle.recording_duration_ms = 0;

				if selection.fallback_to_default {
					handle.last_error = format!(
						"requested microphone unavailable; using {}",
						selection.selected_device_name
					);
				}
			},
			Err(err) => {
				handle.snapshot.dictation_state = DictationSurfaceState::Idle;

				set_error(handle, format!("failed to start recording: {err}"));
			},
		}

		VoxitStatus::Ok
	}
	#[cfg(not(target_os = "macos"))]
	{
		handle.snapshot.dictation_state = DictationSurfaceState::Idle;

		set_error(handle, "recording is only supported on macOS in this build");

		VoxitStatus::Ok
	}
}

fn stop_dictation(handle: &mut VoxitHostSessionHandle) -> VoxitStatus {
	#[cfg(target_os = "macos")]
	{
		let Some(recorder) = handle.recorder.take() else {
			set_error(handle, "recording is not active");

			return VoxitStatus::Ok;
		};

		handle.snapshot.dictation_state = DictationSurfaceState::Finalizing;

		let recording = match voxit_audio::stop_recording(recorder) {
			Ok(recording) => recording,
			Err(err) => {
				handle.snapshot.dictation_state = DictationSurfaceState::Done;

				stop_realtime_preview(handle);
				set_error(handle, format!("failed to stop recording: {err}"));

				return VoxitStatus::Ok;
			},
		};

		handle.recording_duration_ms = recording.duration_ms;

		stop_realtime_preview(handle);
		drain_realtime_events(handle);

		let (raw_transcript, _) =
			match voxit_core::transcribe_only(&recording.wav, &handle.config.openai.finalize_model)
			{
				Ok(result) => result,
				Err(err) => {
					let fallback = realtime_transcript_text(handle);

					if fallback.is_empty() {
						handle.snapshot.dictation_state = DictationSurfaceState::Done;

						set_error(handle, format!("transcription failed: {err}"));

						return VoxitStatus::Ok;
					}

					handle.last_error =
						format!("transcription failed; using realtime transcript: {err}");

					(fallback, 0)
				},
			};

		handle.last_raw_transcript = raw_transcript;
		handle.last_final_output = handle.last_raw_transcript.clone();

		if handle.config.rewrite.enabled && handle.config.rewrite.auto {
			handle.snapshot.dictation_state = DictationSurfaceState::Rewriting;

			let settings = rewrite_settings(handle);

			match voxit_core::rewrite_only_with_plan(
				&handle.last_raw_transcript,
				&handle.config.openai.rewrite_model,
				&handle.voice_plan,
				&settings,
			) {
				Ok((result, _)) => {
					if let Some(rewritten) = result.rewritten_transcript {
						handle.last_final_output = rewritten;
					}
					if let Some(reason) = result.reason {
						handle.last_error = reason;
					}
				},
				Err(err) => {
					handle.last_error = format!("rewrite failed: {err}");
				},
			}
		}

		handle.snapshot.dictation_state = DictationSurfaceState::Done;

		if matches!(handle.voice_plan.output_policy, VoiceOutputPolicy::InsertText) {
			let _ = paste_final_output(handle);
		}

		VoxitStatus::Ok
	}
	#[cfg(not(target_os = "macos"))]
	{
		handle.snapshot.dictation_state = DictationSurfaceState::Done;

		set_error(handle, "recording is only supported on macOS in this build");

		VoxitStatus::Ok
	}
}

fn paste_final_output(handle: &mut VoxitHostSessionHandle) -> VoxitStatus {
	#[cfg(target_os = "macos")]
	{
		if handle.last_final_output.is_empty() {
			set_error(handle, "no final output is available to paste");

			return VoxitStatus::Ok;
		}

		let target =
			if handle.config.paste.lock_frontmost_app { handle.target_app.as_ref() } else { None };

		if let Err(err) = voxit_macos::paste_text(
			target,
			&handle.last_final_output,
			handle.config.paste.lock_frontmost_app,
		) {
			set_error(handle, format!("paste failed: {err}"));
		}

		VoxitStatus::Ok
	}

	#[cfg(not(target_os = "macos"))]
	{
		set_error(handle, "paste is only supported on macOS in this build");

		VoxitStatus::Ok
	}
}

fn save_preferences(
	handle: &mut VoxitHostSessionHandle,
	preferences: VoxitHostPreferences,
	hotkey_chord: &str,
) -> VoxitStatus {
	handle.config.ui.start_hidden = preferences.start_hidden != 0;
	handle.config.hotkey.chord = hotkey_chord.to_ascii_lowercase().replace('-', "+");
	handle.config.hotkey.mode = match preferences.hotkey_mode {
		VoxitHotkeyMode::Hold => "hold".to_string(),
		VoxitHotkeyMode::Toggle => "toggle".to_string(),
	};
	handle.config.rewrite.enabled = preferences.rewrite_after_transcription != 0;
	handle.config.rewrite.auto = preferences.rewrite_after_transcription != 0;
	handle.config.paste.method = if preferences.paste_after_transcription != 0 {
		"clipboard_cmd_v".to_string()
	} else {
		"manual".to_string()
	};
	handle.snapshot.hotkey_mode = match preferences.hotkey_mode {
		VoxitHotkeyMode::Hold => HotkeySurfaceMode::Hold,
		VoxitHotkeyMode::Toggle => HotkeySurfaceMode::Toggle,
	};
	handle.snapshot.rewrite_enabled = handle.config.rewrite.enabled;

	if let Err(err) = handle.config.save() {
		set_error(handle, format!("failed to save config: {err}"));
	} else {
		handle.last_error.clear();
	}

	VoxitStatus::Ok
}

fn save_model_preferences(
	handle: &mut VoxitHostSessionHandle,
	realtime_model: String,
	realtime_transcription_model: String,
	finalize_model: String,
	rewrite_model: String,
) -> VoxitStatus {
	handle.config.openai.realtime_model = realtime_model;
	handle.config.openai.realtime.transcription_model = realtime_transcription_model;
	handle.config.openai.finalize_model = finalize_model;
	handle.config.openai.rewrite_model = rewrite_model;

	if let Err(err) = handle.config.save() {
		set_error(handle, format!("failed to save config: {err}"));
	} else {
		handle.last_error.clear();
	}

	VoxitStatus::Ok
}

fn clear_run_output(handle: &mut VoxitHostSessionHandle) {
	stop_realtime_preview(handle);

	handle.transcript_assembler.reset();
	handle.pass1_committed_transcript.clear();
	handle.pass1_draft_transcript.clear();
	handle.last_raw_transcript.clear();
	handle.last_final_output.clear();
	handle.last_error.clear();

	handle.recording_duration_ms = 0;
}

#[cfg(target_os = "macos")]
fn realtime_session_config(handle: &VoxitHostSessionHandle) -> RealtimeSessionConfig {
	RealtimeSessionConfig {
		model: handle.config.openai.realtime_model.clone(),
		transcription_model: handle.config.openai.realtime.transcription_model.clone(),
		language: handle.config.openai.language.clone(),
		sample_rate_hz: handle.config.audio.realtime_target_rate_hz,
		noise_reduction: handle.config.openai.realtime.noise_reduction.clone(),
		instructions: realtime_session_instructions(handle),
		reasoning_effort: reasoning_effort_value(handle.voice_plan.reasoning_effort).to_string(),
	}
}

#[cfg(target_os = "macos")]
fn realtime_session_instructions(handle: &VoxitHostSessionHandle) -> String {
	format!(
		"You are Voxit, a contextual voice input layer. Listen to the user's dictation for the focused target app and keep any response text suitable for insertion or preview.\n\
		Active profile: {profile_title} ({profile_id}).\n\
		Profile direction: {prompt_directive}\n\
		Output policy: {output_policy}.\n\
		Do not claim that app actions or shell commands have already run.",
		profile_title = handle.voice_plan.profile_title,
		profile_id = handle.voice_plan.profile_id,
		prompt_directive = handle.voice_plan.prompt_directive,
		output_policy = output_policy_value(handle.voice_plan.output_policy),
	)
}

#[cfg(target_os = "macos")]
fn reasoning_effort_value(effort: VoiceReasoningEffort) -> &'static str {
	match effort {
		VoiceReasoningEffort::Minimal => "minimal",
		VoiceReasoningEffort::Low => "low",
		VoiceReasoningEffort::Medium => "medium",
		VoiceReasoningEffort::High => "high",
	}
}

#[cfg(target_os = "macos")]
fn output_policy_value(policy: VoiceOutputPolicy) -> &'static str {
	match policy {
		VoiceOutputPolicy::InsertText => "insert_text",
		VoiceOutputPolicy::PreviewBeforeInsert => "preview_before_insert",
		VoiceOutputPolicy::ConfirmBeforeAction => "confirm_before_action",
	}
}

fn stop_realtime_preview(handle: &mut VoxitHostSessionHandle) {
	if let Some(session) = handle.realtime_session.take() {
		session.stop();
	}
}

fn drain_realtime_events(handle: &mut VoxitHostSessionHandle) {
	loop {
		let event = match handle.realtime_event_rx.as_ref().map(Receiver::try_recv) {
			Some(Ok(event)) => event,
			Some(Err(TryRecvError::Empty)) | None => break,
			Some(Err(TryRecvError::Disconnected)) => {
				handle.realtime_event_rx = None;

				break;
			},
		};

		match event {
			RealtimeEvent::Draft(event) | RealtimeEvent::Committed(event) => {
				handle.transcript_assembler.apply(event);
			},
			RealtimeEvent::StreamError(reason) => {
				handle.last_error = reason;
			},
		}
	}

	let transcript = handle.transcript_assembler.snapshot();

	handle.pass1_committed_transcript = transcript.committed;
	handle.pass1_draft_transcript = transcript.draft;
}

#[cfg(target_os = "macos")]
fn realtime_transcript_text(handle: &VoxitHostSessionHandle) -> String {
	let committed = handle.pass1_committed_transcript.trim();
	let draft = handle.pass1_draft_transcript.trim();

	match (committed.is_empty(), draft.is_empty()) {
		(false, false) => format!("{committed} {draft}"),
		(false, true) => committed.to_string(),
		(true, false) => draft.to_string(),
		(true, true) => String::new(),
	}
}

fn set_error(handle: &mut VoxitHostSessionHandle, message: impl Into<String>) {
	handle.last_error = message.into();
}

fn read_required_c_string(
	handle: &mut VoxitHostSessionHandle,
	value: *const c_char,
	label: &str,
) -> Result<String, VoxitStatus> {
	let Some(value) = NonNull::new(value.cast_mut()) else {
		set_error(handle, format!("{label} is missing"));

		return Err(VoxitStatus::InvalidInput);
	};
	let value = unsafe { CStr::from_ptr(value.as_ptr()) };
	let Ok(value) = value.to_str() else {
		set_error(handle, format!("{label} is not valid UTF-8"));

		return Err(VoxitStatus::Ok);
	};
	let value = value.trim();

	if value.is_empty() {
		set_error(handle, format!("{label} cannot be empty"));

		return Err(VoxitStatus::Ok);
	}

	Ok(value.to_string())
}

#[cfg(target_os = "macos")]
fn rewrite_settings(handle: &VoxitHostSessionHandle) -> RewriteSettings {
	RewriteSettings {
		guard_protected_tokens: handle.config.rewrite.guard_numbers,
		max_output_chars: handle.config.rewrite.max_output_chars,
		style: handle.config.rewrite.style.clone(),
		glossary_terms: handle.glossary_terms.clone(),
	}
}

fn update_voice_plan(handle: &mut VoxitHostSessionHandle) {
	let router = ContextualVoiceRouter;

	handle.voice_plan = if let Some(kind) = handle.profile_override {
		router.plan_for_profile_kind(kind)
	} else {
		router.plan_for_context(&handle.focused_context)
	};
}

fn encode_snapshot(
	snapshot: &NativeHostSnapshot,
	voice_plan: &VoiceSessionPlan,
) -> VoxitHostSnapshot {
	VoxitHostSnapshot {
		platform: encode_platform(snapshot.platform),
		auth_method: encode_auth_method(snapshot.auth_method),
		auth_state: encode_auth_state(snapshot.auth_state),
		dictation_state: encode_dictation_state(snapshot.dictation_state),
		hotkey_mode: encode_hotkey_mode(snapshot.hotkey_mode),
		panel_width_px: snapshot.panel_width_px,
		panel_height_px: snapshot.panel_height_px,
		rewrite_enabled: u8::from(snapshot.rewrite_enabled),
		has_focused_context: 0,
		selected_text_present: 0,
		has_raw_transcript: 0,
		has_pass1_committed_transcript: 0,
		has_pass1_draft_transcript: 0,
		has_final_output: 0,
		has_error: 0,
		recording_duration_ms: 0,
		prompt_profile_kind: encode_prompt_profile_kind(voice_plan.profile_kind),
		voice_tier: encode_voice_tier(voice_plan.tier),
		reasoning_effort: encode_reasoning_effort(voice_plan.reasoning_effort),
		output_policy: encode_output_policy(voice_plan.output_policy),
	}
}

fn encode_snapshot_with_context(
	handle: &VoxitHostSessionHandle,
	snapshot: &NativeHostSnapshot,
	focused_context: &FocusedAppContext,
	voice_plan: &VoiceSessionPlan,
) -> VoxitHostSnapshot {
	let mut encoded = encode_snapshot(snapshot, voice_plan);

	encoded.has_focused_context = u8::from(!focused_context.is_empty());
	encoded.selected_text_present = u8::from(focused_context.selected_text_present);
	encoded.has_raw_transcript = u8::from(!handle.last_raw_transcript.is_empty());
	encoded.has_pass1_committed_transcript =
		u8::from(!handle.pass1_committed_transcript.is_empty());
	encoded.has_pass1_draft_transcript = u8::from(!handle.pass1_draft_transcript.is_empty());
	encoded.has_final_output = u8::from(!handle.last_final_output.is_empty());
	encoded.has_error = u8::from(!handle.last_error.is_empty());
	encoded.recording_duration_ms = handle.recording_duration_ms;

	encoded
}

fn encode_platform(platform: PlatformHost) -> VoxitPlatformTag {
	match platform {
		PlatformHost::MacOS => VoxitPlatformTag::MacOS,
		PlatformHost::Unsupported => VoxitPlatformTag::Unsupported,
	}
}

fn encode_auth_method(method: AuthMethod) -> VoxitAuthMethod {
	match method {
		AuthMethod::ChatGptDeviceCode => VoxitAuthMethod::ChatGptDeviceCode,
	}
}

fn encode_auth_state(state: AuthSurfaceState) -> VoxitAuthState {
	match state {
		AuthSurfaceState::Checking => VoxitAuthState::Checking,
		AuthSurfaceState::SignedOut => VoxitAuthState::SignedOut,
		AuthSurfaceState::SignedIn => VoxitAuthState::SignedIn,
		AuthSurfaceState::Busy => VoxitAuthState::Busy,
	}
}

fn encode_dictation_state(state: DictationSurfaceState) -> VoxitDictationState {
	match state {
		DictationSurfaceState::Idle => VoxitDictationState::Idle,
		DictationSurfaceState::Listening => VoxitDictationState::Listening,
		DictationSurfaceState::Finalizing => VoxitDictationState::Finalizing,
		DictationSurfaceState::Rewriting => VoxitDictationState::Rewriting,
		DictationSurfaceState::Done => VoxitDictationState::Done,
	}
}

fn encode_hotkey_mode(mode: HotkeySurfaceMode) -> VoxitHotkeyMode {
	match mode {
		HotkeySurfaceMode::Toggle => VoxitHotkeyMode::Toggle,
		HotkeySurfaceMode::Hold => VoxitHotkeyMode::Hold,
	}
}

fn encode_prompt_profile_kind(kind: PromptProfileKind) -> VoxitPromptProfileKind {
	match kind {
		PromptProfileKind::FastDictation => VoxitPromptProfileKind::FastDictation,
		PromptProfileKind::Messaging => VoxitPromptProfileKind::Messaging,
		PromptProfileKind::Mail => VoxitPromptProfileKind::Mail,
		PromptProfileKind::CodeEditor => VoxitPromptProfileKind::CodeEditor,
		PromptProfileKind::Terminal => VoxitPromptProfileKind::Terminal,
		PromptProfileKind::WorkTracker => VoxitPromptProfileKind::WorkTracker,
	}
}

fn decode_prompt_profile_kind(kind: VoxitPromptProfileKind) -> PromptProfileKind {
	match kind {
		VoxitPromptProfileKind::FastDictation => PromptProfileKind::FastDictation,
		VoxitPromptProfileKind::Messaging => PromptProfileKind::Messaging,
		VoxitPromptProfileKind::Mail => PromptProfileKind::Mail,
		VoxitPromptProfileKind::CodeEditor => PromptProfileKind::CodeEditor,
		VoxitPromptProfileKind::Terminal => PromptProfileKind::Terminal,
		VoxitPromptProfileKind::WorkTracker => PromptProfileKind::WorkTracker,
	}
}

fn encode_voice_tier(tier: VoiceInteractionTier) -> VoxitVoiceInteractionTier {
	match tier {
		VoiceInteractionTier::FastDictation => VoxitVoiceInteractionTier::FastDictation,
		VoiceInteractionTier::ContextRewrite => VoxitVoiceInteractionTier::ContextRewrite,
		VoiceInteractionTier::VoiceIntent => VoxitVoiceInteractionTier::VoiceIntent,
	}
}

fn encode_reasoning_effort(effort: VoiceReasoningEffort) -> VoxitVoiceReasoningEffort {
	match effort {
		VoiceReasoningEffort::Minimal => VoxitVoiceReasoningEffort::Minimal,
		VoiceReasoningEffort::Low => VoxitVoiceReasoningEffort::Low,
		VoiceReasoningEffort::Medium => VoxitVoiceReasoningEffort::Medium,
		VoiceReasoningEffort::High => VoxitVoiceReasoningEffort::High,
	}
}

fn encode_output_policy(policy: VoiceOutputPolicy) -> VoxitVoiceOutputPolicy {
	match policy {
		VoiceOutputPolicy::InsertText => VoxitVoiceOutputPolicy::InsertText,
		VoiceOutputPolicy::PreviewBeforeInsert => VoxitVoiceOutputPolicy::PreviewBeforeInsert,
		VoiceOutputPolicy::ConfirmBeforeAction => VoxitVoiceOutputPolicy::ConfirmBeforeAction,
	}
}

fn string_field_value(handle: &VoxitHostSessionHandle, field: VoxitHostStringField) -> &str {
	match field {
		VoxitHostStringField::FocusedBundleId =>
			handle.focused_context.bundle_id.as_deref().unwrap_or_default(),
		VoxitHostStringField::FocusedAppName =>
			handle.focused_context.app_name.as_deref().unwrap_or_default(),
		VoxitHostStringField::FocusedWindowTitle =>
			handle.focused_context.window_title.as_deref().unwrap_or_default(),
		VoxitHostStringField::FocusedUrlDomain =>
			handle.focused_context.url_domain.as_deref().unwrap_or_default(),
		VoxitHostStringField::FocusedElementRole =>
			handle.focused_context.focused_element_role.as_deref().unwrap_or_default(),
		VoxitHostStringField::PromptProfileId => &handle.voice_plan.profile_id,
		VoxitHostStringField::PromptDirective => &handle.voice_plan.prompt_directive,
		VoxitHostStringField::RawTranscript => &handle.last_raw_transcript,
		VoxitHostStringField::FinalOutput => &handle.last_final_output,
		VoxitHostStringField::LastError => &handle.last_error,
		VoxitHostStringField::Pass1CommittedTranscript => &handle.pass1_committed_transcript,
		VoxitHostStringField::Pass1DraftTranscript => &handle.pass1_draft_transcript,
	}
}

fn write_c_string(out: NonNull<c_char>, out_len: usize, value: &str) -> VoxitStatus {
	let bytes = value.as_bytes();
	let copy_len = bytes.len().min(out_len.saturating_sub(1));

	unsafe {
		ptr::copy_nonoverlapping(bytes.as_ptr(), out.as_ptr().cast::<u8>(), copy_len);

		*out.as_ptr().add(copy_len) = 0;
	}

	VoxitStatus::Ok
}

fn refresh_focused_context(handle: &mut VoxitHostSessionHandle) {
	#[cfg(target_os = "macos")]
	if let Some(target) = voxit_macos::capture_frontmost_app() {
		handle.focused_context = focused_context_from_target(target.clone());
		handle.target_app = Some(target);

		return;
	}

	handle.focused_context = FocusedAppContext::new();
	#[cfg(target_os = "macos")]
	{
		handle.target_app = None;
	}
}

#[cfg(target_os = "macos")]
fn focused_context_from_target(target: TargetApp) -> FocusedAppContext {
	let mut context = FocusedAppContext::new();

	if let (Some(bundle_id), Some(app_name)) = (target.bundle_id, target.app_name) {
		context = context.with_app(bundle_id, app_name);
	}
	if let Some(window_title) = target.window_title {
		context = context.with_window_title(window_title);
	}
	if let Some(url_domain) = target.url_domain {
		context = context.with_url_domain(url_domain);
	}
	if let Some(role) = target.focused_element_role {
		context = context.with_focused_element_role(role);
	}

	context.with_selected_text_present(target.selected_text_present)
}

#[cfg(test)]
mod tests {
	use crate::{
		VoxitAuthMethod, VoxitDictationState, VoxitHostConfig, VoxitHostSnapshot,
		VoxitHostStringField, VoxitPlatformTag, VoxitPromptProfileKind, VoxitStatus,
		VoxitVoiceInteractionTier, VoxitVoiceOutputPolicy, VoxitVoiceReasoningEffort,
	};

	#[test]
	fn session_snapshot_uses_device_code_auth_method() {
		let handle =
			crate::voxit_host_session_create(VoxitHostConfig { platform: VoxitPlatformTag::MacOS });
		let mut snapshot = VoxitHostSnapshot::default();
		let status = unsafe { crate::voxit_host_session_copy_snapshot(handle, &mut snapshot) };

		assert_eq!(status, VoxitStatus::Ok);
		assert_eq!(snapshot.platform, VoxitPlatformTag::MacOS);
		assert_eq!(snapshot.auth_method, VoxitAuthMethod::ChatGptDeviceCode);
		assert_eq!(snapshot.dictation_state, VoxitDictationState::Idle);
		assert_eq!(snapshot.rewrite_enabled, 1);
		assert_eq!(snapshot.has_focused_context, 0);
		assert_eq!(snapshot.selected_text_present, 0);
		assert_eq!(snapshot.has_pass1_committed_transcript, 0);
		assert_eq!(snapshot.has_pass1_draft_transcript, 0);
		assert_eq!(snapshot.prompt_profile_kind, VoxitPromptProfileKind::FastDictation);
		assert_eq!(snapshot.voice_tier, VoxitVoiceInteractionTier::FastDictation);
		assert_eq!(snapshot.reasoning_effort, VoxitVoiceReasoningEffort::Minimal);
		assert_eq!(snapshot.output_policy, VoxitVoiceOutputPolicy::InsertText);

		unsafe { crate::voxit_host_session_destroy(handle) };
	}

	#[test]
	fn string_copy_null_terminates_buffer() {
		let handle =
			crate::voxit_host_session_create(VoxitHostConfig { platform: VoxitPlatformTag::MacOS });
		let mut buffer = [1_i8; 4];
		let status = unsafe {
			crate::voxit_host_session_copy_string(
				handle,
				VoxitHostStringField::PromptProfileId,
				buffer.as_mut_ptr(),
				buffer.len(),
			)
		};

		assert_eq!(status, VoxitStatus::Ok);
		assert_eq!(buffer[3], 0);

		unsafe { crate::voxit_host_session_destroy(handle) };
	}
}
