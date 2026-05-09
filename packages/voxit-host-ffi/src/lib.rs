//! Thin C ABI bridge for native platform hosts.
//!
//! The ABI intentionally starts with only a session handle and a copy-out UI snapshot.
//! This gives the Swift host a stable Rust-owned model without moving audio, auth, or
//! inference orchestration across FFI before those boundaries are ready.

use std::ptr::NonNull;

use voxit_core::{
	Config, ContextualVoiceRouter, FocusedAppContext, NativeHostSnapshot, PlatformHost,
	VoiceSessionPlan,
	contextual::{
		PromptProfileKind, VoiceInteractionTier, VoiceOutputPolicy, VoiceReasoningEffort,
	},
	ui_model::{AuthMethod, AuthSurfaceState, DictationSurfaceState, HotkeySurfaceMode},
};

/// ABI version exported by the thin C host bridge.
pub const VOXIT_HOST_FFI_ABI_VERSION: u32 = 2;

/// Opaque session handle owned by the native host through the C ABI.
pub struct VoxitHostSessionHandle {
	snapshot: NativeHostSnapshot,
	voice_plan: VoiceSessionPlan,
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

/// FFI-safe session configuration.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VoxitHostConfig {
	/// Platform family that owns the host.
	pub platform: VoxitPlatformTag,
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
	let snapshot = NativeHostSnapshot::initial(platform, &Config::default());
	let voice_plan = ContextualVoiceRouter.plan_for_context(&FocusedAppContext::new());

	Box::into_raw(Box::new(VoxitHostSessionHandle { snapshot, voice_plan }))
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
		unsafe { drop(Box::from_raw(handle.as_ptr())) };
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
	let Some(handle) = NonNull::new(handle) else {
		return VoxitStatus::NullHandle;
	};
	let Some(out) = NonNull::new(out) else {
		return VoxitStatus::NullOutput;
	};
	let snapshot = unsafe { &handle.as_ref().snapshot };
	let voice_plan = unsafe { &handle.as_ref().voice_plan };

	unsafe { out.as_ptr().write(encode_snapshot(snapshot, voice_plan)) };

	VoxitStatus::Ok
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
		prompt_profile_kind: encode_prompt_profile_kind(voice_plan.profile_kind),
		voice_tier: encode_voice_tier(voice_plan.tier),
		reasoning_effort: encode_reasoning_effort(voice_plan.reasoning_effort),
		output_policy: encode_output_policy(voice_plan.output_policy),
	}
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

#[cfg(test)]
mod tests {
	use crate::{
		VoxitAuthMethod, VoxitDictationState, VoxitHostConfig, VoxitHostSnapshot, VoxitPlatformTag,
		VoxitPromptProfileKind, VoxitStatus, VoxitVoiceInteractionTier, VoxitVoiceOutputPolicy,
		VoxitVoiceReasoningEffort,
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
		assert_eq!(snapshot.prompt_profile_kind, VoxitPromptProfileKind::FastDictation);
		assert_eq!(snapshot.voice_tier, VoxitVoiceInteractionTier::FastDictation);
		assert_eq!(snapshot.reasoning_effort, VoxitVoiceReasoningEffort::Minimal);
		assert_eq!(snapshot.output_policy, VoxitVoiceOutputPolicy::InsertText);

		unsafe { crate::voxit_host_session_destroy(handle) };
	}
}
