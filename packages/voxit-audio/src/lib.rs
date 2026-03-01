//! Audio capture primitives for Voxit.

use std::{
	io::Cursor,
	sync::{
		Arc, Mutex,
		mpsc::{Receiver, SyncSender, sync_channel},
	},
	time::Instant,
};

#[cfg(target_os = "macos")]
use coreaudio::audio_unit::{
	AudioUnit, Element, IOType, SampleFormat, Scope, StreamFormat,
	audio_format::LinearPcmFlags,
	macos_helpers,
	render_callback::{Args, data::Interleaved},
};
#[cfg(target_os = "macos")] use coreaudio::error::Error as CoreAudioError;
#[cfg(target_os = "macos")]
use objc2_audio_toolbox::{
	kAudioOutputUnitProperty_CurrentDevice, kAudioOutputUnitProperty_EnableIO,
};

#[cfg(target_os = "macos")]
const CLIP_MIN: f32 = -1.0;
#[cfg(target_os = "macos")]
const CLIP_MAX: f32 = 1.0;

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
/// Public audio input device entry for user selection.
pub struct InputDevice {
	pub name: String,
	pub device_id: u32,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
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
	pub sample_rate: u32,
	pub channels: u16,
	pub frames: u64,
	pub duration_ms: u64,
	pub wav: Vec<u8>,
}

/// Real-time audio chunk stream type used by pass-1 transcription.
#[allow(dead_code)]
pub type AudioChunk = Vec<i16>;

/// Real-time audio chunk receiver for pass-1 transcription.
#[allow(dead_code)]
pub type AudioChunkReceiver = Receiver<AudioChunk>;

#[cfg(target_os = "macos")]
/// Active microphone capture session.
pub struct Recorder {
	started_at: Instant,
	sample_rate: u32,
	channels: u16,
	recording: Arc<Mutex<Vec<i16>>>,
	audio_unit: AudioUnit,
}

#[cfg(target_os = "macos")]
impl Recorder {
	/// Start a mic capture session and optionally stream PCM chunks to the caller.
	pub fn start_with_stream(
		stream_tx: Option<SyncSender<AudioChunk>>,
		device_id: u32,
	) -> Result<Self, String> {
		let mut audio_unit = AudioUnit::new(IOType::VoiceProcessingIO)
			.map_err(|err: CoreAudioError| err.to_string())?;
		let _ = audio_unit.uninitialize();
		let enable_input = 1_u32;
		let disable_output = 0_u32;

		audio_unit
			.set_property(
				kAudioOutputUnitProperty_EnableIO,
				Scope::Input,
				Element::Input,
				Some(&enable_input),
			)
			.map_err(|err: CoreAudioError| err.to_string())?;
		audio_unit
			.set_property(
				kAudioOutputUnitProperty_EnableIO,
				Scope::Output,
				Element::Output,
				Some(&disable_output),
			)
			.map_err(|err: CoreAudioError| err.to_string())?;

		audio_unit
			.set_property(
				kAudioOutputUnitProperty_CurrentDevice,
				Scope::Global,
				Element::Output,
				Some(&device_id),
			)
			.map_err(|err: CoreAudioError| err.to_string())?;

		let input_format =
			audio_unit.input_stream_format().map_err(|err: CoreAudioError| err.to_string())?;
		let _ = audio_unit.uninitialize();
		let (sample_rate, channels) = configure_input_format(
			&mut audio_unit,
			input_format.sample_rate,
			input_format.channels,
		)?;
		let recording = Arc::new(Mutex::new(Vec::<i16>::new()));
		let recording_cb = Arc::clone(&recording);
		let callback_tx = stream_tx.clone();

		audio_unit
			.set_input_callback(move |Args { data, .. }: Args<Interleaved<f32>>| {
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
			.map_err(|err: CoreAudioError| err.to_string())?;

		audio_unit.initialize().map_err(|err: CoreAudioError| err.to_string())?;
		audio_unit.start().map_err(|err: CoreAudioError| err.to_string())?;

		Ok(Self { started_at: Instant::now(), sample_rate, channels, recording, audio_unit })
	}

	/// Stop the capture session and return WAV bytes.
	pub fn stop(mut self) -> Result<Recording, String> {
		self.audio_unit.stop().map_err(|err: CoreAudioError| err.to_string())?;

		let samples = self
			.recording
			.lock()
			.map_err(|_| "audio capture buffer is unavailable".to_string())?
			.clone();

		let frames = if self.channels == 0 {
			0
		} else {
			(samples.len() / usize::from(self.channels)) as u64
		};
		let duration_ms =
			if self.sample_rate == 0 { 0 } else { self.started_at.elapsed().as_millis() as u64 };
		let wav = encode_wav(&samples, self.sample_rate, self.channels)?;

		Ok(Recording {
			sample_rate: self.sample_rate,
			channels: self.channels,
			frames,
			duration_ms,
			wav,
		})
	}
}

#[cfg(target_os = "macos")]
fn configure_input_format(
	audio_unit: &mut AudioUnit,
	sample_rate: f64,
	channels: u32,
) -> Result<(u32, u16), String> {
	let format = StreamFormat {
		sample_rate,
		sample_format: SampleFormat::F32,
		flags: LinearPcmFlags::IS_FLOAT | LinearPcmFlags::IS_PACKED,
		channels,
	};
	audio_unit
		.set_stream_format(format, Scope::Output, Element::Input)
		.map_err(|err: CoreAudioError| err.to_string())?;

	let sample_rate = if sample_rate.is_sign_negative() { 0 } else { sample_rate as u32 };
	let channels = u16::try_from(channels).map_err(|_| "unsupported channel count".to_string())?;
	Ok((sample_rate, channels))
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
	let mut cursor = Cursor::new(Vec::new());
	let spec = hound::WavSpec {
		channels,
		sample_rate,
		bits_per_sample: 16,
		sample_format: hound::SampleFormat::Int,
	};

	let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|err| err.to_string())?;

	for sample in samples {
		writer.write_sample::<i16>(*sample).map_err(|err| err.to_string())?;
	}
	writer.finalize().map_err(|err| err.to_string())?;

	Ok(cursor.into_inner())
}

#[cfg(target_os = "macos")]
/// Start recording and receive real-time i16 PCM chunks.
#[allow(dead_code)]
pub fn start_recording_with_stream(
	chunk_capacity: usize,
	preferred_device_id: Option<u32>,
) -> Result<(Recorder, AudioChunkReceiver, InputDeviceSelection), String> {
	let (tx, rx) = sync_channel(chunk_capacity);
	let selection = resolve_input_device(preferred_device_id)?;
	let recorder = Recorder::start_with_stream(Some(tx), selection.selected_device_id)?;
	Ok((recorder, rx, selection))
}

#[cfg(target_os = "macos")]
pub fn stop_recording(recorder: Recorder) -> Result<Recording, String> {
	recorder.stop()
}

#[cfg(target_os = "macos")]
/// Return all input-capable microphones available to the app.
pub fn list_input_devices() -> Result<Vec<InputDevice>, String> {
	let mut devices = Vec::new();
	let ids =
		macos_helpers::get_audio_device_ids().map_err(|err: CoreAudioError| err.to_string())?;

	for device_id in ids {
		let supports_input =
			macos_helpers::get_audio_device_supports_scope(device_id, Scope::Input)
				.unwrap_or(false);
		if !supports_input {
			continue;
		}

		let name = macos_helpers::get_device_name(device_id)
			.map_err(|err: CoreAudioError| err.to_string())?;
		devices.push(InputDevice { name, device_id });
	}

	devices.sort_by(|a, b| match a.name.cmp(&b.name) {
		std::cmp::Ordering::Equal => a.device_id.cmp(&b.device_id),
		other => other,
	});

	Ok(devices)
}

#[cfg(target_os = "macos")]
fn default_input_device() -> Result<(u32, String), String> {
	let device_id = macos_helpers::get_default_device_id(true)
		.ok_or_else(|| "no default input device".to_string())?;
	let name =
		macos_helpers::get_device_name(device_id).map_err(|err: CoreAudioError| err.to_string())?;

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
		let name = macos_helpers::get_device_name(device_id)
			.map_err(|err: CoreAudioError| err.to_string())?;
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

#[cfg(not(target_os = "macos"))]
#[derive(Debug)]
pub struct Recorder;

#[cfg(not(target_os = "macos"))]
#[derive(Debug)]
pub struct Recording;

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
#[derive(Debug, Clone)]
/// Public audio input device entry for user selection.
pub struct InputDevice {
	/// Human-readable name shown in the UI dropdown.
	pub name: String,
	/// Input device identifier for configuration.
	pub device_id: u32,
}

#[cfg(not(target_os = "macos"))]
#[derive(Debug, Clone)]
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

#[cfg(not(target_os = "macos"))]
/// Return all input-capable microphones available to the app.
pub fn list_input_devices() -> Result<Vec<InputDevice>, String> {
	Ok(Vec::new())
}
