//! Audio payload preparation shared by speech providers.

#[cfg(target_os = "macos")] use std::io::Cursor;

#[cfg(target_os = "macos")] use hound::{SampleFormat, WavReader, WavSpec, WavWriter};

#[cfg(target_os = "macos")]
const MODEL_AUDIO_SAMPLE_RATE: u32 = 24_000;
#[cfg(target_os = "macos")]
const MODEL_AUDIO_CHANNELS: u16 = 1;

/// Metadata extracted from a WAV payload.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct WavMetadata {
	pub(crate) sample_rate_hz: u32,
	pub(crate) channels: u16,
	pub(crate) bits_per_sample: u16,
	pub(crate) duration_ms: u64,
}

/// Provider-ready transcription audio payload.
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub(crate) struct PreparedTranscriptionAudio {
	pub(crate) wav: Vec<u8>,
	pub(crate) input: WavMetadata,
	pub(crate) request: WavMetadata,
}

/// Normalize captured WAV bytes into the ChatGPT transcription input shape.
#[cfg(target_os = "macos")]
pub(crate) fn prepare_chatgpt_transcription_wav(
	wav: &[u8],
) -> Result<PreparedTranscriptionAudio, String> {
	let (input_spec, input_samples) = decode_wav_pcm16(wav)?;
	let input = wav_metadata(&input_spec, input_samples.len());
	let request_wav = encode_wav_normalized(
		&input_samples,
		input_spec.sample_rate,
		input_spec.channels,
		MODEL_AUDIO_SAMPLE_RATE,
		MODEL_AUDIO_CHANNELS,
	)?;
	let request = inspect_wav_metadata(&request_wav)?;

	Ok(PreparedTranscriptionAudio { wav: request_wav, input, request })
}

#[cfg(target_os = "macos")]
fn inspect_wav_metadata(wav: &[u8]) -> Result<WavMetadata, String> {
	let reader =
		WavReader::new(Cursor::new(wav)).map_err(|err| format!("failed to parse wav: {err}"))?;
	let spec = reader.spec();
	let samples = reader.duration() as usize;

	Ok(wav_metadata(&spec, samples))
}

#[cfg(target_os = "macos")]
fn wav_metadata(spec: &WavSpec, sample_count: usize) -> WavMetadata {
	let channels = spec.channels.max(1);
	let frames = sample_count / usize::from(channels);
	let duration_ms = if spec.sample_rate == 0 {
		0
	} else {
		((frames as u128) * 1_000 / (spec.sample_rate as u128)) as u64
	};

	WavMetadata {
		sample_rate_hz: spec.sample_rate,
		channels: spec.channels,
		bits_per_sample: spec.bits_per_sample,
		duration_ms,
	}
}

#[cfg(target_os = "macos")]
fn decode_wav_pcm16(wav: &[u8]) -> Result<(WavSpec, Vec<i16>), String> {
	let mut reader =
		WavReader::new(Cursor::new(wav)).map_err(|err| format!("failed to parse wav: {err}"))?;
	let spec = reader.spec();

	if spec.channels == 0 {
		return Err("wav has invalid channel count (0)".to_string());
	}
	if spec.sample_rate == 0 {
		return Err("wav has invalid sample rate (0)".to_string());
	}

	let samples = match spec.sample_format {
		SampleFormat::Int if spec.bits_per_sample == 0 || spec.bits_per_sample > 32 =>
			return Err(format!("unsupported integer wav bit depth: {}", spec.bits_per_sample)),
		SampleFormat::Int if spec.bits_per_sample <= 16 => reader
			.samples::<i16>()
			.map(|sample| sample.map_err(|err| err.to_string()))
			.collect::<Result<Vec<_>, _>>()?,
		SampleFormat::Int => {
			let shift = spec.bits_per_sample.saturating_sub(16).min(16);

			reader
				.samples::<i32>()
				.map(|sample| {
					sample.map_err(|err| err.to_string()).map(|value| {
						(value >> shift).clamp(i16::MIN as i32, i16::MAX as i32) as i16
					})
				})
				.collect::<Result<Vec<_>, _>>()?
		},
		SampleFormat::Float => reader
			.samples::<f32>()
			.map(|sample| {
				sample.map_err(|err| err.to_string()).map(|value| {
					(value.clamp(-1.0, 1.0) * (i16::MAX as f32))
						.round()
						.clamp(i16::MIN as f32, i16::MAX as f32) as i16
				})
			})
			.collect::<Result<Vec<_>, _>>()?,
	};

	Ok((spec, samples))
}

#[cfg(target_os = "macos")]
fn encode_wav_normalized(
	input: &[i16],
	input_sample_rate: u32,
	input_channels: u16,
	output_sample_rate: u32,
	output_channels: u16,
) -> Result<Vec<u8>, String> {
	if input_sample_rate == 0
		|| input_channels == 0
		|| output_sample_rate == 0
		|| output_channels == 0
	{
		return Err("invalid audio format for normalization".to_string());
	}

	let converted = if input_sample_rate == output_sample_rate && input_channels == output_channels
	{
		input.to_vec()
	} else {
		convert_pcm16(input, input_sample_rate, input_channels, output_sample_rate, output_channels)
	};
	let mut peak_abs = 0_i32;

	for sample in &converted {
		peak_abs = peak_abs.max((*sample as i32).abs());
	}

	let target = (i16::MAX as f32) * 0.9;
	let gain = if peak_abs > 0 { target / (peak_abs as f32) } else { 1.0 };
	let spec = WavSpec {
		channels: output_channels,
		sample_rate: output_sample_rate,
		bits_per_sample: 16,
		sample_format: SampleFormat::Int,
	};
	let mut cursor = Cursor::new(Vec::new());
	let mut writer = WavWriter::new(&mut cursor, spec)
		.map_err(|err| format!("failed to create wav writer: {err}"))?;

	for sample in converted {
		let scaled =
			((sample as f32) * gain).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;

		writer.write_sample(scaled).map_err(|err| format!("failed writing wav sample: {err}"))?;
	}

	writer.finalize().map_err(|err| format!("failed to finalize wav: {err}"))?;

	Ok(cursor.into_inner())
}

#[cfg(target_os = "macos")]
fn convert_pcm16(
	input: &[i16],
	input_sample_rate: u32,
	input_channels: u16,
	output_sample_rate: u32,
	output_channels: u16,
) -> Vec<i16> {
	if input.is_empty() || input_channels == 0 || output_channels == 0 {
		return Vec::new();
	}

	let in_channels = input_channels as usize;
	let out_channels = output_channels as usize;
	let in_frames = input.len() / in_channels;

	if in_frames == 0 {
		return Vec::new();
	}

	let out_frames = if input_sample_rate == output_sample_rate {
		in_frames
	} else {
		(((in_frames as u64) * (output_sample_rate as u64)) / (input_sample_rate as u64)).max(1)
			as usize
	};
	let mut out = Vec::with_capacity(out_frames.saturating_mul(out_channels));

	for out_idx in 0..out_frames {
		let src_frame_idx = if output_sample_rate == input_sample_rate {
			out_idx
		} else {
			((out_idx as u64) * (input_sample_rate as u64) / (output_sample_rate as u64)) as usize
		}
		.min(in_frames - 1);
		let src_start = src_frame_idx.saturating_mul(in_channels);
		let src = &input[src_start..src_start + in_channels];

		match (in_channels, out_channels) {
			(1, 1) => out.push(src[0]),
			(1, m) => {
				let s = src[0];

				for _ in 0..m {
					out.push(s);
				}
			},
			(n, 1) => {
				let sum: i32 = src.iter().map(|s| *s as i32).sum();

				out.push((sum / (n as i32)) as i16);
			},
			(n, m) if n == m => out.extend_from_slice(src),
			(n, m) if n > m => out.extend_from_slice(&src[..m]),
			(n, m) => {
				out.extend_from_slice(src);

				let last = *src.last().unwrap_or(&0);

				for _ in n..m {
					out.push(last);
				}
			},
		}
	}

	out
}
