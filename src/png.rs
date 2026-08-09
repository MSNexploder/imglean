use std::fmt;
use std::io::Cursor;
use std::time::Instant;

use crate::limits::{
    MAX_ANCILLARY_BYTES, MAX_CANDIDATE_BYTES, MAX_CHUNK_BYTES, MAX_CHUNKS, MAX_HEIGHT, MAX_PIXELS,
    MAX_RECONSTRUCTED_BYTES, MAX_SOURCE_BYTES, MAX_WIDTH, VALIDATION_TIMEOUT,
};

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const XMP_KEYWORD: &[u8] = b"XML:com.adobe.xmp";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPng {
    encoded_bytes: usize,
    width: u32,
    height: u32,
}

impl ValidatedPng {
    pub fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ValidationError {
    message: &'static str,
}

impl ValidationError {
    pub fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for ValidationError {}

pub fn validate_source(bytes: &[u8]) -> Result<ValidatedPng, ValidationError> {
    validate(bytes, MAX_SOURCE_BYTES)
}

pub fn validate_candidate(
    source: &ValidatedPng,
    bytes: &[u8],
) -> Result<ValidatedPng, ValidationError> {
    let candidate = validate(bytes, MAX_CANDIDATE_BYTES)?;
    if source.width != candidate.width || source.height != candidate.height {
        return failure("candidate changes PNG dimensions");
    }
    Ok(candidate)
}

fn validate(bytes: &[u8], maximum_encoded_bytes: u64) -> Result<ValidatedPng, ValidationError> {
    let started = Instant::now();
    if u64::try_from(bytes.len()).map_or(true, |length| length > maximum_encoded_bytes) {
        return failure("PNG exceeds the encoded-byte limit");
    }
    inspect_container(bytes, started)?;

    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: MAX_RECONSTRUCTED_BYTES,
        },
    );
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder
        .read_info()
        .map_err(|_| error("PNG header or metadata is invalid"))?;
    let info = reader.info();
    let width = info.width;
    let height = info.height;
    if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
        return failure("PNG dimensions are outside the accepted limits");
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(error("PNG pixel count overflows"))?;
    if pixels > MAX_PIXELS {
        return failure("PNG exceeds the pixel limit");
    }
    if info.animation_control.is_some() || info.frame_control.is_some() {
        return failure("APNG is not supported");
    }
    let output_size = reader
        .output_buffer_size()
        .ok_or(error("PNG decoded size overflows"))?;
    if output_size > MAX_RECONSTRUCTED_BYTES {
        return failure("PNG exceeds the reconstructed-byte limit");
    }
    check_time(started)?;
    let mut decoded = vec![0; output_size];
    reader
        .next_frame(&mut decoded)
        .map_err(|_| error("PNG image data is invalid"))?;
    check_time(started)?;

    Ok(ValidatedPng {
        encoded_bytes: bytes.len(),
        width,
        height,
    })
}

fn inspect_container(bytes: &[u8], started: Instant) -> Result<(), ValidationError> {
    if !bytes.starts_with(SIGNATURE) {
        return failure("invalid PNG signature");
    }
    let mut position = SIGNATURE.len();
    let mut chunks = 0usize;
    let mut ancillary_bytes = 0usize;
    let mut saw_iend = false;
    while position < bytes.len() {
        check_time(started)?;
        if chunks == MAX_CHUNKS {
            return failure("PNG exceeds the chunk-count limit");
        }
        chunks += 1;
        let header_end = position
            .checked_add(8)
            .ok_or(error("PNG chunk overflows"))?;
        if header_end > bytes.len() {
            return failure("truncated PNG chunk header");
        }
        let length = usize::try_from(u32::from_be_bytes(
            bytes[position..position + 4]
                .try_into()
                .map_err(|_| error("invalid PNG chunk length"))?,
        ))
        .map_err(|_| error("PNG chunk length is unsupported"))?;
        if length > MAX_CHUNK_BYTES {
            return failure("PNG chunk exceeds the chunk-byte limit");
        }
        let chunk_end = header_end
            .checked_add(length)
            .and_then(|end| end.checked_add(4))
            .ok_or(error("PNG chunk overflows"))?;
        if chunk_end > bytes.len() {
            return failure("truncated PNG chunk");
        }
        let chunk_type: [u8; 4] = bytes[position + 4..position + 8]
            .try_into()
            .map_err(|_| error("invalid PNG chunk type"))?;
        let data = &bytes[header_end..header_end + length];
        validate_crc(chunk_type, data, &bytes[header_end + length..chunk_end])?;
        if chunk_type[0] & 0x20 != 0 {
            ancillary_bytes = ancillary_bytes
                .checked_add(length)
                .ok_or(error("PNG ancillary size overflows"))?;
            if ancillary_bytes > MAX_ANCILLARY_BYTES {
                return failure("PNG exceeds the ancillary-byte limit");
            }
        }
        match &chunk_type {
            b"acTL" | b"fcTL" | b"fdAT" => return failure("APNG is not supported"),
            b"caBX" => return failure("C2PA-bearing PNG is not supported"),
            b"tEXt" | b"zTXt" | b"iTXt" if text_keyword(data) == Some(XMP_KEYWORD) => {
                return failure("XMP-bearing PNG is not supported");
            }
            b"IEND" => {
                saw_iend = true;
                position = chunk_end;
                break;
            }
            _ => {}
        }
        position = chunk_end;
    }
    if !saw_iend {
        return failure("PNG is missing IEND");
    }
    if position != bytes.len() {
        return failure("PNG has trailing bytes");
    }
    Ok(())
}

fn validate_crc(chunk_type: [u8; 4], data: &[u8], stored: &[u8]) -> Result<(), ValidationError> {
    let stored = u32::from_be_bytes(
        stored
            .try_into()
            .map_err(|_| error("invalid PNG chunk CRC"))?,
    );
    let mut crc = crc32fast::Hasher::new();
    crc.update(&chunk_type);
    crc.update(data);
    if crc.finalize() != stored {
        return failure("PNG chunk CRC mismatch");
    }
    Ok(())
}

fn text_keyword(data: &[u8]) -> Option<&[u8]> {
    let end = data.iter().position(|byte| *byte == 0)?;
    Some(&data[..end])
}

fn check_time(started: Instant) -> Result<(), ValidationError> {
    if started.elapsed() > VALIDATION_TIMEOUT {
        failure("PNG validation exceeded the elapsed-time limit")
    } else {
        Ok(())
    }
}

fn error(message: &'static str) -> ValidationError {
    ValidationError { message }
}

fn failure<T>(message: &'static str) -> Result<T, ValidationError> {
    Err(error(message))
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    use super::*;

    #[test]
    fn validates_all_static_png_encodings_in_the_corpus() {
        for bytes in [
            include_bytes!("../tests/corpus/png/v2/accepted/grayscale1.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/grayscale2.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/grayscale4.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/grayscale8.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/grayscale16.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/truecolor8.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/truecolor16.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/indexed1.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/indexed2.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/indexed4.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/indexed8.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/grayscale-alpha8.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/grayscale-alpha16.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/truecolor-alpha8.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/truecolor-alpha16.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/adam7.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/ancillary-before-after.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/indexed-transparency.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/compressed-text.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/international-text.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/icc-profile.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/exif.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/unknown-ancillary.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/accepted/pngquant-reduction.png").as_slice(),
        ] {
            validate_source(bytes).unwrap();
        }
    }

    #[test]
    fn accepts_changed_pixels_but_rejects_changed_dimensions() {
        let source_bytes = png(1, 1, 42);
        let changed_pixels = png(1, 1, 43);
        let changed_dimensions = png(2, 1, 42);
        let source = validate_source(&source_bytes).unwrap();
        validate_candidate(&source, &changed_pixels).unwrap();
        assert_eq!(
            validate_candidate(&source, &changed_dimensions)
                .unwrap_err()
                .message(),
            "candidate changes PNG dimensions"
        );
    }

    #[test]
    fn rejects_apng_xmp_c2pa_bad_crc_and_trailing_bytes() {
        for bytes in [
            include_bytes!("../tests/corpus/png/v2/rejected/apng.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/rejected/xmp.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/rejected/compressed-xmp.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/rejected/international-xmp.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/rejected/cabx.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/rejected/bad-crc.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/rejected/trailing.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/rejected/truncated.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/rejected/invalid-filter.png").as_slice(),
            include_bytes!("../tests/corpus/png/v2/rejected/oversized-dimensions.png").as_slice(),
        ] {
            assert!(validate_source(bytes).is_err());
        }
    }

    #[test]
    fn every_truncation_is_rejected() {
        let bytes = png(1, 1, 42);
        for length in 0..bytes.len() {
            assert!(
                validate_source(&bytes[..length]).is_err(),
                "length {length}"
            );
        }
    }

    fn png(width: u32, height: u32, sample: u8) -> Vec<u8> {
        let mut bytes = SIGNATURE.to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
        push_chunk(&mut bytes, b"IHDR", &ihdr);
        let mut filtered = Vec::new();
        for _ in 0..height {
            filtered.push(0);
            filtered.extend(std::iter::repeat_n(sample, width as usize));
        }
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&filtered).unwrap();
        push_chunk(&mut bytes, b"IDAT", &encoder.finish().unwrap());
        push_chunk(&mut bytes, b"IEND", &[]);
        bytes
    }

    fn push_chunk(png: &mut Vec<u8>, name: &[u8; 4], data: &[u8]) {
        png.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
        png.extend_from_slice(name);
        png.extend_from_slice(data);
        let mut crc = crc32fast::Hasher::new();
        crc.update(name);
        crc.update(data);
        png.extend_from_slice(&crc.finalize().to_be_bytes());
    }
}
