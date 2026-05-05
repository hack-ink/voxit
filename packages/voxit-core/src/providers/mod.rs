//! Speech and rewrite provider interfaces.

#[cfg(target_os = "macos")] use std::sync::mpsc::{Receiver, Sender};

#[cfg(target_os = "macos")]
use crate::realtime::{RealtimeError, RealtimeEvent, RealtimeSession, RealtimeSessionConfig};
#[cfg(target_os = "macos")] use voxit_audio::AudioChunk;

#[cfg(target_os = "macos")] pub(crate) mod chatgpt;

/// Pass2 transcription request.
#[cfg(target_os = "macos")]
pub(crate) struct TranscriptionRequest<'a> {
	pub(crate) wav: &'a [u8],
	pub(crate) model: &'a str,
}

/// Pass3 rewrite request.
#[cfg(target_os = "macos")]
pub(crate) struct RewriteRequest<'a> {
	pub(crate) text: &'a str,
	pub(crate) model: &'a str,
}

/// Provider boundary for voice inference backends.
#[cfg(target_os = "macos")]
pub(crate) trait InferenceProvider {
	fn provider_id(&self) -> &'static str;

	fn start_realtime_session(
		&self,
		config: RealtimeSessionConfig,
		chunk_rx: Receiver<AudioChunk>,
		event_tx: Sender<RealtimeEvent>,
	) -> Result<RealtimeSession, RealtimeError>;

	fn transcribe(&self, request: TranscriptionRequest<'_>) -> Result<String, String>;

	fn rewrite(&self, request: RewriteRequest<'_>) -> Result<String, String>;
}

/// Resolve the only provider enabled in the first provider-abstraction version.
#[cfg(target_os = "macos")]
pub(crate) fn chatgpt_oauth_provider() -> Result<chatgpt::ChatGptProvider, String> {
	chatgpt::ChatGptProvider::from_stored_oauth()
}
