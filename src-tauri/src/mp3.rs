use anyhow::{Context, Result};
use std::path::Path;
use windows::core::HSTRING;
use windows::Win32::Media::MediaFoundation::{
    MFAudioFormat_MP3, MFAudioFormat_PCM, MFCreateMediaType, MFCreateMemoryBuffer,
    MFCreateSample, MFCreateSinkWriterFromURL, MFMediaType_Audio, MFStartup, MFShutdown,
    MFSTARTUP_NOSOCKET, MF_VERSION, MF_MT_ALL_SAMPLES_INDEPENDENT,
    MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT,
    MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
};

const SAMPLE_RATE: u32 = 16_000;
const OUTPUT_SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u32 = 1;
const BITS_PER_SAMPLE: u32 = 16;
const MP3_BYTES_PER_SECOND: u32 = 8_000;

/// Convert the app's mono 16-bit/16 kHz recording to MP3 using the Windows
/// Media Foundation encoder. No external ffmpeg/lame binary is required.
pub fn encode(input: &Path, output: &Path) -> Result<()> {
    let mut reader = hound::WavReader::open(input)
        .with_context(|| format!("Không mở được bản thu {}", input.display()))?;
    let spec = reader.spec();
    anyhow::ensure!(
        spec.channels == CHANNELS as u16
            && spec.sample_rate == SAMPLE_RATE
            && spec.bits_per_sample == BITS_PER_SAMPLE as u16
            && spec.sample_format == hound::SampleFormat::Int,
        "Bản thu không phải PCM 16-bit mono 16 kHz"
    );
    let samples: Vec<i16> = reader
        .samples::<i16>()
        .collect::<std::result::Result<_, _>>()
        .context("Không đọc được dữ liệu bản thu")?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if output.exists() {
        std::fs::remove_file(output)?;
    }
    let samples = resample(&samples, SAMPLE_RATE, OUTPUT_SAMPLE_RATE);

    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).context("Không khởi động được Media Foundation")?;
        let result = encode_samples(&samples, output);
        let _ = MFShutdown();
        if result.is_err() {
            let _ = std::fs::remove_file(output);
        }
        result
    }
}

fn resample(samples: &[i16], from: u32, to: u32) -> Vec<i16> {
    if samples.is_empty() || from == to {
        return samples.to_vec();
    }
    let out_len = ((samples.len() as u64 * to as u64) / from as u64) as usize;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * from as f64 / to as f64;
            let left = pos.floor() as usize;
            let fraction = pos - left as f64;
            let a = samples[left.min(samples.len() - 1)] as f64;
            let b = samples[(left + 1).min(samples.len() - 1)] as f64;
            (a + (b - a) * fraction).round() as i16
        })
        .collect()
}

unsafe fn encode_samples(samples: &[i16], output: &Path) -> Result<()> {
    let output_url = HSTRING::from(output.to_string_lossy().as_ref());
    let writer = MFCreateSinkWriterFromURL(
        &output_url,
        None::<&windows::Win32::Media::MediaFoundation::IMFByteStream>,
        None::<&windows::Win32::Media::MediaFoundation::IMFAttributes>,
    )
    .context("Không tạo được bộ mã hóa MP3")?;

    let output_type = MFCreateMediaType()?;
    output_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
    output_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_MP3)?;
    output_type.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, CHANNELS)?;
    output_type.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, OUTPUT_SAMPLE_RATE)?;
    output_type.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MP3_BYTES_PER_SECOND)?;
    let stream = writer.AddStream(&output_type).context("Không thêm được luồng MP3")?;

    let input_type = MFCreateMediaType()?;
    input_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
    input_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
    input_type.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, CHANNELS)?;
    input_type.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, OUTPUT_SAMPLE_RATE)?;
    input_type.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, BITS_PER_SAMPLE)?;
    input_type.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, CHANNELS * BITS_PER_SAMPLE / 8)?;
    input_type.SetUINT32(
        &MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
        OUTPUT_SAMPLE_RATE * CHANNELS * BITS_PER_SAMPLE / 8,
    )?;
    input_type.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
    writer
        .SetInputMediaType(stream, &input_type, None::<&windows::Win32::Media::MediaFoundation::IMFAttributes>)
        .context("Không cấu hình được đầu vào MP3")?;
    writer.BeginWriting().context("Không bắt đầu ghi MP3")?;

    let mut written = 0usize;
    for chunk in samples.chunks(OUTPUT_SAMPLE_RATE as usize) {
        let byte_len = chunk.len() * 2;
        let buffer = MFCreateMemoryBuffer(byte_len as u32)?;
        let mut target: *mut u8 = std::ptr::null_mut();
        buffer.Lock(&mut target, None, None)?;
        std::ptr::copy_nonoverlapping(
            chunk.as_ptr().cast::<u8>(),
            target,
            byte_len,
        );
        buffer.Unlock()?;
        buffer.SetCurrentLength(byte_len as u32)?;

        let sample = MFCreateSample()?;
        sample.AddBuffer(&buffer)?;
        sample.SetSampleTime((written as i64 * 10_000_000) / OUTPUT_SAMPLE_RATE as i64)?;
        sample.SetSampleDuration((chunk.len() as i64 * 10_000_000) / OUTPUT_SAMPLE_RATE as i64)?;
        writer.WriteSample(stream, &sample)?;
        written += chunk.len();
    }
    writer.Finalize().context("Không hoàn tất được file MP3")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_pcm_wav_to_mp3() {
        let dir = std::env::temp_dir().join(format!("innote-mp3-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("input.wav");
        let mp3 = dir.join("output.mp3");
        let mut writer = hound::WavWriter::create(
            &wav,
            hound::WavSpec {
                channels: CHANNELS as u16,
                sample_rate: SAMPLE_RATE,
                bits_per_sample: BITS_PER_SAMPLE as u16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for i in 0..SAMPLE_RATE {
            let sample = ((i as f32 * 440.0 * std::f32::consts::TAU / SAMPLE_RATE as f32).sin()
                * 0.2
                * i16::MAX as f32) as i16;
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
        encode(&wav, &mp3).unwrap();
        assert!(std::fs::metadata(&mp3).unwrap().len() > 1_000);
        let _ = std::fs::remove_dir_all(dir);
    }
}