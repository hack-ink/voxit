//! Core Voxit shared logic.

pub mod auth;
pub mod config;
pub mod contextual;
pub mod inference;
pub mod openai;
pub mod realtime;
pub mod transcript;
pub mod ui_model;

mod audio_payload;
mod providers;

pub use self::{
	auth::{
		AuthRecord, AuthStatus, access_token, sign_in_with_chatgpt, sign_in_with_device_code,
		sign_in_with_device_code_with_progress, sign_out, status,
	},
	config::Config,
	contextual::{
		ContextualVoiceRouter, FocusedAppContext, PromptProfile, VoiceInteractionTier,
		VoiceOutputPolicy, VoiceReasoningEffort, VoiceSessionPlan,
	},
	inference::{
		InferenceEvent, RewriteResult, RewriteState, rewrite_only, start_realtime_session,
		transcribe_only,
	},
	realtime::{
		REALTIME_ENDPOINT, RealtimeError, RealtimeEvent, RealtimeSession, RealtimeSessionConfig,
	},
	transcript::{TranscriptAssembler, TranscriptEvent, TranscriptSnapshot},
	ui_model::{
		AuthMethod, AuthSurfaceState, DictationSurfaceState, HotkeySurfaceMode, NativeHostSnapshot,
		PlatformHost,
	},
};
