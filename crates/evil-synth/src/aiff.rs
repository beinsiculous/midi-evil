use std::{fs, path::Path};

use crate::{Error, RenderedSound, sample::MAX_SAMPLE_SECONDS, wav::downmix_interleaved};

#[derive(Clone, Copy)]
enum Encoding {
    Pcm { little_endian: bool },
    Float32,
    Float64,
}

struct CommonChunk {
    channels: u16,
    frames: usize,
    bits_per_sample: u16,
    sample_rate: u32,
    encoding: Encoding,
}

/// Read finite mono/stereo uncompressed AIFF/AIFC audio and downmix it to mono.
pub fn read_mono_aiff(path: impl AsRef<Path>) -> Result<RenderedSound, Error> {
    let bytes = fs::read(path).map_err(|error| Error::InvalidAiff(error.to_string()))?;
    if bytes.len() < 12 || &bytes[0..4] != b"FORM" {
        return Err(invalid("AIFF audio needs a FORM header"));
    }
    let form = match &bytes[8..12] {
        b"AIFF" => *b"AIFF",
        b"AIFC" => *b"AIFC",
        _ => return Err(invalid("FORM must contain AIFF or AIFC audio")),
    };
    let form_end = be_u32(&bytes[4..8])? as usize + 8;
    if form_end > bytes.len() || form_end < 12 {
        return Err(invalid("AIFF FORM size exceeds the file"));
    }

    let mut common = None;
    let mut sound = None;
    let mut position = 12;
    while position < form_end {
        if form_end - position < 8 {
            return Err(invalid("AIFF has a truncated chunk header"));
        }
        let id = &bytes[position..position + 4];
        let size = be_u32(&bytes[position + 4..position + 8])? as usize;
        let data_start = position + 8;
        let data_end = data_start
            .checked_add(size)
            .filter(|end| *end <= form_end)
            .ok_or_else(|| Error::InvalidAiff("AIFF chunk exceeds the FORM size".to_owned()))?;
        let data = &bytes[data_start..data_end];
        match id {
            b"COMM" => common = Some(parse_common(data, &form)?),
            b"SSND" => {
                if data.len() < 8 {
                    return Err(invalid("AIFF SSND chunk is truncated"));
                }
                let offset = be_u32(&data[0..4])? as usize;
                let audio_start = 8_usize
                    .checked_add(offset)
                    .filter(|start| *start <= data.len())
                    .ok_or_else(|| {
                        Error::InvalidAiff("AIFF SSND offset exceeds the chunk".to_owned())
                    })?;
                sound = Some(&data[audio_start..]);
            }
            _ => {}
        }
        position = data_end
            .checked_add(size % 2)
            .filter(|next| *next <= form_end + 1)
            .ok_or_else(|| Error::InvalidAiff("AIFF chunk padding is invalid".to_owned()))?;
    }
    let common = common.ok_or_else(|| Error::InvalidAiff("AIFF needs a COMM chunk".to_owned()))?;
    let sound = sound.ok_or_else(|| Error::InvalidAiff("AIFF needs an SSND chunk".to_owned()))?;
    decode(common, sound)
}

fn parse_common(data: &[u8], form: &[u8; 4]) -> Result<CommonChunk, Error> {
    let aifc = form == b"AIFC";
    if data.len() < if aifc { 22 } else { 18 } {
        return Err(invalid("AIFF COMM chunk is truncated"));
    }
    let channels = be_u16(&data[0..2])?;
    if !(1..=2).contains(&channels) {
        return Err(invalid("use a mono/stereo AIFF source"));
    }
    let frames = be_u32(&data[2..6])? as usize;
    let bits_per_sample = be_u16(&data[6..8])?;
    let sample_rate = extended_sample_rate(&data[8..18])?;
    let encoding = if aifc {
        match &data[18..22] {
            b"NONE" | b"twos" => Encoding::Pcm {
                little_endian: false,
            },
            b"sowt" => Encoding::Pcm {
                little_endian: true,
            },
            b"fl32" | b"FL32" => Encoding::Float32,
            b"fl64" | b"FL64" => Encoding::Float64,
            _ => {
                return Err(invalid(
                    "AIFC compression is unsupported; use NONE, twos, sowt, fl32, or fl64",
                ));
            }
        }
    } else {
        Encoding::Pcm {
            little_endian: false,
        }
    };
    let valid_bits = match encoding {
        Encoding::Pcm { .. } => matches!(bits_per_sample, 8 | 16 | 24 | 32),
        Encoding::Float32 => bits_per_sample == 32,
        Encoding::Float64 => bits_per_sample == 64,
    };
    if !valid_bits {
        return Err(invalid(
            "AIFF supports 8/16/24/32-bit PCM and 32/64-bit float samples",
        ));
    }
    if frames == 0 || frames > sample_rate as usize * MAX_SAMPLE_SECONDS as usize {
        return Err(invalid(
            "AIFF instrument sources must be nonempty and at most 30 seconds long",
        ));
    }
    Ok(CommonChunk {
        channels,
        frames,
        bits_per_sample,
        sample_rate,
        encoding,
    })
}

fn decode(common: CommonChunk, sound: &[u8]) -> Result<RenderedSound, Error> {
    let bytes_per_sample = usize::from(common.bits_per_sample / 8);
    let sample_count = common
        .frames
        .checked_mul(usize::from(common.channels))
        .ok_or_else(|| Error::InvalidAiff("AIFF sample count overflows".to_owned()))?;
    let byte_count = sample_count
        .checked_mul(bytes_per_sample)
        .filter(|count| *count <= sound.len())
        .ok_or_else(|| Error::InvalidAiff("AIFF SSND audio is shorter than COMM".to_owned()))?;
    let mut samples = Vec::with_capacity(sample_count);
    for bytes in sound[..byte_count].chunks_exact(bytes_per_sample) {
        let sample = match common.encoding {
            Encoding::Pcm { little_endian } => {
                pcm_sample(bytes, common.bits_per_sample, little_endian)
            }
            Encoding::Float32 => f32::from_bits(be_u32(bytes)?),
            Encoding::Float64 => f64::from_bits(be_u64(bytes)?) as f32,
        };
        samples.push(sample);
    }
    let samples = downmix_interleaved(samples, common.channels);
    if samples.iter().any(|sample| !sample.is_finite()) {
        return Err(invalid("AIFF samples must be finite"));
    }
    Ok(RenderedSound {
        samples,
        sample_rate: common.sample_rate,
    })
}

fn pcm_sample(bytes: &[u8], bits: u16, little_endian: bool) -> f32 {
    match bits {
        8 => f32::from(bytes[0] as i8) / 128.0,
        16 => {
            let bytes = [bytes[0], bytes[1]];
            let value = if little_endian {
                i16::from_le_bytes(bytes)
            } else {
                i16::from_be_bytes(bytes)
            };
            f32::from(value) / 32_768.0
        }
        24 => {
            let mut value = if little_endian {
                i32::from(bytes[0]) | i32::from(bytes[1]) << 8 | i32::from(bytes[2]) << 16
            } else {
                i32::from(bytes[2]) | i32::from(bytes[1]) << 8 | i32::from(bytes[0]) << 16
            };
            if value & 0x80_0000 != 0 {
                value |= !0xff_ffff;
            }
            value as f32 / 8_388_608.0
        }
        32 => {
            let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
            let value = if little_endian {
                i32::from_le_bytes(bytes)
            } else {
                i32::from_be_bytes(bytes)
            };
            value as f32 / 2_147_483_648.0
        }
        _ => unreachable!("AIFF PCM width was validated"),
    }
}

fn extended_sample_rate(bytes: &[u8]) -> Result<u32, Error> {
    let sign_and_exponent = be_u16(&bytes[0..2])?;
    let exponent = i32::from(sign_and_exponent & 0x7fff);
    let significand = be_u64(&bytes[2..10])?;
    let mut rate = significand as f64 * 2.0_f64.powi(exponent - 16_383 - 63);
    if sign_and_exponent & 0x8000 != 0 {
        rate = -rate;
    }
    let rounded = rate.round();
    if !rate.is_finite()
        || (rate - rounded).abs() > 0.0001
        || !(8_000.0..=192_000.0).contains(&rounded)
    {
        return Err(invalid("use an integer AIFF sample rate at 8–192 kHz"));
    }
    Ok(rounded as u32)
}

fn be_u16(bytes: &[u8]) -> Result<u16, Error> {
    let bytes: [u8; 2] = bytes
        .try_into()
        .map_err(|_| Error::InvalidAiff("truncated 16-bit AIFF value".to_owned()))?;
    Ok(u16::from_be_bytes(bytes))
}

fn be_u32(bytes: &[u8]) -> Result<u32, Error> {
    let bytes: [u8; 4] = bytes
        .try_into()
        .map_err(|_| Error::InvalidAiff("truncated 32-bit AIFF value".to_owned()))?;
    Ok(u32::from_be_bytes(bytes))
}

fn be_u64(bytes: &[u8]) -> Result<u64, Error> {
    let bytes: [u8; 8] = bytes
        .try_into()
        .map_err(|_| Error::InvalidAiff("truncated 64-bit AIFF value".to_owned()))?;
    Ok(u64::from_be_bytes(bytes))
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidAiff(message.into())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn pcm_aiff_and_little_endian_aifc_decode_and_downmix() {
        let directory = tempfile::tempdir().unwrap();
        let aiff = directory.path().join("stereo.aiff");
        let mut pcm = Vec::new();
        for value in [16_384_i16, 0, -16_384, 0] {
            pcm.extend(value.to_be_bytes());
        }
        fs::write(&aiff, fixture(*b"AIFF", None, 2, 16, 48_000, pcm)).unwrap();
        let sound = read_mono_aiff(&aiff).unwrap();
        assert_eq!(sound.samples, vec![0.25, -0.25]);
        assert_eq!(sound.sample_rate, 48_000);

        let aifc = directory.path().join("little.aifc");
        let mut pcm = Vec::new();
        for value in [16_384_i16, -16_384] {
            pcm.extend(value.to_le_bytes());
        }
        fs::write(&aifc, fixture(*b"AIFC", Some(*b"sowt"), 1, 16, 44_100, pcm)).unwrap();
        let sound = read_mono_aiff(&aifc).unwrap();
        assert_eq!(sound.samples, vec![0.5, -0.5]);
        assert_eq!(sound.sample_rate, 44_100);
    }

    #[test]
    fn pcm_widths_and_float_aifc_are_decoded() {
        let directory = tempfile::tempdir().unwrap();
        for (name, bits, bytes, expected) in [
            ("eight.aiff", 8, vec![0x80, 0x40], vec![-1.0, 0.5]),
            (
                "twenty-four.aiff",
                24,
                vec![0x40, 0, 0, 0xc0, 0, 0],
                vec![0.5, -0.5],
            ),
            (
                "thirty-two.aiff",
                32,
                [0x4000_0000_i32, -0x4000_0000]
                    .into_iter()
                    .flat_map(i32::to_be_bytes)
                    .collect(),
                vec![0.5, -0.5],
            ),
        ] {
            let path = directory.path().join(name);
            fs::write(&path, fixture(*b"AIFF", None, 1, bits, 8_000, bytes)).unwrap();
            assert_eq!(read_mono_aiff(&path).unwrap().samples, expected);
        }

        for (name, compression, bits, bytes) in [
            (
                "float.aifc",
                *b"fl32",
                32,
                [0.25_f32, -0.5]
                    .into_iter()
                    .flat_map(f32::to_be_bytes)
                    .collect::<Vec<_>>(),
            ),
            (
                "double.aifc",
                *b"fl64",
                64,
                [0.25_f64, -0.5]
                    .into_iter()
                    .flat_map(f64::to_be_bytes)
                    .collect::<Vec<_>>(),
            ),
        ] {
            let path = directory.path().join(name);
            fs::write(
                &path,
                fixture(*b"AIFC", Some(compression), 1, bits, 8_000, bytes),
            )
            .unwrap();
            assert_eq!(read_mono_aiff(&path).unwrap().samples, vec![0.25, -0.5]);
        }
    }

    #[test]
    fn invalid_aiff_is_rejected_without_panicking_on_truncation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("invalid.aiff");
        let bytes = fixture(*b"AIFF", None, 1, 16, 48_000, vec![0, 0]);
        for end in 0..bytes.len() {
            fs::write(&path, &bytes[..end]).unwrap();
            assert!(read_mono_aiff(&path).is_err(), "accepted prefix {end}");
        }
        fs::write(&path, bytes).unwrap();
        assert!(read_mono_aiff(&path).is_ok());

        fs::write(
            &path,
            fixture(*b"AIFC", Some(*b"ulaw"), 1, 16, 48_000, vec![0, 0]),
        )
        .unwrap();
        assert!(read_mono_aiff(&path).is_err());

        fs::write(
            &path,
            fixture(
                *b"AIFC",
                Some(*b"fl32"),
                1,
                32,
                48_000,
                f32::NAN.to_be_bytes().to_vec(),
            ),
        )
        .unwrap();
        assert!(read_mono_aiff(&path).is_err());

        fs::write(&path, fixture(*b"AIFF", None, 3, 16, 48_000, vec![0; 6])).unwrap();
        assert!(read_mono_aiff(&path).is_err());
        fs::write(&path, fixture(*b"AIFF", None, 1, 16, 4_000, vec![0, 0])).unwrap();
        assert!(read_mono_aiff(&path).is_err());
    }

    fn fixture(
        form: [u8; 4],
        compression: Option<[u8; 4]>,
        channels: u16,
        bits: u16,
        sample_rate: u32,
        audio: Vec<u8>,
    ) -> Vec<u8> {
        let mut common = Vec::new();
        common.extend(channels.to_be_bytes());
        let frames = audio.len() / (usize::from(channels) * usize::from(bits / 8));
        common.extend((frames as u32).to_be_bytes());
        common.extend(bits.to_be_bytes());
        common.extend(extended_rate(sample_rate));
        if let Some(compression) = compression {
            common.extend(compression);
            common.extend([0]);
        }
        let mut sound = vec![0; 8];
        sound.extend(audio);
        let mut chunks = chunk(*b"COMM", &common);
        chunks.extend(chunk(*b"SSND", &sound));
        let mut bytes = Vec::new();
        bytes.extend(b"FORM");
        bytes.extend((chunks.len() as u32 + 4).to_be_bytes());
        bytes.extend(form);
        bytes.extend(chunks);
        bytes
    }

    fn chunk(id: [u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(id);
        bytes.extend((data.len() as u32).to_be_bytes());
        bytes.extend(data);
        if !data.len().is_multiple_of(2) {
            bytes.push(0);
        }
        bytes
    }

    fn extended_rate(rate: u32) -> [u8; 10] {
        let mut bytes = [0; 10];
        let exponent = 31 - rate.leading_zeros();
        let significand = u64::from(rate) << (63 - exponent);
        bytes[0..2].copy_from_slice(&(16_383 + exponent as u16).to_be_bytes());
        bytes[2..10].copy_from_slice(&significand.to_be_bytes());
        bytes
    }
}
