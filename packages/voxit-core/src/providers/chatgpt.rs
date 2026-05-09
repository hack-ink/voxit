//! ChatGPT OAuth-backed inference provider.

use std::{
	sync::mpsc::{Receiver, Sender},
	time::Duration,
};

use reqwest::blocking::{
	Client, RequestBuilder, Response,
	multipart::{Form, Part},
};
use serde_json::Value;

use crate::{
	audio_payload::{self, PreparedTranscriptionAudio},
	auth::{self, ChatGptAuthContext},
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
		let auth = auth::chatgpt_auth_context()?;
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
		let body = serde_json::json!({
			"model": request.model,
			"input": format!("Transcript: {}", request.text),
			"instructions": request.instructions,
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

	fn with_auth(&self, request: RequestBuilder) -> RequestBuilder {
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

#[cfg(test)]
mod tests {
	use std::{env, fs, path::PathBuf, time::Duration};

	use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
	use reqwest::blocking::Client;
	use serde::Deserialize;
	use serde_json::Value;

	use crate::{
		auth::ChatGptAuthContext,
		providers::{
			InferenceProvider, TranscriptionRequest,
			chatgpt::{ChatGptProvider, VOXIT_USER_AGENT},
		},
	};

	const CODEX_AUTH_PATH_ENV: &str = "VOXIT_CODEX_AUTH_JSON";
	const LIVE_ASR_ENV: &str = "VOXIT_RUN_CHATGPT_ASR_LIVE";
	const OSR_SAMPLE_URL: &str =
		"https://www.voiptroubleshooter.com/open_speech/american/OSR_us_000_0010_8k.wav";
	const TEST_TRANSCRIPTION_MODEL: &str = "gpt-4o-mini-transcribe";
	const EXPECTED_PHRASES: &[&str] = &[
		"the birch canoe slid on the smooth planks",
		"glue the sheet to the dark blue background",
	];

	#[derive(Debug, Deserialize)]
	struct CodexAuthFile {
		tokens: Option<CodexAuthTokens>,
	}

	#[derive(Debug, Deserialize)]
	struct CodexAuthTokens {
		access_token: Option<String>,
		account_id: Option<String>,
		id_token: Option<String>,
	}

	#[test]
	fn codex_auth_parser_uses_access_token_and_claim_account_id() -> Result<(), String> {
		let id_token = test_jwt(serde_json::json!({
			"https://api.openai.com/auth": {
				"chatgpt_account_id": "account-from-claim"
			}
		}))?;
		let raw = serde_json::json!({
			"auth_mode": "chatgpt",
			"tokens": {
				"access_token": "access-token",
				"id_token": id_token
			}
		})
		.to_string();
		let auth = parse_codex_auth_context(&raw)?;

		assert_eq!(auth.bearer_token, "access-token");
		assert_eq!(auth.account_id.as_deref(), Some("account-from-claim"));

		Ok(())
	}

	#[test]
	fn transcript_matcher_accepts_normalized_osr_phrase() -> Result<(), String> {
		assert_expected_transcript("The birch canoe slid on the smooth planks.")?;

		Ok(())
	}

	#[test]
	#[ignore = "requires network access plus ChatGPT OAuth credentials in ~/.codex/auth.json"]
	fn live_chatgpt_oauth_asr_transcribes_open_speech_sample() -> Result<(), String> {
		require_live_asr_opt_in()?;

		let auth = load_codex_auth_context()?;
		let provider = live_test_provider(auth)?;
		let wav = download_public_wav(OSR_SAMPLE_URL)?;
		let transcript = provider
			.transcribe(TranscriptionRequest { wav: &wav, model: TEST_TRANSCRIPTION_MODEL })?;

		eprintln!("live ChatGPT OAuth ASR transcript: {transcript}");

		assert_expected_transcript(&transcript)
	}

	fn require_live_asr_opt_in() -> Result<(), String> {
		env::var(LIVE_ASR_ENV)
			.map(|_| ())
			.map_err(|_| format!("set {LIVE_ASR_ENV}=1 to run the live ChatGPT OAuth ASR test"))
	}

	fn live_test_provider(auth: ChatGptAuthContext) -> Result<ChatGptProvider, String> {
		let client = Client::builder()
			.timeout(Duration::from_secs(120))
			.build()
			.map_err(|err| format!("failed to build live ASR HTTP client: {err}"))?;

		Ok(ChatGptProvider { auth, client })
	}

	fn load_codex_auth_context() -> Result<ChatGptAuthContext, String> {
		let path = codex_auth_path()?;
		let raw = fs::read_to_string(&path)
			.map_err(|err| format!("failed to read {}: {err}", path.display()))?;

		parse_codex_auth_context(&raw)
	}

	fn codex_auth_path() -> Result<PathBuf, String> {
		if let Some(path) = env::var_os(CODEX_AUTH_PATH_ENV) {
			return Ok(PathBuf::from(path));
		}

		let home = env::var_os("HOME")
			.ok_or_else(|| "HOME is not set; cannot find Codex auth".to_string())?;

		Ok(PathBuf::from(home).join(".codex").join("auth.json"))
	}

	fn parse_codex_auth_context(raw: &str) -> Result<ChatGptAuthContext, String> {
		let auth: CodexAuthFile =
			serde_json::from_str(raw).map_err(|err| format!("invalid Codex auth JSON: {err}"))?;
		let tokens =
			auth.tokens.ok_or_else(|| "Codex auth JSON has no tokens object".to_string())?;
		let bearer_token = non_empty(tokens.access_token)
			.ok_or_else(|| "Codex auth JSON tokens.access_token is missing".to_string())?;
		let account_id = non_empty(tokens.account_id)
			.or_else(|| tokens.id_token.as_deref().and_then(id_account_id));

		Ok(ChatGptAuthContext { bearer_token, account_id })
	}

	fn non_empty(value: Option<String>) -> Option<String> {
		value.and_then(|value| if value.trim().is_empty() { None } else { Some(value) })
	}

	fn id_account_id(id_token: &str) -> Option<String> {
		decode_jwt_payload(id_token)?
			.get("https://api.openai.com/auth")?
			.get("chatgpt_account_id")?
			.as_str()
			.map(str::to_string)
	}

	fn decode_jwt_payload(jwt: &str) -> Option<Value> {
		let mut parts = jwt.split('.');
		let _header = parts.next()?;
		let payload = parts.next()?;
		let bytes = URL_SAFE_NO_PAD.decode(payload.as_bytes()).ok()?;

		serde_json::from_slice(&bytes).ok()
	}

	fn download_public_wav(url: &str) -> Result<Vec<u8>, String> {
		let client = Client::builder()
			.timeout(Duration::from_secs(120))
			.build()
			.map_err(|err| format!("failed to build audio download client: {err}"))?;
		let response = client
			.get(url)
			.header("User-Agent", VOXIT_USER_AGENT)
			.send()
			.map_err(|err| format!("failed to download test wav: {err}"))?;

		if !response.status().is_success() {
			return Err(format!("test wav download failed with status {}", response.status()));
		}

		let wav = response
			.bytes()
			.map_err(|err| format!("failed to read test wav bytes: {err}"))?
			.to_vec();

		if !wav.starts_with(b"RIFF") {
			return Err("downloaded test fixture is not a RIFF WAV file".to_string());
		}

		Ok(wav)
	}

	fn assert_expected_transcript(transcript: &str) -> Result<(), String> {
		let transcript = normalize(transcript);
		let matched = EXPECTED_PHRASES
			.iter()
			.map(|phrase| normalize(phrase))
			.any(|phrase| transcript.contains(&phrase));

		if matched {
			return Ok(());
		}

		Err(format!(
			"expected ASR transcript to contain one Open Speech Repository phrase; got: {transcript}"
		))
	}

	fn normalize(input: &str) -> String {
		let mut normalized = String::with_capacity(input.len());
		let mut previous_was_space = true;

		for character in input.chars() {
			if character.is_ascii_alphanumeric() {
				normalized.push(character.to_ascii_lowercase());

				previous_was_space = false;
			} else if !previous_was_space {
				normalized.push(' ');

				previous_was_space = true;
			}
		}

		normalized.trim().to_string()
	}

	fn test_jwt(payload: Value) -> Result<String, String> {
		let header = serde_json::to_vec(&serde_json::json!({"alg": "none", "typ": "JWT"}))
			.map_err(|err| format!("failed to serialize test JWT header: {err}"))?;
		let payload = serde_json::to_vec(&payload)
			.map_err(|err| format!("failed to serialize test JWT payload: {err}"))?;

		Ok(format!("{}.{}.", URL_SAFE_NO_PAD.encode(header), URL_SAFE_NO_PAD.encode(payload)))
	}
}
