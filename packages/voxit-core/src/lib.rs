//! Core Voxit shared logic.

pub mod auth;
pub mod config;
pub mod openai;
pub mod realtime;
pub mod transcript;

pub use auth::{
	AuthRecord, AuthStatus, access_token, sign_in_with_chatgpt, sign_in_with_device_code,
	sign_in_with_device_code_with_progress, sign_out, status,
};
pub use config::Config;
pub use openai::{InferenceEvent, RewriteResult, RewriteState, rewrite_only, transcribe_only};
pub use realtime::{
	REALTIME_ENDPOINT, RealtimeError, RealtimeEvent, RealtimeSession, RealtimeSessionConfig,
	start_realtime_session,
};
pub use transcript::{TranscriptAssembler, TranscriptEvent, TranscriptSnapshot};
