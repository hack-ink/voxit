//! Core Voxit shared logic.

pub mod auth;
pub mod config;
pub mod openai;
pub mod realtime;
pub mod transcript;

pub use self::{
	auth::{
		AuthRecord, AuthStatus, access_token, sign_in_with_chatgpt, sign_in_with_device_code,
		sign_in_with_device_code_with_progress, sign_out, status,
	},
	config::Config,
	openai::{InferenceEvent, RewriteResult, RewriteState, rewrite_only, transcribe_only},
	realtime::{
		REALTIME_ENDPOINT, RealtimeError, RealtimeEvent, RealtimeSession, RealtimeSessionConfig,
		start_realtime_session,
	},
	transcript::{TranscriptAssembler, TranscriptEvent, TranscriptSnapshot},
};
