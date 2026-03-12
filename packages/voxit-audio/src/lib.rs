//! Audio capture primitives for Voxit.

use std::{
	io::Cursor,
	sync::{
		Arc, Mutex,
		atomic::AtomicU64,
		mpsc,
		mpsc::{Receiver, SyncSender},
	},
	time::Instant,
};

#[cfg(target_os = "macos")]
use coreaudio::audio_unit::{
	AudioUnit, Element, IOType, Scope, StreamFormat,
	audio_format::LinearPcmFlags,
	macos_helpers,
	render_callback::{Args, data::Interleaved},
};
use coreaudio::error::Error;
#[cfg(target_os = "macos")]
use objc2_audio_toolbox::{
	kAudioOutputUnitProperty_CurrentDevice, kAudioOutputUnitProperty_EnableIO,
};

/// Real-time audio chunk stream type used by pass-1 transcription.
#[allow(dead_code)]
pub type AudioChunk = Vec<i16>;

/// Real-time audio chunk receiver for pass-1 transcription.
#[allow(dead_code)]
pub type AudioChunkReceiver = Receiver<AudioChunk>;

#[cfg(target_os = "macos")]
const CLIP_MIN: f32 = -1.0;
#[cfg(target_os = "macos")]
const CLIP_MAX: f32 = 1.0;

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
/// Public audio input device entry for user selection.
pub struct InputDevice {
	/// Human-readable name shown in the UI dropdown.
	pub name: String,
	/// Input device identifier for configuration.
	pub device_id: u32,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
/// Audio device selection result for a recording startup attempt.
pub struct InputDeviceSelection {
	/// User-requested input device id, if any.
	pub requested_device_id: Option<u32>,
	/// Actual input device id used for capture.
	pub selected_device_id: u32,
	/// Resolved input device name used for capture.
	pub selected_device_name: String,
	/// True when the selected device differs from the requested selection.
	pub fallback_to_default: bool,
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
/// Finalized capture output.
pub struct Recording {
	/// Sample rate used for the captured PCM stream.
	pub sample_rate: u32,
	/// Channel count used for the captured PCM stream.
	pub channels: u16,
	/// Total frame count captured before WAV encoding.
	pub frames: u64,
	/// Capture duration measured at stop time.
	pub duration_ms: u64,
	/// Encoded WAV payload returned to the caller.
	pub wav: Vec<u8>,
}

#[cfg(target_os = "macos")]
/// Active microphone capture session.
pub struct Recorder {
	io_type: String,
	selected_device_id: u32,
	selected_device_name: String,
	started_at: Instant,
	sample_rate: u32,
	channels: u16,
	recording: Arc<Mutex<Vec<i16>>>,
	callback_calls: Arc<AtomicU64>,
	callback_samples: Arc<AtomicU64>,
	audio_unit: AudioUnit,
}

#[cfg(not(target_os = "macos"))]
#[derive(Debug)]
pub struct Recorder;
#[cfg(target_os = "macos")]
impl Recorder {
	/// Start a mic capture session and optionally stream PCM chunks to the caller.
	pub fn start_with_stream(
		stream_tx: Option<SyncSender<AudioChunk>>,
		selection: &InputDeviceSelection,
	) -> Result<Self, String> {
		let use_voice_processing = selection.requested_device_id.is_none();
		let io_type =
			if use_voice_processing { IOType::VoiceProcessingIO } else { IOType::HalOutput };
		let io_type_name = if use_voice_processing { "VoiceProcessingIO" } else { "HalOutput" };
		let selected_device_id = selection.selected_device_id;

		log_audio_unit_start(io_type_name, selection);

		let mut audio_unit = AudioUnit::new(io_type).map_err(|err: Error| err.to_string())?;
		let _ = audio_unit.uninitialize();

		configure_io_enablement(&mut audio_unit, io_type_name)?;

		if !use_voice_processing {
			ensure_hal_input_device(&mut audio_unit, selected_device_id)?;
		}

		let input_format =
			audio_unit.input_stream_format().map_err(|err: Error| err.to_string())?;
		let _ = audio_unit.uninitialize();
		let (sample_rate, channels) = configure_input_format(
			&mut audio_unit,
			input_format.sample_rate,
			input_format.channels,
		)?;
		let recording = Arc::new(Mutex::new(Vec::<i16>::new()));
		let recording_cb = Arc::clone(&recording);
		let callback_tx = stream_tx.clone();
		let callback_calls = Arc::new(AtomicU64::new(0));
		let callback_samples = Arc::new(AtomicU64::new(0));
		let callback_calls_cb = Arc::clone(&callback_calls);
		let callback_samples_cb = Arc::clone(&callback_samples);

		audio_unit
			.set_input_callback(move |Args { data, .. }: Args<Interleaved<f32>>| {
				callback_calls_cb.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
				callback_samples_cb
					.fetch_add(data.buffer.len() as u64, std::sync::atomic::Ordering::SeqCst);

				let mut samples = match recording_cb.lock() {
					Ok(samples) => samples,
					Err(_) => return Err(()),
				};
				let mut chunk = Vec::with_capacity(data.buffer.len());

				for sample in data.buffer.iter() {
					let sample_i16 = f32_to_i16(*sample);

					samples.push(sample_i16);
					chunk.push(sample_i16);
				}

				if let Some(tx) = callback_tx.as_ref() {
					let _ = tx.try_send(chunk);
				}

				Ok(())
			})
			.map_err(|err: Error| err.to_string())?;

		audio_unit.initialize().map_err(|err: Error| err.to_string())?;
		audio_unit.start().map_err(|err: Error| err.to_string())?;

		Ok(Self {
			io_type: io_type_name.to_string(),
			selected_device_id,
			selected_device_name: selection.selected_device_name.clone(),
			started_at: Instant::now(),
			sample_rate,
			channels,
			recording,
			callback_calls,
			callback_samples,
			audio_unit,
		})
	}

	/// Stop the capture session and return WAV bytes.
	pub fn stop(mut self) -> Result<Recording, String> {
		self.audio_unit.stop().map_err(|err: Error| err.to_string())?;

		let samples = self
			.recording
			.lock()
			.map_err(|_| "audio capture buffer is unavailable".to_string())?
			.clone();
		let callback_calls = self.callback_calls.load(std::sync::atomic::Ordering::SeqCst);
		let callback_samples = self.callback_samples.load(std::sync::atomic::Ordering::SeqCst);
		let frames = if self.channels == 0 {
			0
		} else {
			(samples.len() / usize::from(self.channels)) as u64
		};
		let duration_ms =
			if self.sample_rate == 0 { 0 } else { self.started_at.elapsed().as_millis() as u64 };
		let wav = encode_wav(&samples, self.sample_rate, self.channels)?;

		tracing::info!(
			io_type = %self.io_type,
			selected_device_id = self.selected_device_id,
			selected_device_name = %self.selected_device_name,
			sample_rate = self.sample_rate,
			channels = self.channels,
			frames,
			duration_ms,
			wav_bytes = wav.len(),
			callback_calls,
			callback_samples,
			buffer_samples = samples.len(),
			"audio capture stopped"
		);

		if samples.is_empty() && duration_ms >= 1_000 {
			return Err(format!(
				"no-audio: duration_ms={duration_ms} io_type={} device_id={} device_name=\"{}\"",
				self.io_type, self.selected_device_id, self.selected_device_name
			));
		}

		Ok(Recording {
			sample_rate: self.sample_rate,
			channels: self.channels,
			frames,
			duration_ms,
			wav,
		})
	}
}

#[cfg(not(target_os = "macos"))]
#[derive(Debug)]
pub struct Recording;

#[cfg(not(target_os = "macos"))]
#[derive(Clone, Debug)]
/// Public audio input device entry for user selection.
pub struct InputDevice {
	/// Human-readable name shown in the UI dropdown.
	pub name: String,
	/// Input device identifier for configuration.
	pub device_id: u32,
}

#[cfg(not(target_os = "macos"))]
#[derive(Clone, Debug)]
/// Audio device selection result for a recording startup attempt.
pub struct InputDeviceSelection {
	/// User-requested input device id, if any.
	pub requested_device_id: Option<u32>,
	/// Actual input device id used for capture.
	pub selected_device_id: u32,
	/// Resolved input device name used for capture.
	pub selected_device_name: String,
	/// True when the selected device differs from the requested selection.
	pub fallback_to_default: bool,
}

#[cfg(target_os = "macos")]
/// Start recording and receive real-time i16 PCM chunks.
#[allow(dead_code)]
pub fn start_recording_with_stream(
	chunk_capacity: usize,
	preferred_device_id: Option<u32>,
) -> Result<(Recorder, AudioChunkReceiver, InputDeviceSelection), String> {
	let (tx, rx) = mpsc::sync_channel(chunk_capacity);
	let selection = resolve_input_device(preferred_device_id)?;
	let recorder = Recorder::start_with_stream(Some(tx), &selection)?;

	Ok((recorder, rx, selection))
}

#[cfg(target_os = "macos")]
/// Stop recording and return the finalized capture payload.
pub fn stop_recording(recorder: Recorder) -> Result<Recording, String> {
	recorder.stop()
}

#[cfg(target_os = "macos")]
/// Return all input-capable microphones available to the app.
pub fn list_input_devices() -> Result<Vec<InputDevice>, String> {
	let ids = macos_helpers::get_audio_device_ids().map_err(|err: Error| err.to_string())?;
	let mut devices = Vec::new();

	for device_id in ids {
		let supports_input =
			macos_helpers::get_audio_device_supports_scope(device_id, Scope::Input)
				.unwrap_or(false);

		if !supports_input {
			continue;
		}

		let name =
			macos_helpers::get_device_name(device_id).map_err(|err: Error| err.to_string())?;

		devices.push(InputDevice { name, device_id });
	}

	devices.sort_by(|a, b| match a.name.cmp(&b.name) {
		std::cmp::Ordering::Equal => a.device_id.cmp(&b.device_id),
		other => other,
	});

	Ok(devices)
}

#[cfg(not(target_os = "macos"))]
/// Start recording with realtime chunk stream (stub on non-macOS).
pub fn start_recording_with_stream(
	_chnk_capacity: usize,
	_preferred_device_id: Option<u32>,
) -> Result<(Recorder, AudioChunkReceiver, InputDeviceSelection), String> {
	let _ = _chnk_capacity;
	let _ = _preferred_device_id;

	Err("recording is only supported on macOS in this build".to_string())
}

#[cfg(not(target_os = "macos"))]
/// Stop recording (stub on non-macOS).
pub fn stop_recording(_recorder: Recorder) -> Result<Recording, String> {
	Err("recording is only supported on macOS in this build".to_string())
}

#[cfg(not(target_os = "macos"))]
/// Return all input-capable microphones available to the app.
pub fn list_input_devices() -> Result<Vec<InputDevice>, String> {
	Ok(Vec::new())
}

#[cfg(target_os = "macos")]
fn log_audio_unit_start(io_type_name: &str, selection: &InputDeviceSelection) {
	tracing::info!(
		io_type = io_type_name,
		requested_device_id = selection.requested_device_id,
		selected_device_id = selection.selected_device_id,
		selected_device_name = %selection.selected_device_name,
		"starting audio unit with selected input device"
	);
}

#[cfg(target_os = "macos")]
fn configure_input_format(
	audio_unit: &mut AudioUnit,
	sample_rate: f64,
	channels: u32,
) -> Result<(u32, u16), String> {
	let format = StreamFormat {
		sample_rate,
		sample_format: coreaudio::audio_unit::SampleFormat::F32,
		flags: LinearPcmFlags::IS_FLOAT | LinearPcmFlags::IS_PACKED,
		channels,
	};

	audio_unit
		.set_stream_format(format, Scope::Output, Element::Input)
		.map_err(|err: Error| err.to_string())?;

	let sample_rate = if sample_rate.is_sign_negative() { 0 } else { sample_rate as u32 };
	let channels = u16::try_from(channels).map_err(|_| "unsupported channel count".to_string())?;

	Ok((sample_rate, channels))
}

#[cfg(target_os = "macos")]
fn configure_io_enablement(audio_unit: &mut AudioUnit, io_type_name: &str) -> Result<(), String> {
	let enable_input = 1_u32;
	let disable_output = 0_u32;
	let enable_output = 1_u32;

	audio_unit
		.set_property(
			kAudioOutputUnitProperty_EnableIO,
			Scope::Input,
			Element::Input,
			Some(&enable_input),
		)
		.map_err(|err: Error| {
			format!(
				"failed to enable input IO (io_type={io_type_name}, scope=Input, element=Input): {err}"
			)
		})?;

	if io_type_name == "VoiceProcessingIO" {
		audio_unit
			.set_property(
				kAudioOutputUnitProperty_EnableIO,
				Scope::Output,
				Element::Output,
				Some(&disable_output),
			)
			.map_err(|err: Error| {
				format!(
					"failed to disable output IO (io_type={io_type_name}, scope=Output, element=Output): {err}"
				)
			})?;
	} else {
		audio_unit
			.set_property(
				kAudioOutputUnitProperty_EnableIO,
				Scope::Output,
				Element::Output,
				Some(&enable_output),
			)
			.map_err(|err: Error| {
				format!(
					"failed to enable output IO (io_type={io_type_name}, scope=Output, element=Output): {err}"
				)
			})?;
	}

	Ok(())
}

#[cfg(target_os = "macos")]
fn set_hal_input_device(audio_unit: &mut AudioUnit, device_id: u32) -> Result<(), String> {
	let output_err = audio_unit
		.set_property(
			kAudioOutputUnitProperty_CurrentDevice,
			Scope::Global,
			Element::Output,
			Some(&device_id),
		)
		.err()
		.map(|err| err.to_string());

	if output_err.is_none() {
		return Ok(());
	}

	let input_err = audio_unit
		.set_property(
			kAudioOutputUnitProperty_CurrentDevice,
			Scope::Global,
			Element::Input,
			Some(&device_id),
		)
		.err()
		.map(|err| err.to_string());

	if input_err.is_none() {
		return Ok(());
	}

	let output_err = output_err.unwrap_or_else(|| "unknown error".to_string());
	let input_err = input_err.unwrap_or_else(|| "unknown error".to_string());

	Err(format!(
		"failed to set current input device (io_type=HalOutput, device_id={device_id}): output={output_err}, input={input_err}"
	))
}

#[cfg(target_os = "macos")]
fn ensure_hal_input_device(
	audio_unit: &mut AudioUnit,
	selected_device_id: u32,
) -> Result<(), String> {
	if let Err(primary_err) = set_hal_input_device(audio_unit, selected_device_id) {
		let (fallback_device_id, _) = default_input_device()?;

		if fallback_device_id != selected_device_id {
			tracing::warn!(
				device_id = selected_device_id,
				fallback_device_id = fallback_device_id,
				"retrying audio unit device selection after initial failure"
			);

			if let Err(fallback_err) = set_hal_input_device(audio_unit, fallback_device_id) {
				return Err(format!(
					"{primary_err}; fallback retry also failed (device_id={fallback_device_id}, error={fallback_err})"
				));
			}
		} else {
			return Err(primary_err);
		}
	}

	Ok(())
}

#[cfg(target_os = "macos")]
fn f32_to_i16(value: f32) -> i16 {
	let normalized = value.clamp(CLIP_MIN, CLIP_MAX);

	(normalized * f32::from(i16::MAX)).round() as i16
}

#[cfg(target_os = "macos")]
fn encode_wav(samples: &[i16], sample_rate: u32, channels: u16) -> Result<Vec<u8>, String> {
	if channels == 0 {
		return Err("invalid channel count".to_string());
	}
	if sample_rate == 0 {
		return Err("invalid sample rate".to_string());
	}

	let spec = hound::WavSpec {
		channels,
		sample_rate,
		bits_per_sample: 16,
		sample_format: hound::SampleFormat::Int,
	};
	let mut cursor = Cursor::new(Vec::new());
	let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|err| err.to_string())?;

	for sample in samples {
		writer.write_sample::<i16>(*sample).map_err(|err| err.to_string())?;
	}

	writer.finalize().map_err(|err| err.to_string())?;

	Ok(cursor.into_inner())
}

#[cfg(target_os = "macos")]
fn default_input_device() -> Result<(u32, String), String> {
	let device_id = macos_helpers::get_default_device_id(true)
		.ok_or_else(|| "no default input device".to_string())?;
	let name = macos_helpers::get_device_name(device_id).map_err(|err: Error| err.to_string())?;

	Ok((device_id, name))
}

#[cfg(target_os = "macos")]
fn resolve_input_device(preferred_device_id: Option<u32>) -> Result<InputDeviceSelection, String> {
	let Some(device_id) = preferred_device_id else {
		let (default_device_id, default_name) = default_input_device()?;

		return Ok(InputDeviceSelection {
			requested_device_id: None,
			selected_device_id: default_device_id,
			selected_device_name: default_name,
			fallback_to_default: false,
		});
	};
	let supports_input =
		macos_helpers::get_audio_device_supports_scope(device_id, Scope::Input).unwrap_or(false);

	if supports_input {
		let name =
			macos_helpers::get_device_name(device_id).map_err(|err: Error| err.to_string())?;

		return Ok(InputDeviceSelection {
			requested_device_id: Some(device_id),
			selected_device_id: device_id,
			selected_device_name: name,
			fallback_to_default: false,
		});
	}

	tracing::warn!(
		requested_device_id = device_id,
		"requested input device cannot be used; falling back to default"
	);

	let (default_device_id, default_name) = default_input_device()?;

	Ok(InputDeviceSelection {
		requested_device_id: Some(device_id),
		selected_device_id: default_device_id,
		selected_device_name: default_name,
		fallback_to_default: true,
	})
}
