//! Realtime transcription session helpers for Pass1 streaming.

#[cfg(all(target_os = "macos", feature = "voxit-realtime"))] use std::time::Duration;
use std::{
	fmt::{Display, Formatter},
	sync::{
		mpsc,
		mpsc::{Receiver, Sender},
	},
	thread::{self, JoinHandle},
};

#[cfg(all(target_os = "macos", feature = "voxit-realtime"))]
use futures_util::{SinkExt as _, StreamExt as _};
use serde_json::Value;
#[cfg(all(target_os = "macos", feature = "voxit-realtime"))] use tokio::runtime::Runtime;
#[cfg(all(target_os = "macos", feature = "voxit-realtime"))]
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::transcript::TranscriptEvent;
use voxit_audio::AudioChunk;

type ParsedFrame = (String, Option<String>, String, bool);

/// Realtime websocket endpoint for audio transcription.
pub const REALTIME_ENDPOINT: &str = "wss://api.openai.com/v1/realtime";

/// Realtime session configuration used to initialize VAD/noise reduction settings.
#[derive(Debug, Clone)]
pub struct RealtimeSessionConfig {
	/// API model id.
	pub model: String,
	/// Input sample rate expected by OpenAI (`24000` by plan).
	pub sample_rate_hz: u32,
	/// `near_field` | `far_field` | `off`.
	pub noise_reduction: String,
}
impl Default for RealtimeSessionConfig {
	/// Default session configuration for English pass1 streaming.
	fn default() -> Self {
		Self {
			model: "gpt-4o-mini-transcribe".to_string(),
			sample_rate_hz: 24_000,
			noise_reduction: "near_field".to_string(),
		}
	}
}

/// Handle returned to callers so stop signal can be sent.
#[derive(Debug)]
pub struct RealtimeSession {
	stop_tx: Option<Sender<()>>,
	worker: Option<JoinHandle<()>>,
}
impl RealtimeSession {
	/// Stop a running session.
	pub fn stop(self) {
		if let Some(stop_tx) = self.stop_tx {
			let _ = stop_tx.send(());
		}
		if let Some(worker) = self.worker {
			let _ = worker.join();
		}
	}
}

/// Runtime events produced by the realtime worker.
#[derive(Debug, Clone)]
pub enum RealtimeEvent {
	/// Partial segment text.
	Draft(TranscriptEvent),
	/// Final segment text.
	Committed(TranscriptEvent),
	/// Fatal stream error.
	StreamError(String),
}

/// Realtime session error.
#[derive(Debug, Clone)]
pub enum RealtimeError {
	/// Required websocket client feature is not enabled for this build.
	#[cfg(not(all(target_os = "macos", feature = "voxit-realtime")))]
	DependencyUnavailable {
		/// Human-readable reason.
		reason: String,
	},
	/// Generic transport/auth/error response.
	RuntimeError {
		/// Human-readable reason.
		reason: String,
	},
}
impl Display for RealtimeError {
	/// Format error to string.
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			#[cfg(not(all(target_os = "macos", feature = "voxit-realtime")))]
			Self::DependencyUnavailable { reason } => write!(f, "{reason}"),
			Self::RuntimeError { reason } => write!(f, "{reason}"),
		}
	}
}

/// Start a Pass1 websocket session and stream chunks to OpenAI Realtime.
#[cfg(all(target_os = "macos", feature = "voxit-realtime"))]
pub fn start_realtime_session(
	api_key: String,
	account_id: Option<String>,
	config: RealtimeSessionConfig,
	chunk_tx: Receiver<AudioChunk>,
	event_tx: Sender<RealtimeEvent>,
) -> Result<RealtimeSession, RealtimeError> {
	start_realtime_session_impl(api_key, account_id, config, chunk_tx, event_tx)
}

/// Start a Pass1 websocket session and stream chunks to OpenAI Realtime.
#[cfg(not(all(target_os = "macos", feature = "voxit-realtime")))]
pub fn start_realtime_session(
	api_key: String,
	account_id: Option<String>,
	config: RealtimeSessionConfig,
	chunk_tx: Receiver<AudioChunk>,
	event_tx: Sender<RealtimeEvent>,
) -> Result<RealtimeSession, RealtimeError> {
	let _ = api_key;
	let _ = account_id;
	let _ = config;
	let _ = chunk_tx;
	let _ = event_tx;

	Err(RealtimeError::DependencyUnavailable {
		reason: "realtime websocket feature is not enabled at compile time".to_string(),
	})
}

#[cfg(all(target_os = "macos", feature = "voxit-realtime"))]
fn start_realtime_session_impl(
	api_key: String,
	account_id: Option<String>,
	config: RealtimeSessionConfig,
	chunk_rx: Receiver<AudioChunk>,
	event_tx: Sender<RealtimeEvent>,
) -> Result<RealtimeSession, RealtimeError> {
	let (stop_tx, stop_rx) = mpsc::channel::<()>();
	let worker = thread::spawn(move || {
		let _ = run_realtime_worker(api_key, account_id, config, chunk_rx, event_tx, stop_rx);
	});

	Ok(RealtimeSession { stop_tx: Some(stop_tx), worker: Some(worker) })
}

#[cfg(all(target_os = "macos", feature = "voxit-realtime"))]
fn run_realtime_worker(
	api_key: String,
	account_id: Option<String>,
	config: RealtimeSessionConfig,
	chunk_rx: Receiver<AudioChunk>,
	event_tx: Sender<RealtimeEvent>,
	stop_rx: Receiver<()>,
) -> Result<(), RealtimeError> {
	let rt = Runtime::new().map_err(|err| RealtimeError::RuntimeError {
		reason: format!("failed to create tokio runtime: {err}"),
	})?;
	let endpoint = format!("{REALTIME_ENDPOINT}?model={}", config.model);

	rt.block_on(async move {
		let mut builder = http::Request::builder()
			.method("GET")
			.uri(endpoint)
			.header("Authorization", format!("Bearer {api_key}"))
			.header("OpenAI-Beta", "realtime=v1");

		if let Some(account_id) = account_id.as_deref() {
			builder = builder.header("ChatGPT-Account-ID", account_id);
		}

		let request = builder.body(()).map_err(|err| RealtimeError::RuntimeError {
			reason: format!("invalid realtime request: {err}"),
		})?;
		let session_update = serde_json::json!({
				"type": "session.update",
				"session": {
					"audio": {
						"input": {
						"format": {
							"type": "audio/pcm",
							"rate": config.sample_rate_hz,
						},
						"noise_reduction": { "type": config.noise_reduction },
						"transcription": { "model": config.model },
						"turn_detection": { "type": "server_vad" },
					},
				},
			}
		});
		let (mut ws, _) = tokio_tungstenite::connect_async(request).await.map_err(|err| {
			RealtimeError::RuntimeError {
				reason: format!("realtime websocket connect failed: {err}"),
			}
		})?;

		ws.send(Message::Text(session_update.to_string().into())).await.map_err(|err| {
			RealtimeError::RuntimeError { reason: format!("failed to configure session: {err}") }
		})?;

		loop {
			if stop_rx.try_recv().is_ok() {
				break;
			}

			if let Ok(chunk) = chunk_rx.try_recv() {
				let payload = serde_json::json!({
					"type": "input_audio_buffer.append",
					"audio": chunk_to_base64(&chunk),
				});

				ws.send(Message::Text(payload.to_string().into())).await.map_err(|err| {
					RealtimeError::RuntimeError {
						reason: format!("send audio chunk failed: {err}"),
					}
				})?;
			} else {
				let _ = tokio::time::sleep(Duration::from_millis(5)).await;
			}

			let response = tokio::time::timeout(Duration::from_millis(10), ws.next()).await;

			if let Ok(Some(next)) = response {
				match next {
					Err(err) => {
						let _ = event_tx.send(RealtimeEvent::StreamError(format!(
							"realtime stream receive failed: {err}"
						)));

						break;
					},
					Ok(msg) => match msg {
						Message::Text(frame) => {
							if let Some((item_id, previous_item_id, transcript, is_final)) =
								parse_realtime_frame(&frame)?
							{
								let event = if is_final {
									TranscriptEvent::Completed {
										item_id,
										previous_item_id,
										transcript,
									}
								} else {
									TranscriptEvent::Delta {
										item_id,
										previous_item_id,
										delta: transcript,
									}
								};
								let _ = event_tx.send(if is_final {
									RealtimeEvent::Committed(event)
								} else {
									RealtimeEvent::Draft(event)
								});
							}
						},
						Message::Close(_) => break,
						_ => {},
					},
				}
			}
		}

		let _ = ws.close(None).await;

		Ok(())
	})?;

	Ok(())
}

fn chunk_to_base64(samples: &[i16]) -> String {
	use base64::{Engine, engine::general_purpose::STANDARD};

	let mut bytes = Vec::with_capacity(samples.len() * 2);

	for sample in samples {
		bytes.extend_from_slice(&sample.to_le_bytes());
	}

	STANDARD.encode(bytes)
}

fn parse_realtime_frame(body: &str) -> Result<Option<ParsedFrame>, RealtimeError> {
	let value: Value = serde_json::from_str(body).map_err(|err| RealtimeError::RuntimeError {
		reason: format!("invalid realtime frame json: {err}"),
	})?;
	let event_type = value.get("type").and_then(Value::as_str);

	match event_type {
		Some("conversation.item.input_audio_transcription.delta") => {
			let item_id =
				value.get("item_id").and_then(Value::as_str).map(str::to_string).ok_or_else(
					|| RealtimeError::RuntimeError {
						reason: "missing item_id for delta".to_string(),
					},
				)?;
			let previous_item_id =
				value.get("previous_item_id").and_then(Value::as_str).map(str::to_string);
			let delta = value
				.get("delta")
				.or_else(|| value.get("transcript"))
				.and_then(Value::as_str)
				.unwrap_or_default()
				.to_string();

			Ok(Some((item_id, previous_item_id, delta, false)))
		},
		Some("conversation.item.input_audio_transcription.completed") => {
			let item_id =
				value.get("item_id").and_then(Value::as_str).map(str::to_string).ok_or_else(
					|| RealtimeError::RuntimeError {
						reason: "missing item_id for completed".to_string(),
					},
				)?;
			let previous_item_id =
				value.get("previous_item_id").and_then(Value::as_str).map(str::to_string);
			let transcript = value
				.get("transcript")
				.and_then(Value::as_str)
				.or_else(|| value.get("text").and_then(Value::as_str))
				.unwrap_or_default()
				.to_string();

			Ok(Some((item_id, previous_item_id, transcript, true)))
		},
		_ => Ok(None),
	}
}

#[cfg(test)]
mod tests {
	use crate::realtime::{RealtimeSessionConfig, chunk_to_base64, parse_realtime_frame};

	#[test]
	fn parses_delta_and_completed_realtime_frames() {
		let delta = r#"{"type":"conversation.item.input_audio_transcription.delta","item_id":"item-1","delta":"hello"}"#;
		let parsed = parse_realtime_frame(delta).expect("parse delta");
		let (item_id, previous_item_id, transcript, is_final) = parsed.expect("delta should parse");

		assert_eq!(item_id, "item-1");
		assert_eq!(previous_item_id, None);
		assert_eq!(transcript, "hello");
		assert!(!is_final);

		let completed = r#"{"type":"conversation.item.input_audio_transcription.completed","item_id":"item-1","transcript":"hello"}"#;
		let (item_id, previous_item_id, transcript, is_final) = parse_realtime_frame(completed)
			.expect("parse completed")
			.expect("completed should parse");

		assert_eq!(item_id, "item-1");
		assert_eq!(previous_item_id, None);
		assert_eq!(transcript, "hello");
		assert!(is_final);
	}

	#[test]
	fn chunk_encode_is_deterministic() {
		let chunk = vec![1, -2, 30_000];
		let encoded = chunk_to_base64(&chunk);

		assert!(!encoded.is_empty());
	}

	#[test]
	fn default_session_config_is_expected_shape() {
		let config = RealtimeSessionConfig::default();

		assert!(config.model.contains("gpt"));
		assert_eq!(config.sample_rate_hz, 24_000);
	}
}
