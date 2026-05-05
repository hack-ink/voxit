//! Voxit app entrypoint.

#[cfg(target_os = "macos")] mod hotkey_macos;
mod prelude {
	pub use color_eyre::eyre::{Result, eyre};
}

use std::{
	env, fs, panic,
	path::Path,
	process,
	sync::{
		Arc,
		atomic::{AtomicU8, Ordering},
		mpsc::{self, Receiver, Sender},
	},
	thread,
	time::{Duration, Instant},
};

use arboard::Clipboard;
use directories::ProjectDirs;
use eframe::{
	App, Frame,
	egui::{
		self, Button, CentralPanel, ComboBox, Context, ScrollArea, Ui, ViewportBuilder,
		ViewportCommand,
	},
};
#[cfg(target_os = "macos")] use enigo::{Direction, Enigo, Key, Keyboard, Settings};
#[cfg(target_os = "macos")] use global_hotkey::GlobalHotKeyManager;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
#[cfg(target_os = "macos")]
use tray_icon::{
	TrayIcon, TrayIconBuilder,
	menu::{
		Menu, MenuEvent, MenuItem, PredefinedMenuItem,
		accelerator::{Accelerator, Code, Modifiers},
	},
};

use crate::prelude::Result;
use voxit_audio::{InputDevice, Recorder};
use voxit_core::{
	auth::{self, AuthRecord, AuthStatus},
	config::Config,
	inference::{self, RewriteState},
	realtime::{self, RealtimeEvent, RealtimeSession},
	transcript::TranscriptAssembler,
};
use voxit_macos::{MicrophonePermissionState, PermissionSettingsPane, TargetApp};

#[cfg(target_os = "macos")]
const TRAY_MENU_ITEM_SHOW: &str = "show_window";
#[cfg(target_os = "macos")]
const TRAY_MENU_ITEM_QUIT: &str = "quit_voxit";
const PERMISSION_POLL_INTERVAL: Duration = Duration::from_millis(500);
const PERMISSION_POLL_TIMEOUT: Duration = Duration::from_secs(7);

#[derive(Clone, Copy, Debug)]
struct PermissionPoll {
	target: PermissionSettingsPane,
	next_check_at: Instant,
	deadline: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HotkeyMode {
	Toggle,
	Hold,
}
impl HotkeyMode {
	const fn as_u8(self) -> u8 {
		match self {
			Self::Toggle => 0,
			Self::Hold => 1,
		}
	}

	const fn from_u8(raw: u8) -> Self {
		match raw {
			1 => Self::Hold,
			_ => Self::Toggle,
		}
	}

	const fn as_label(self) -> &'static str {
		match self {
			Self::Toggle => "toggle",
			Self::Hold => "hold",
		}
	}
}

#[derive(Debug)]
enum AppCommand {
	ToggleRecording,
	StartRecording,
	StopRecording,
	ShowWindow,
	Quit,
}

#[derive(Debug)]
enum AuthEvent {
	SignedIn(AuthRecord),
	StatusChecked(AuthStatus),
	DeviceCodeInfo { user_code: String, verification_uri: String },
	Failed(String),
}

struct VoxitApp {
	config: Config,
	command_rx: Receiver<AppCommand>,
	auth_event_rx: Receiver<AuthEvent>,
	auth_event_tx: Sender<AuthEvent>,
	realtime_event_rx: Receiver<RealtimeEvent>,
	realtime_event_tx: Sender<RealtimeEvent>,
	inference_tx: Sender<inference::InferenceEvent>,
	inference_rx: Receiver<inference::InferenceEvent>,
	is_recording: bool,
	is_window_visible: bool,
	state: String,
	is_finalizing: bool,
	is_rewriting: bool,
	ignore_rewrite_result: bool,
	auth_status_refresh_started: bool,
	auth_status_checked_once: bool,
	status: String,
	auth_status: String,
	auth_signed_in: bool,
	auth_busy: bool,
	hotkey_mode: HotkeyMode,
	hotkey_mode_u8: Arc<AtomicU8>,
	stream_committed: String,
	stream_draft: String,
	transcript_assembler: TranscriptAssembler,
	transcription_result: String,
	rewritten_result: String,
	rewrite_enabled: bool,
	microphone_permission_state: MicrophonePermissionState,
	microphone_checked: bool,
	accessibility_checked: bool,
	permission_poll: Option<PermissionPoll>,
	audio_input_devices: Vec<InputDevice>,
	device_code_user_code: Option<String>,
	device_code_verification_uri: Option<String>,
	recording: Option<Recorder>,
	realtime_session: Option<RealtimeSession>,
	target_app: Option<TargetApp>,
	#[cfg(target_os = "macos")]
	_hotkey_manager: Option<GlobalHotKeyManager>,
	#[cfg(target_os = "macos")]
	_tray_icon: TrayIcon,
}
impl VoxitApp {
	#[cfg(not(target_os = "macos"))]
	#[allow(clippy::too_many_arguments)]
	fn new(
		config: Config,
		command_rx: Receiver<AppCommand>,
		auth_event_rx: Receiver<AuthEvent>,
		auth_event_tx: Sender<AuthEvent>,
		realtime_event_rx: Receiver<RealtimeEvent>,
		realtime_event_tx: Sender<RealtimeEvent>,
		inference_tx: Sender<inference::InferenceEvent>,
		inference_rx: Receiver<inference::InferenceEvent>,
		hotkey_mode_u8: Arc<AtomicU8>,
	) -> Self {
		let start_hidden = config.ui.start_hidden;
		let hotkey_mode = match config.hotkey.mode.as_str() {
			"hold" => HotkeyMode::Hold,
			_ => HotkeyMode::Toggle,
		};
		let rewrite_enabled = config.rewrite.enabled;
		let mut app = Self {
			config,
			auth_event_tx,
			auth_event_rx,
			realtime_event_rx,
			realtime_event_tx,
			inference_tx,
			inference_rx,
			command_rx,
			hotkey_mode_u8,
			is_recording: false,
			is_window_visible: !start_hidden,
			is_finalizing: false,
			is_rewriting: false,
			ignore_rewrite_result: false,
			auth_status_refresh_started: false,
			auth_status_checked_once: false,
			hotkey_mode,
			state: "Ready to listen.".to_string(),
			status: "No action yet.".to_string(),
			auth_status: "Checking auth...".to_string(),
			auth_signed_in: false,
			auth_busy: true,
			stream_committed: String::new(),
			stream_draft: String::new(),
			transcript_assembler: TranscriptAssembler::new(),
			transcription_result: String::new(),
			rewritten_result: String::new(),
			rewrite_enabled,
			microphone_permission_state: voxit_macos::microphone_permission_state(),
			microphone_checked: false,
			accessibility_checked: voxit_macos::permission_is_granted(
				PermissionSettingsPane::Accessibility,
			),
			permission_poll: None,
			audio_input_devices: Vec::new(),
			device_code_user_code: None,
			device_code_verification_uri: None,
			recording: None,
			realtime_session: None,
			target_app: None,
		};

		app.refresh_input_devices();
		app.refresh_permission_checks();
		app.refresh_auth_status_if_needed();

		app
	}

	fn handle_commands(&mut self, ctx: &Context) {
		self.handle_auth_events(ctx);
		self.handle_realtime_events();
		self.handle_inference_events();

		while let Ok(command) = self.command_rx.try_recv() {
			match command {
				AppCommand::ToggleRecording => self.toggle_recording(),
				AppCommand::StartRecording => self.start_recording(),
				AppCommand::StopRecording => self.stop_recording(),
				AppCommand::ShowWindow => self.show_window(ctx),
				AppCommand::Quit => self.quit_app(ctx),
			}
		}

		if self.is_window_visible && !self.auth_status_checked_once {
			self.refresh_auth_status_if_needed();
		}
	}

	fn set_hotkey_mode(&mut self, mode: HotkeyMode) {
		self.hotkey_mode = mode;
		self.config.hotkey.mode = mode.as_label().to_string();

		self.persist_config();
	}

	fn persist_config(&self) {
		if let Err(err) = self.config.save() {
			tracing::warn!(error = %err, "failed to persist config");
		}
	}

	fn configured_input_device_id(&self) -> Option<u32> {
		(self.config.audio.input_device_id != 0).then_some(self.config.audio.input_device_id)
	}

	fn sync_input_device_name(&mut self) {
		let Some(selected_id) = self.configured_input_device_id() else {
			self.config.audio.input_device_name.clear();

			return;
		};

		if let Some(device) =
			self.audio_input_devices.iter().find(|device| device.device_id == selected_id)
		{
			self.config.audio.input_device_name = device.name.clone();
		}
	}

	fn refresh_input_devices(&mut self) {
		match voxit_audio::list_input_devices() {
			Ok(devices) => {
				self.audio_input_devices = devices;

				self.sync_input_device_name();

				self.status = "Microphone list refreshed.".to_string();
			},
			Err(err) => {
				self.audio_input_devices.clear();

				tracing::warn!(error = %err, "failed to list input audio devices");

				if self.status == "No action yet." {
					self.status = "Microphone list unavailable in this environment.".to_string();
				}
			},
		}
	}

	fn selected_input_device_label(&self) -> String {
		let selected_id = self.config.audio.input_device_id;

		if selected_id == 0 {
			return "System default".to_string();
		}

		if let Some(device) =
			self.audio_input_devices.iter().find(|device| device.device_id == selected_id)
		{
			return format!("{} ({})", device.name, selected_id);
		}

		if self.config.audio.input_device_name.is_empty() {
			format!("Device #{selected_id}")
		} else {
			format!("{} ({})", self.config.audio.input_device_name, selected_id)
		}
	}

	fn refresh_auth_status_if_needed(&mut self) {
		if self.auth_status_refresh_started {
			return;
		}

		tracing::info!("auth status refresh starting");

		self.auth_status_refresh_started = true;
		self.auth_busy = true;
		self.auth_status = "Checking auth...".to_string();

		#[cfg(target_os = "macos")]
		let _ = voxit_macos::activate_current_application();
		let tx = self.auth_event_tx.clone();
		let _ = thread::spawn(move || {
			let status = panic::catch_unwind(auth::status).unwrap_or_else(|_| {
				tracing::error!("auth status check panicked");

				AuthStatus { signed_in: false, account_id: None }
			});
			let _ = tx.send(AuthEvent::StatusChecked(status));
		});
	}

	fn handle_auth_events(&mut self, ctx: &Context) {
		let mut did_update = false;

		while let Ok(event) = self.auth_event_rx.try_recv() {
			did_update = true;

			match event {
				AuthEvent::SignedIn(record) => {
					self.device_code_user_code = None;
					self.device_code_verification_uri = None;
					self.auth_busy = false;
					self.auth_signed_in = true;
					self.auth_status = record
						.account_id
						.map_or_else(|| "Signed in".to_string(), |id| format!("Signed in: {id}"));
				},
				AuthEvent::StatusChecked(status) => {
					tracing::info!(
						signed_in = status.signed_in,
						has_account_id = status.account_id.is_some(),
						"auth status refresh completed"
					);

					self.auth_status_refresh_started = false;
					self.auth_status_checked_once = true;
					self.auth_busy = false;
					self.auth_signed_in = status.signed_in;
					self.auth_status = if status.signed_in {
						status.account_id.map_or_else(
							|| "Signed in".to_string(),
							|id| format!("Signed in: {id}"),
						)
					} else {
						"Not signed in".to_string()
					};
				},
				AuthEvent::DeviceCodeInfo { user_code, verification_uri } => {
					self.device_code_user_code = Some(user_code);
					self.device_code_verification_uri = Some(verification_uri);
					self.auth_busy = true;
					self.status =
						"Device code shown in panel. Open the URL and enter the code.".to_string();
				},
				AuthEvent::Failed(err) => {
					self.device_code_user_code = None;
					self.device_code_verification_uri = None;
					self.auth_busy = false;
					self.status = format!("Auth failed: {err}");
				},
			}
		}

		if did_update {
			ctx.request_repaint();
		}
	}

	fn refresh_permission_checks(&mut self) {
		self.microphone_permission_state = voxit_macos::microphone_permission_state();
		self.microphone_checked =
			voxit_macos::permission_is_granted(PermissionSettingsPane::Microphone);
		self.accessibility_checked =
			voxit_macos::permission_is_granted(PermissionSettingsPane::Accessibility);
	}

	fn should_poll_permission(&self, target: PermissionSettingsPane) -> bool {
		match target {
			PermissionSettingsPane::Microphone => !matches!(
				self.microphone_permission_state,
				MicrophonePermissionState::Granted
					| MicrophonePermissionState::Denied
					| MicrophonePermissionState::Restricted
			),
			PermissionSettingsPane::Accessibility => !self.accessibility_checked,
		}
	}

	fn start_permission_poll_if_needed(&mut self, target: PermissionSettingsPane) {
		if self.should_poll_permission(target) {
			let now = Instant::now();

			self.permission_poll = Some(PermissionPoll {
				target,
				next_check_at: now,
				deadline: now + PERMISSION_POLL_TIMEOUT,
			});
		} else {
			self.permission_poll = None;
		}
	}

	fn stop_permission_poll(&mut self) {
		self.permission_poll = None;
	}

	fn is_permission_polling_done(&self, target: PermissionSettingsPane) -> bool {
		match target {
			PermissionSettingsPane::Microphone => {
				matches!(
					self.microphone_permission_state,
					MicrophonePermissionState::Granted
						| MicrophonePermissionState::Denied
						| MicrophonePermissionState::Restricted
				)
			},
			PermissionSettingsPane::Accessibility => self.accessibility_checked,
		}
	}

	fn handle_permission_poll(&mut self, ctx: &Context) {
		let Some(poll) = self.permission_poll else {
			return;
		};
		let now = Instant::now();

		if now >= poll.deadline {
			self.stop_permission_poll();

			return;
		}
		if now < poll.next_check_at {
			ctx.request_repaint_after(poll.next_check_at - now);

			return;
		}

		self.refresh_permission_checks();

		if self.is_permission_polling_done(poll.target) {
			self.stop_permission_poll();
		} else {
			self.permission_poll = Some(PermissionPoll {
				target: poll.target,
				next_check_at: now + PERMISSION_POLL_INTERVAL,
				deadline: poll.deadline,
			});

			ctx.request_repaint_after(PERMISSION_POLL_INTERVAL);
		}
	}

	fn microphone_status_text(&self) -> &'static str {
		match self.microphone_permission_state {
			MicrophonePermissionState::Granted => "granted",
			MicrophonePermissionState::NotDetermined => "not requested",
			MicrophonePermissionState::Denied => "denied",
			MicrophonePermissionState::Restricted => "restricted",
			MicrophonePermissionState::Prompted => "prompted",
			MicrophonePermissionState::Unknown => "unknown",
		}
	}

	fn start_sign_in_with_chatgpt(&mut self) {
		if self.auth_busy {
			return;
		}

		self.auth_busy = true;
		self.device_code_user_code = None;
		self.device_code_verification_uri = None;
		self.status = "Starting ChatGPT device code login...".to_string();

		let tx = self.auth_event_tx.clone();
		#[cfg(target_os = "macos")]
		let _ = voxit_macos::activate_current_application();

		thread::spawn(move || {
			let event = match auth::sign_in_with_chatgpt(|user_code, verification_uri| {
				let _ = tx.send(AuthEvent::DeviceCodeInfo {
					user_code: user_code.to_string(),
					verification_uri: verification_uri.to_string(),
				});
			}) {
				Ok(record) => AuthEvent::SignedIn(record),
				Err(err) => AuthEvent::Failed(err),
			};
			let _ = tx.send(event);
		});
	}

	fn sign_out(&mut self) {
		self.device_code_user_code = None;
		self.device_code_verification_uri = None;
		self.auth_status = match auth::sign_out() {
			Ok(()) => {
				self.auth_signed_in = false;

				"Not signed in".to_string()
			},
			Err(err) => {
				self.status = format!("Sign out failed: {err}");

				self.auth_status.clone()
			},
		};
	}

	fn handle_realtime_events(&mut self) {
		while let Ok(event) = self.realtime_event_rx.try_recv() {
			match event {
				realtime::RealtimeEvent::Draft(transcript)
				| realtime::RealtimeEvent::Committed(transcript) => {
					self.transcript_assembler.apply(transcript);

					let snapshot = self.transcript_assembler.snapshot();

					self.stream_committed = snapshot.committed;
					self.stream_draft = snapshot.draft;
				},
				realtime::RealtimeEvent::StreamError(err) => {
					self.status = format!("Realtime stream degraded: {err}");
				},
			}
		}
	}

	fn handle_inference_events(&mut self) {
		while let Ok(event) = self.inference_rx.try_recv() {
			match event {
				inference::InferenceEvent::Pass2Completed { total_ms, raw_transcript } => {
					self.is_finalizing = false;
					self.transcription_result = raw_transcript;
					self.state = "FinalizingPass2".to_string();
					self.status = format!(
						"Pass2 completed in {total_ms} ms ({} chars).",
						self.transcription_result.len()
					);

					if self.rewrite_enabled && self.config.rewrite.auto {
						self.start_rewrite_pass();
					} else {
						let text = self.transcription_result.clone();
						let paste_status = self.paste_transcript(&text);

						self.state = "Done".to_string();
						self.status = format!("Pass2 completed in {total_ms} ms. {paste_status}");
					}
				},
				inference::InferenceEvent::RewriteCompleted { total_ms, result } => {
					self.is_rewriting = false;

					if self.ignore_rewrite_result {
						self.status = format!(
							"Rewrite finished in {total_ms} ms but was ignored because raw transcript was already pasted."
						);

						continue;
					}

					match result.state {
						RewriteState::Applied => {
							self.rewritten_result = result.rewritten_transcript.unwrap_or_default();

							let final_text = if self.rewritten_result.is_empty() {
								self.transcription_result.clone()
							} else {
								self.rewritten_result.clone()
							};
							let paste_status = self.paste_transcript(&final_text);

							self.state = "Done".to_string();
							self.status = format!(
								"Pass3 completed in {total_ms} ms. {paste_status} (raw {} chars, rewritten {} chars)",
								self.transcription_result.len(),
								self.rewritten_result.len()
							);
						},
						RewriteState::Rejected | RewriteState::Skipped => {
							let fallback_text = self.transcription_result.clone();
							let paste_status = self.paste_transcript(&fallback_text);

							self.state = "Done".to_string();
							self.status = format!(
								"Pass3 skipped/rejected in {total_ms} ms. {} {}",
								result.reason.unwrap_or_default(),
								paste_status
							);
						},
					}
				},
				inference::InferenceEvent::Failed(err) => {
					self.is_finalizing = false;
					self.is_rewriting = false;
					self.status = format!("OpenAI failed: {err}");
				},
			}
		}
	}

	fn toggle_recording(&mut self) {
		if self.is_recording {
			self.stop_recording();
		} else {
			self.start_recording();
		}
	}

	fn start_recording(&mut self) {
		if self.is_recording {
			return;
		}

		self.refresh_permission_checks();

		if !self.microphone_checked {
			self.status =
				"Microphone permission not granted. Open Preferences and request it first."
					.to_string();

			return;
		}

		self.transcript_assembler.reset();
		self.stream_committed.clear();
		self.stream_draft.clear();
		self.transcription_result.clear();
		self.rewritten_result.clear();

		self.ignore_rewrite_result = false;
		self.target_app = if self.config.paste.lock_frontmost_app {
			voxit_macos::capture_frontmost_app()
		} else {
			None
		};

		match voxit_audio::start_recording_with_stream(64, self.configured_input_device_id()) {
			Ok((recorder, chunk_rx, device_selection)) => {
				self.recording = Some(recorder);

				let used_fallback = device_selection.fallback_to_default
					&& device_selection.requested_device_id.is_some();
				let fallback_prefix = if used_fallback {
					format!(
						"Selected microphone unavailable. Falling back to default: {}.",
						device_selection.selected_device_name
					)
				} else {
					String::new()
				};

				if used_fallback {
					tracing::warn!(
						requested_device_id = device_selection.requested_device_id,
						selected_device_id = device_selection.selected_device_id,
						selected_device_name = device_selection.selected_device_name.as_str(),
						"input microphone unavailable; fell back to system default"
					);
				}

				let realtime_config = realtime::RealtimeSessionConfig {
					model: self.config.openai.realtime_model.clone(),
					sample_rate_hz: self.config.audio.realtime_target_rate_hz,
					noise_reduction: self.config.openai.realtime.noise_reduction.clone(),
				};
				let realtime_status = match inference::start_realtime_session(
					realtime_config,
					chunk_rx,
					self.realtime_event_tx.clone(),
				) {
					Ok(session) => {
						self.realtime_session = Some(session);

						None
					},
					Err(err) => Some(format!(
						"Realtime unavailable ({err}). Recording continues with Pass2 finalize."
					)),
				};

				self.is_recording = true;
				self.is_finalizing = false;
				self.is_rewriting = false;
				self.state = "Listening".to_string();

				let mut started_status =
					"Recording started. Pass1 streaming will appear in the panel.".to_string();

				if used_fallback {
					started_status = format!("{fallback_prefix} {started_status}");
				}

				if let Some(realtime_status) = realtime_status {
					started_status = format!("{started_status} {realtime_status}");
				}

				self.status = started_status;
			},
			Err(err) => {
				self.is_recording = false;
				self.status = format!("Failed to start recording: {err}");

				tracing::error!(error = %err, "start recording failed");
			},
		}
	}

	fn stop_recording(&mut self) {
		if !self.is_recording {
			self.state = "Idle".to_string();
			self.status = "Not recording. Press start to capture again.".to_string();

			return;
		}

		self.is_recording = false;

		if let Some(session) = self.realtime_session.take() {
			session.stop();
		}

		#[cfg(target_os = "macos")]
		{
			let Some(recorder) = self.recording.take() else {
				self.status = "No active recording session.".to_string();

				return;
			};
			let (frames_captured, wav) = match voxit_audio::stop_recording(recorder) {
				Ok(result) => {
					self.state = "Stopped".to_string();
					self.status = format!(
						"Captured {} frames in {} ms at {}Hz, {} ch.",
						result.frames, result.duration_ms, result.sample_rate, result.channels
					);

					tracing::info!(
						duration_ms = result.duration_ms,
						frames = result.frames,
						sample_rate = result.sample_rate,
						channels = result.channels,
						wav_bytes = result.wav.len(),
						"Recorded audio"
					);

					(result.frames, result.wav)
				},
				Err(err) => {
					let err_msg = err.to_string();

					self.status = format!("Failed to stop recording: {err_msg}");

					return;
				},
			};
			let is_empty_capture = frames_captured == 0 || wav.len() <= 44 || wav.is_empty();

			if is_empty_capture {
				self.status = "No valid audio captured; skipping transcription.".to_string();

				return;
			}

			let tx = self.inference_tx.clone();
			let model = self.config.openai.finalize_model.clone();

			self.is_finalizing = true;
			self.status = "Finalizing Pass2 transcript...".to_string();
			thread::spawn(move || {
				let outcome = inference::transcribe_only(&wav, &model)
					.map(|(raw_transcript, total_ms)| inference::InferenceEvent::Pass2Completed {
						total_ms,
						raw_transcript,
					})
					.unwrap_or_else(inference::InferenceEvent::Failed);
				let _ = tx.send(outcome);
			});
		}
		#[cfg(not(target_os = "macos"))]
		{
			self.status = "This build does not support recording.".to_string();
		}
	}

	fn start_rewrite_pass(&mut self) {
		if self.transcription_result.is_empty() {
			return;
		}

		self.is_rewriting = true;
		self.state = "RewritingPass3".to_string();
		self.status = "Running Pass3 rewrite...".to_string();

		let tx = self.inference_tx.clone();
		let model = self.config.openai.rewrite_model.clone();
		let raw = self.transcription_result.clone();

		thread::spawn(move || {
			let outcome = inference::rewrite_only(&raw, &model)
				.map(|(result, total_ms)| inference::InferenceEvent::RewriteCompleted {
					total_ms,
					result,
				})
				.unwrap_or_else(inference::InferenceEvent::Failed);
			let _ = tx.send(outcome);
		});
	}

	fn paste_transcript(&mut self, text: &str) -> String {
		if text.is_empty() {
			return "No text to paste.".to_string();
		}
		if self.config.paste.lock_frontmost_app
			&& let Some(target_app) = self.target_app.as_ref()
		{
			let activated = voxit_macos::activate_target(target_app, 4, Duration::from_millis(80));

			if !activated {
				tracing::warn!("failed to re-activate captured frontmost app before paste");
			}
		}

		match paste_text(text) {
			Ok(()) => {
				self.accessibility_checked = true;

				"Pasted transcript into target app.".to_string()
			},
			Err(err) => format!("Paste failed: {err}"),
		}
	}

	fn request_permission(&mut self, pane: PermissionSettingsPane) {
		tracing::info!(
			?pane,
			microphone_checked = self.microphone_checked,
			accessibility_checked = self.accessibility_checked,
			"requesting macOS permission"
		);

		let pane_name = pane.display_name();

		match pane {
			PermissionSettingsPane::Microphone => {
				let mic_state = voxit_macos::request_microphone_permission();

				self.refresh_permission_checks();

				self.microphone_permission_state = voxit_macos::microphone_permission_state();

				if self.microphone_permission_state == MicrophonePermissionState::Unknown {
					self.microphone_permission_state = mic_state;
				}

				self.status = match mic_state {
					MicrophonePermissionState::Granted => {
						format!("{pane_name} permission granted.")
					},
					MicrophonePermissionState::Prompted => format!(
						"{pane_name} permission prompt shown. Approve it in macOS, then reopen Preferences."
					),
					MicrophonePermissionState::NotDetermined => format!(
						"{pane_name} permission still not requested. Please respond to system prompt."
					),
					MicrophonePermissionState::Denied => format!(
						"{pane_name} permission denied. Open System Settings > Privacy & Security and enable it."
					),
					MicrophonePermissionState::Restricted => {
						format!("{pane_name} permission is restricted by system policy.")
					},
					MicrophonePermissionState::Unknown => {
						format!("{pane_name} permission status is unknown.")
					},
				};

				self.start_permission_poll_if_needed(PermissionSettingsPane::Microphone);
			},
			PermissionSettingsPane::Accessibility => {
				let granted = if self.accessibility_checked {
					true
				} else {
					voxit_macos::request_permission(PermissionSettingsPane::Accessibility)
				};

				self.accessibility_checked = granted;

				self.refresh_permission_checks();

				self.status = if granted {
					format!("{pane_name} permission granted.")
				} else {
					format!("{pane_name} permission not granted. Enable it in System Settings.")
				};

				self.start_permission_poll_if_needed(PermissionSettingsPane::Accessibility);
			},
		}
	}

	fn render_auth_section(&mut self, ui: &mut Ui) {
		ui.heading("Voxit");
		ui.label("macOS tray, global hotkey, Pass1 stream, Pass2 finalize, Pass3 guarded rewrite.");
		ui.separator();

		ui.horizontal(|ui| {
			ui.label(format!("Auth: {}", self.auth_status));

			if let (Some(code), Some(uri)) = (
				self.device_code_user_code.as_deref(),
				self.device_code_verification_uri.as_deref(),
			) {
				ui.separator();
				ui.label(format!("Device code: {code}"));
				ui.label(format!("Verify at: {uri}"));
			}

			if self.auth_busy {
				ui.separator();
				ui.label("Authenticating...");
			}
		});
		ui.horizontal_wrapped(|ui| {
			let can_auth = !self.auth_busy;

			if self.auth_signed_in {
				if ui.add_enabled(can_auth, Button::new("Sign out")).clicked() {
					self.sign_out();
				}
			} else {
				if ui.add_enabled(can_auth, Button::new("Sign in with ChatGPT")).clicked() {
					self.start_sign_in_with_chatgpt();
				}
			}
		});
	}

	fn render_runtime_controls(&mut self, ui: &mut Ui) {
		ui.separator();
		ui.horizontal(|ui| {
			ui.label(format!("Status: {}", self.state));
			ui.separator();
			ui.label(format!("Mode: {}", self.hotkey_mode.as_label()));
		});
		ui.separator();
		self.render_permissions(ui);
		ui.separator();
		self.render_microphone_input(ui);
		self.render_dictation_controls(ui);
		ui.separator();
		self.render_hotkey_controls(ui);
	}

	fn render_permissions(&mut self, ui: &mut Ui) {
		ui.label("Permissions:");
		ui.label(format!("Microphone: {}", self.microphone_status_text()));

		if !self.microphone_checked && ui.button("Request Microphone permission").clicked() {
			self.request_permission(PermissionSettingsPane::Microphone);
		}

		ui.label(format!(
			"Accessibility (Cmd+V): {}",
			if self.accessibility_checked { "granted" } else { "missing" }
		));

		if !self.accessibility_checked && ui.button("Request Accessibility permission").clicked() {
			self.request_permission(PermissionSettingsPane::Accessibility);
		}
	}

	fn render_microphone_input(&mut self, ui: &mut Ui) {
		ui.label("Microphone input:");

		ui.horizontal(|ui| {
			if ui.button("Refresh microphones").clicked() {
				self.refresh_input_devices();
			}

			let mut selected_device_id = self.config.audio.input_device_id;
			let mut changed = false;

			ComboBox::from_label("Input device")
				.selected_text(self.selected_input_device_label())
				.show_ui(ui, |ui| {
					if ui.selectable_value(&mut selected_device_id, 0, "System default").clicked() {
						changed = true;
					}

					for device in &self.audio_input_devices {
						if ui
							.selectable_value(
								&mut selected_device_id,
								device.device_id,
								format!("{} ({})", device.name, device.device_id),
							)
							.clicked()
						{
							changed = true;
						}
					}
				});

			if changed {
				self.config.audio.input_device_id = selected_device_id;
				self.config.audio.input_device_name = if selected_device_id == 0 {
					String::new()
				} else {
					self.audio_input_devices
						.iter()
						.find(|device| device.device_id == selected_device_id)
						.map(|device| device.name.clone())
						.unwrap_or_else(|| format!("Device #{selected_device_id}"))
				};

				self.persist_config();
			}
		});
	}

	fn render_dictation_controls(&mut self, ui: &mut Ui) {
		let button_text = if self.is_recording { "Stop Dictation" } else { "Start Dictation" };

		if ui.button(button_text).clicked() {
			self.toggle_recording();
		}
		if self.is_finalizing {
			ui.label("Pass2 finalizing in progress...");
		}
		if self.is_rewriting {
			ui.label("Pass3 rewrite in progress...");
		}
		if (self.is_finalizing || self.is_rewriting)
			&& !self.transcription_result.is_empty()
			&& ui.button("Paste raw now (skip rewrite)").clicked()
		{
			self.ignore_rewrite_result = true;
			self.is_finalizing = false;
			self.is_rewriting = false;

			let fallback = self.transcription_result.clone();
			let paste_status = self.paste_transcript(&fallback);

			self.state = "Done".to_string();
			self.status = format!("Raw transcript pasted. {paste_status}");
		}
	}

	fn render_hotkey_controls(&mut self, ui: &mut Ui) {
		ui.horizontal_wrapped(|ui| {
			ui.label(format!("Hotkey mode: {}", self.hotkey_mode.as_label()));
			ui.separator();

			let original = self.rewrite_enabled;

			ui.checkbox(&mut self.rewrite_enabled, "Auto rewrite after Pass2");

			if original != self.rewrite_enabled {
				self.config.rewrite.enabled = self.rewrite_enabled;

				self.persist_config();
			}
			if ui.selectable_label(self.hotkey_mode == HotkeyMode::Toggle, "toggle").clicked() {
				self.set_hotkey_mode(HotkeyMode::Toggle);
			}
			if ui.selectable_label(self.hotkey_mode == HotkeyMode::Hold, "hold").clicked() {
				self.set_hotkey_mode(HotkeyMode::Hold);
			}
		});
	}

	fn render_output_section(&mut self, ui: &mut Ui) {
		ui.separator();
		ui.label("Pass1 Stream (Committed):");
		ui.label(self.stream_committed.as_str());

		if !self.stream_draft.is_empty() {
			ui.label("Pass1 Draft:");
			ui.weak(self.stream_draft.as_str());
		}

		ui.separator();
		ui.label("Pass2 Raw Transcript:");
		ui.label(self.transcription_result.as_str());

		if ui.button("Test Paste").clicked() {
			self.accessibility_checked = true;

			let paste_status = self.paste_transcript("Voxit paste test");

			self.status = format!("Test paste: {paste_status}");
		}

		ui.separator();
		ui.label(format!("Latest: {}", self.status));

		if !self.rewritten_result.is_empty() {
			ui.separator();
			ui.label("Pass3 Rewritten:");
			ui.label(self.rewritten_result.as_str());
		}
	}

	fn show_window(&mut self, ctx: &Context) {
		self.refresh_permission_checks();

		self.is_window_visible = true;

		ctx.send_viewport_cmd(ViewportCommand::Visible(true));
		ctx.send_viewport_cmd(ViewportCommand::Focus);

		self.auth_status_refresh_started = false;
		self.auth_status_checked_once = false;
		self.auth_busy = true;
		self.auth_status = "Checking auth...".to_string();
	}

	fn quit_app(&mut self, ctx: &Context) {
		ctx.send_viewport_cmd(ViewportCommand::Close);
	}
}

#[cfg(target_os = "macos")]
impl VoxitApp {
	#[allow(clippy::too_many_arguments)]
	fn new(
		config: Config,
		command_rx: Receiver<AppCommand>,
		auth_event_rx: Receiver<AuthEvent>,
		auth_event_tx: Sender<AuthEvent>,
		realtime_event_rx: Receiver<RealtimeEvent>,
		realtime_event_tx: Sender<RealtimeEvent>,
		inference_tx: Sender<inference::InferenceEvent>,
		inference_rx: Receiver<inference::InferenceEvent>,
		hotkey_mode_u8: Arc<AtomicU8>,
		_hotkey_manager: Option<GlobalHotKeyManager>,
		_tray_icon: TrayIcon,
	) -> Self {
		let start_hidden = config.ui.start_hidden;
		let hotkey_mode = match config.hotkey.mode.as_str() {
			"hold" => HotkeyMode::Hold,
			_ => HotkeyMode::Toggle,
		};
		let rewrite_enabled = config.rewrite.enabled;
		let mut app = Self {
			config,
			auth_event_tx,
			auth_event_rx,
			realtime_event_rx,
			realtime_event_tx,
			inference_tx,
			inference_rx,
			hotkey_mode_u8,
			is_recording: false,
			is_window_visible: !start_hidden,
			is_finalizing: false,
			is_rewriting: false,
			ignore_rewrite_result: false,
			auth_status_refresh_started: false,
			auth_status_checked_once: false,
			state: "Ready to listen.".to_string(),
			status: "No action yet.".to_string(),
			auth_status: "Checking auth...".to_string(),
			auth_signed_in: false,
			auth_busy: true,
			stream_committed: String::new(),
			stream_draft: String::new(),
			transcript_assembler: TranscriptAssembler::new(),
			transcription_result: String::new(),
			rewritten_result: String::new(),
			rewrite_enabled,
			microphone_permission_state: voxit_macos::microphone_permission_state(),
			microphone_checked: false,
			accessibility_checked: voxit_macos::permission_is_granted(
				PermissionSettingsPane::Accessibility,
			),
			permission_poll: None,
			audio_input_devices: Vec::new(),
			device_code_user_code: None,
			device_code_verification_uri: None,
			command_rx,
			hotkey_mode,
			recording: None,
			realtime_session: None,
			target_app: None,
			_hotkey_manager,
			_tray_icon,
		};

		app.refresh_input_devices();
		app.refresh_permission_checks();
		app.refresh_auth_status_if_needed();

		app
	}
}

impl App for VoxitApp {
	fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
		self.handle_commands(ctx);
		self.handle_permission_poll(ctx);
		#[cfg(target_os = "macos")]
		self.hotkey_mode_u8.store(self.hotkey_mode.as_u8(), Ordering::Release);

		CentralPanel::default().show(ctx, |ui| {
			ScrollArea::vertical().show(ui, |ui| {
				self.render_auth_section(ui);
				self.render_runtime_controls(ui);
				self.render_output_section(ui);
			});
		});

		if self.is_window_visible && (self.auth_busy || self.is_finalizing || self.is_rewriting) {
			ctx.request_repaint_after(Duration::from_millis(100));
		}
	}
}

fn ensure_app_data_dir(app_root: &Path) -> Result<()> {
	fs::create_dir_all(app_root).map_err(|err| {
		crate::prelude::eyre!("Failed to create app data directory {}: {err}", app_root.display())
	})?;

	Ok(())
}

fn main() -> Result<()> {
	color_eyre::install()?;

	let project_dirs = ProjectDirs::from("", "hack.ink", "voxit")
		.ok_or_else(|| crate::prelude::eyre!("Failed to resolve project directories."))?;
	let app_root = project_dirs.data_dir();

	ensure_app_data_dir(app_root)?;

	let (non_blocking, _guard) = tracing_appender::non_blocking(
		RollingFileAppender::builder()
			.rotation(Rotation::WEEKLY)
			.max_log_files(3)
			.filename_suffix("log")
			.build(app_root)?,
	);
	let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

	tracing_subscriber::fmt()
		.with_env_filter(filter)
		.with_ansi(false)
		.with_writer(non_blocking)
		.init();

	let default_hook = panic::take_hook();
	let abort_on_panic = env::var("VOXIT_ABORT_ON_PANIC").is_ok_and(|v| v == "1");

	panic::set_hook(Box::new(move |p| {
		default_hook(p);

		if abort_on_panic {
			process::abort();
		}
	}));

	let outcome = run_ui();

	if let Err(err) = outcome.as_ref() {
		tracing::error!(error = %err, "ui startup failed");
	}

	outcome
}

#[cfg(target_os = "macos")]
fn create_tray_icon() -> Result<TrayIcon> {
	let icon = build_tray_icon();
	let menu = create_tray_menu()?;
	let tray_icon = TrayIconBuilder::new()
		.with_tooltip("Voxit")
		.with_icon(icon)
		.with_menu(Box::new(menu))
		.build()?;

	Ok(tray_icon)
}

#[cfg(target_os = "macos")]
fn create_tray_menu() -> Result<Menu> {
	let menu = Menu::new();
	let show_item = MenuItem::with_id(
		TRAY_MENU_ITEM_SHOW,
		"Preferences…",
		true,
		Some(Accelerator::new(Some(Modifiers::META), Code::Comma)),
	);
	let quit_item = MenuItem::with_id(
		TRAY_MENU_ITEM_QUIT,
		"Quit Voxit",
		true,
		Some(Accelerator::new(Some(Modifiers::META), Code::KeyQ)),
	);

	menu.append(&show_item)?;
	menu.append(&PredefinedMenuItem::separator())?;
	menu.append(&quit_item)?;

	Ok(menu)
}

#[cfg(target_os = "macos")]
fn build_tray_icon() -> tray_icon::Icon {
	let mut pixels = vec![0_u8; 16 * 16 * 4];

	for (idx, chunk) in pixels.chunks_exact_mut(4).enumerate() {
		let x = (idx % 16) as u8;
		let y = ((idx / 16) % 16) as u8;
		let band = if y.is_multiple_of(2) { 0x12 } else { 0x34 };

		chunk[0] = 0x66;
		chunk[1] = 0x88 + band;
		chunk[2] = 0xCC;
		chunk[3] = 0xFF;
		chunk[0] = chunk[0].saturating_add(x);
		chunk[1] = chunk[1].saturating_add(y);
	}

	tray_icon::Icon::from_rgba(pixels, 16, 16).expect("tray icon pixels must be valid RGBA")
}

#[cfg(target_os = "macos")]
fn spawn_tray_listener(command_tx: Sender<AppCommand>) {
	let receiver = MenuEvent::receiver().clone();

	thread::spawn(move || {
		while let Ok(event) = receiver.recv() {
			match event.id.as_ref() {
				TRAY_MENU_ITEM_SHOW => {
					let _ = command_tx.send(AppCommand::ShowWindow);
				},
				TRAY_MENU_ITEM_QUIT => {
					let _ = command_tx.send(AppCommand::Quit);
				},
				_ => {},
			}
		}
	});
}

#[cfg(target_os = "macos")]
fn spawn_global_hotkey_listener(
	command_tx: Sender<AppCommand>,
	mode: Arc<AtomicU8>,
) -> Result<GlobalHotKeyManager> {
	hotkey_macos::spawn_global_hotkey_listener(command_tx, mode)
}

#[cfg(target_os = "macos")]
fn run_ui() -> Result<()> {
	let app_config = Config::load().unwrap_or_else(|err| {
		tracing::warn!(error = %err, "failed to load config; using defaults");

		Config::default()
	});
	let (command_tx, command_rx) = mpsc::channel::<AppCommand>();
	let (auth_event_tx, auth_event_rx) = mpsc::channel::<AuthEvent>();
	let (realtime_event_tx, realtime_event_rx) = mpsc::channel::<RealtimeEvent>();
	let (inference_tx, inference_rx) = mpsc::channel::<inference::InferenceEvent>();
	let initial_hotkey =
		if app_config.hotkey.mode == "hold" { HotkeyMode::Hold } else { HotkeyMode::Toggle };
	let hotkey_mode = Arc::new(AtomicU8::new(initial_hotkey.as_u8()));
	let tray_icon = create_tray_icon()?;

	spawn_tray_listener(command_tx.clone());

	let hotkey_manager =
		match spawn_global_hotkey_listener(command_tx.clone(), Arc::clone(&hotkey_mode)) {
			Ok(manager) => Some(manager),
			Err(err) => {
				tracing::warn!(error = %err, "global hotkey disabled (permission missing?)");

				None
			},
		};
	let mut app = VoxitApp::new(
		app_config.clone(),
		command_rx,
		auth_event_rx,
		auth_event_tx,
		realtime_event_rx,
		realtime_event_tx,
		inference_tx,
		inference_rx,
		Arc::clone(&hotkey_mode),
		hotkey_manager,
		tray_icon,
	);

	if app._hotkey_manager.is_none() {
		app.status =
			"Global hotkey unavailable. Ensure Accessibility is granted, then restart Voxit."
				.to_string();
	}

	let options = eframe::NativeOptions {
		viewport: ViewportBuilder::default()
			.with_inner_size(egui::vec2(
				app_config.ui.panel_width_px as f32,
				app_config.ui.panel_height_px as f32,
			))
			.with_visible(!app_config.ui.start_hidden),
		..Default::default()
	};

	tracing::info!(
		pid = process::id(),
		start_hidden = app_config.ui.start_hidden,
		panel_width_px = app_config.ui.panel_width_px,
		panel_height_px = app_config.ui.panel_height_px,
		"starting voxit ui"
	);

	if let Err(err) = eframe::run_native("Voxit", options, Box::new(|_cc| Ok(Box::new(app)))) {
		return Err(crate::prelude::eyre!(format!("eframe startup failed: {err}")));
	}

	Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run_ui() -> Result<()> {
	let app_config = Config::load().unwrap_or_else(|err| {
		tracing::warn!(error = %err, "failed to load config; using defaults");

		Config::default()
	});
	let (_command_tx, command_rx) = mpsc::channel::<AppCommand>();
	let (auth_event_tx, auth_event_rx) = mpsc::channel::<AuthEvent>();
	let (realtime_event_tx, realtime_event_rx) = mpsc::channel::<RealtimeEvent>();
	let (inference_tx, inference_rx) = mpsc::channel::<inference::InferenceEvent>();
	let initial_hotkey =
		if app_config.hotkey.mode == "hold" { HotkeyMode::Hold } else { HotkeyMode::Toggle };
	let hotkey_mode = Arc::new(AtomicU8::new(initial_hotkey.as_u8()));
	let app = VoxitApp::new(
		app_config.clone(),
		command_rx,
		auth_event_rx,
		auth_event_tx,
		realtime_event_rx,
		realtime_event_tx,
		inference_tx,
		inference_rx,
		Arc::clone(&hotkey_mode),
	);
	let options = eframe::NativeOptions {
		viewport: ViewportBuilder::default()
			.with_inner_size(egui::vec2(
				app_config.ui.panel_width_px as f32,
				app_config.ui.panel_height_px as f32,
			))
			.with_visible(!app_config.ui.start_hidden),
		..Default::default()
	};

	tracing::info!(
		pid = process::id(),
		start_hidden = app_config.ui.start_hidden,
		panel_width_px = app_config.ui.panel_width_px,
		panel_height_px = app_config.ui.panel_height_px,
		"starting voxit ui"
	);

	if let Err(err) = eframe::run_native("Voxit", options, Box::new(|_cc| Ok(Box::new(app)))) {
		return Err(crate::prelude::eyre!(format!("eframe startup failed: {err}")));
	}

	Ok(())
}

#[cfg(target_os = "macos")]
fn paste_text(text: &str) -> Result<(), String> {
	let mut clipboard = Clipboard::new().map_err(|err| err.to_string())?;

	clipboard.set_text(text.to_string()).map_err(|err| err.to_string())?;

	simulate_cmd_v().map_err(|err| format!("clipboard set, but paste failed: {err}"))?;

	Ok(())
}

#[cfg(target_os = "macos")]
fn simulate_cmd_v() -> Result<(), String> {
	let mut enigo = Enigo::new(&Settings::default()).map_err(|err| err.to_string())?;

	enigo.key(Key::Meta, Direction::Press).map_err(|err| err.to_string())?;

	thread::sleep(Duration::from_millis(10));

	enigo.key(Key::Unicode('v'), Direction::Click).map_err(|err| err.to_string())?;

	thread::sleep(Duration::from_millis(10));

	enigo.key(Key::Meta, Direction::Release).map_err(|err| err.to_string())?;

	Ok(())
}

#[cfg(not(target_os = "macos"))]
fn paste_text(_text: &str) -> Result<(), String> {
	Ok(())
}

#[cfg(test)]
mod tests {
	use std::{
		env, fs,
		time::{SystemTime, UNIX_EPOCH},
	};

	#[test]
	fn ensure_app_data_dir_creates_missing_directory() {
		let nonce =
			SystemTime::now().duration_since(UNIX_EPOCH).expect("time went backwards").as_nanos();
		let base = env::temp_dir().join(format!("voxit-test-{nonce}"));
		let app_root = base.join("hack.ink.voxit");

		if base.exists() {
			let _ = fs::remove_dir_all(&base);
		}

		assert!(!app_root.exists());

		crate::ensure_app_data_dir(&app_root).unwrap();

		assert!(app_root.is_dir());

		let _ = fs::remove_dir_all(&base);
	}
}
