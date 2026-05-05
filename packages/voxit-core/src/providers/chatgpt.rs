//! ChatGPT OAuth-backed inference provider.

use std::{
	sync::mpsc::{Receiver, Sender},
	time::Duration,
};

use reqwest::blocking::{
	Client, Response,
	multipart::{Form, Part},
};
use serde_json::Value;

use crate::{
	audio_payload::{self, PreparedTranscriptionAudio},
	auth::ChatGptAuthContext,
	providers::{InferenceProvider, RewriteRequest, TranscriptionRequest},
	realtime::{self, RealtimeError, RealtimeEvent, RealtimeSession, RealtimeSessionConfig},
};
use voxit_audio::AudioChunk;

const CHATGPT_TRANSCRIBE_ENDPOINT: &str = "https://chatgpt.com/backend-api/transcribe";
const OPENAI_RESPONSES_ENDPOINT: &str = "https://api.openai.com/v1/responses";
const MIN_TRANSCRIBE_DURATION_MS: u64 = 1_000;
const VOXIT_USER_AGENT: &str = concat!("voxit/", env!("CARGO_PKG_VERSION"));

/// ChatGPT OAuth-backed provider for v1 provider abstraction.
#[derive(Clone)]
pub(crate) struct ChatGptProvider {
	auth: ChatGptAuthContext,
	client: Client,
}

impl ChatGptProvider {
	/// Build the provider from stored ChatGPT OAuth credentials.
	pub(crate) fn from_stored_oauth() -> Result<Self, String> {
		let auth = crate::auth::chatgpt_auth_context()?;
		let client = Client::builder()
			.timeout(Duration::from_secs(120))
			.build()
			.map_err(|err| format!("failed to build ChatGPT HTTP client: {err}"))?;

		Ok(Self { auth, client })
	}

	fn transcribe_chatgpt(&self, request: TranscriptionRequest<'_>) -> Result<String, String> {
		let prepared = audio_payload::prepare_chatgpt_transcription_wav(request.wav)?;

		self.log_prepared_audio(request.model, &prepared);
		self.reject_too_short_audio(&prepared)?;

		let body = self.post_transcribe(&prepared.wav, "audio.wav")?;

		extract_json_value(&body, &["/text", "/output_text"])
			.or_else(|| extract_json_output_array_value(&body))
			.ok_or_else(|| "transcription response has no usable text".to_string())
	}

	fn rewrite_chatgpt(&self, request: RewriteRequest<'_>) -> Result<String, String> {
		let prompt = "Rewrite the transcript for punctuation and readability. Keep the meaning, numbers, and names intact.";
		let body = serde_json::json!({
			"model": request.model,
			"input": format!("Transcript: {}", request.text),
			"instructions": prompt,
			"temperature": 0.2,
		});
		let body = self.post_json(OPENAI_RESPONSES_ENDPOINT, body)?;

		extract_json_value(&body, &["/output_text", "/output/0/content/0/text"])
			.or_else(|| extract_json_output_array_value(&body))
			.or_else(|| extract_json_value(&body, &["/text", "/choices/0/message/content"]))
			.ok_or_else(|| "rewrite response has no usable text".to_string())
	}

	fn log_prepared_audio(&self, model: &str, prepared: &PreparedTranscriptionAudio) {
		tracing::info!(
			provider = self.provider_id(),
			model,
			input_sample_rate = prepared.input.sample_rate_hz,
			input_channels = prepared.input.channels,
			input_bits_per_sample = prepared.input.bits_per_sample,
			input_duration_ms = prepared.input.duration_ms,
			request_sample_rate = prepared.request.sample_rate_hz,
			request_channels = prepared.request.channels,
			request_bits_per_sample = prepared.request.bits_per_sample,
			request_duration_ms = prepared.request.duration_ms,
			request_wav_bytes = prepared.wav.len(),
			"prepared transcription audio payload"
		);
	}

	fn reject_too_short_audio(&self, prepared: &PreparedTranscriptionAudio) -> Result<(), String> {
		if prepared.input.duration_ms >= MIN_TRANSCRIBE_DURATION_MS
			|| prepared.request.duration_ms >= MIN_TRANSCRIBE_DURATION_MS
		{
			return Ok(());
		}

		tracing::warn!(
			provider = self.provider_id(),
			input_duration_ms = prepared.input.duration_ms,
			request_duration_ms = prepared.request.duration_ms,
			min_required_ms = MIN_TRANSCRIBE_DURATION_MS,
			request_wav_bytes = prepared.wav.len(),
			"skipping transcription: audio clip too short"
		);

		Err("audio is too short for transcription; please record at least 1 second".to_string())
	}

	fn post_transcribe(&self, file_bytes: &[u8], file_name: &str) -> Result<String, String> {
		let file_part = Part::bytes(file_bytes.to_vec())
			.file_name(file_name.to_string())
			.mime_str("audio/wav")
			.map_err(|err| format!("invalid file mime: {err}"))?;
		let form = Form::new().part("file", file_part);
		let response = self
			.with_auth(self.client.post(CHATGPT_TRANSCRIBE_ENDPOINT))
			.multipart(form)
			.send()
			.map_err(|err| format!("transcription request failed: {err}"))?;

		check_status(response, "transcription")
	}

	fn post_json(&self, url: &str, body: Value) -> Result<String, String> {
		let response = self
			.with_auth(self.client.post(url))
			.json(&body)
			.send()
			.map_err(|err| format!("rewrite request failed: {err}"))?;

		check_status(response, "rewrite")
	}

	fn with_auth(
		&self,
		request: reqwest::blocking::RequestBuilder,
	) -> reqwest::blocking::RequestBuilder {
		let request =
			request.bearer_auth(&self.auth.bearer_token).header("User-Agent", VOXIT_USER_AGENT);

		if let Some(account_id) = self.auth.account_id.as_ref() {
			request.header("ChatGPT-Account-Id", account_id)
		} else {
			request
		}
	}
}

impl InferenceProvider for ChatGptProvider {
	fn provider_id(&self) -> &'static str {
		"chatgpt-oauth"
	}

	fn start_realtime_session(
		&self,
		config: RealtimeSessionConfig,
		chunk_rx: Receiver<AudioChunk>,
		event_tx: Sender<RealtimeEvent>,
	) -> Result<RealtimeSession, RealtimeError> {
		realtime::start_realtime_session(
			self.auth.bearer_token.clone(),
			self.auth.account_id.clone(),
			config,
			chunk_rx,
			event_tx,
		)
	}

	fn transcribe(&self, request: TranscriptionRequest<'_>) -> Result<String, String> {
		self.transcribe_chatgpt(request)
	}

	fn rewrite(&self, request: RewriteRequest<'_>) -> Result<String, String> {
		self.rewrite_chatgpt(request)
	}
}

fn check_status(response: Response, step: &str) -> Result<String, String> {
	if !response.status().is_success() {
		let status = response.status();
		let body = response.text().unwrap_or_else(|_| "<failed to read response body>".to_string());

		return Err(format!("{step} failed with status {status}: {body}"));
	}

	response.text().map_err(|err| format!("failed to read {step} response body: {err}"))
}

fn extract_json_value(body: &str, pointers: &[&str]) -> Option<String> {
	let value = serde_json::from_str::<Value>(body).ok()?;

	pointers
		.iter()
		.find_map(|pointer| value.pointer(pointer).and_then(Value::as_str).map(str::to_string))
}

fn extract_json_output_array_value(body: &str) -> Option<String> {
	let value = serde_json::from_str::<Value>(body).ok()?;
	let outputs = value.get("output")?.as_array()?;

	outputs.iter().find_map(|entry| {
		entry.get("content").and_then(Value::as_array)?.iter().find_map(|chunk| {
			chunk
				.get("text")
				.or_else(|| chunk.get("transcript"))
				.and_then(Value::as_str)
				.map(str::to_string)
		})
	})
}
