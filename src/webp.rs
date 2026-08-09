use std::fmt;
use std::io::Cursor;
use std::time::Instant;

use image_webp::WebPDecoder;

use crate::limits::{
    MAX_ANCILLARY_BYTES, MAX_CANDIDATE_BYTES, MAX_CHUNK_BYTES, MAX_CHUNKS, MAX_HEIGHT, MAX_PIXELS,
    MAX_RECONSTRUCTED_BYTES, MAX_SOURCE_BYTES, MAX_WIDTH, VALIDATION_TIMEOUT,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedWebp {
    encoded_bytes: usize,
    width: u32,
    height: u32,
}

impl ValidatedWebp {
    pub const fn encoded_bytes(self) -> usize {
        self.encoded_bytes
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ValidationError {
    message: &'static str,
}

impl ValidationError {
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for ValidationError {}

pub fn validate_source(bytes: &[u8]) -> Result<ValidatedWebp, ValidationError> {
    validate(bytes, MAX_SOURCE_BYTES)
}

pub fn validate_candidate(
    source: &ValidatedWebp,
    bytes: &[u8],
) -> Result<ValidatedWebp, ValidationError> {
    let candidate = validate(bytes, MAX_CANDIDATE_BYTES)?;
    if source.width != candidate.width || source.height != candidate.height {
        return failure("candidate changes WebP dimensions");
    }
    Ok(candidate)
}

fn validate(bytes: &[u8], maximum_encoded_bytes: u64) -> Result<ValidatedWebp, ValidationError> {
    let started = Instant::now();
    if bytes.len() as u64 > maximum_encoded_bytes {
        return failure("WebP exceeds the encoded-byte limit");
    }
    inspect_container(bytes, started)?;
    let mut decoder =
        WebPDecoder::new(Cursor::new(bytes)).map_err(|_| error("WebP image data is invalid"))?;
    decoder.set_memory_limit(MAX_RECONSTRUCTED_BYTES);
    if decoder.is_animated() {
        return failure("animated WebP is not supported");
    }
    let (width, height) = decoder.dimensions();
    validate_dimensions(width, height)?;
    let decoded_bytes = decoder
        .output_buffer_size()
        .ok_or(error("WebP decoded size overflows"))?;
    if decoded_bytes > MAX_RECONSTRUCTED_BYTES {
        return failure("WebP exceeds the reconstructed-byte limit");
    }
    let mut decoded = vec![0; decoded_bytes];
    decoder
        .read_image(&mut decoded)
        .map_err(|_| error("WebP image data is invalid"))?;
    check_time(started)?;
    Ok(ValidatedWebp {
        encoded_bytes: bytes.len(),
        width,
        height,
    })
}

fn inspect_container(bytes: &[u8], started: Instant) -> Result<(), ValidationError> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return failure("invalid WebP signature");
    }
    let declared = usize::try_from(u32::from_le_bytes(bytes[4..8].try_into().unwrap()))
        .map_err(|_| error("WebP container size overflows"))?
        .checked_add(8)
        .ok_or(error("WebP container size overflows"))?;
    if declared != bytes.len() {
        return failure("WebP container size does not match the file");
    }

    let mut position = 12usize;
    let mut chunks = 0usize;
    let mut ancillary = 0usize;
    while position < bytes.len() {
        check_time(started)?;
        if chunks == MAX_CHUNKS || position.checked_add(8).is_none_or(|end| end > bytes.len()) {
            return failure("truncated WebP chunk header");
        }
        chunks += 1;
        let name: &[u8; 4] = bytes[position..position + 4].try_into().unwrap();
        let size = usize::try_from(u32::from_le_bytes(
            bytes[position + 4..position + 8].try_into().unwrap(),
        ))
        .map_err(|_| error("WebP chunk size overflows"))?;
        if size > MAX_CHUNK_BYTES {
            return failure("WebP exceeds the chunk-byte limit");
        }
        let data_end = position
            .checked_add(8)
            .and_then(|start| start.checked_add(size))
            .ok_or(error("WebP chunk size overflows"))?;
        let padded_end = data_end
            .checked_add(size & 1)
            .ok_or(error("WebP chunk size overflows"))?;
        if padded_end > bytes.len() {
            return failure("truncated WebP chunk");
        }
        match name {
            b"ANIM" | b"ANMF" => return failure("animated WebP is not supported"),
            b"XMP " => return failure("XMP-bearing WebP is not supported"),
            b"C2PA" => return failure("C2PA-bearing WebP is not supported"),
            b"VP8 " | b"VP8L" | b"ALPH" => {}
            _ => {
                ancillary = ancillary
                    .checked_add(size)
                    .ok_or(error("WebP ancillary size overflows"))?;
                if ancillary > MAX_ANCILLARY_BYTES {
                    return failure("WebP exceeds the ancillary-byte limit");
                }
            }
        }
        position = padded_end;
    }
    if chunks == 0 {
        return failure("WebP has no chunks");
    }
    Ok(())
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), ValidationError> {
    if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
        return failure("WebP dimensions are outside the accepted limits");
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(error("WebP pixel count overflows"))?;
    if pixels > MAX_PIXELS {
        return failure("WebP exceeds the pixel limit");
    }
    Ok(())
}

fn check_time(started: Instant) -> Result<(), ValidationError> {
    if started.elapsed() > VALIDATION_TIMEOUT {
        failure("WebP validation exceeded the elapsed-time limit")
    } else {
        Ok(())
    }
}

const fn error(message: &'static str) -> ValidationError {
    ValidationError { message }
}

fn failure<T>(message: &'static str) -> Result<T, ValidationError> {
    Err(error(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASELINE: &[u8] =
        include_bytes!("../tests/corpus/webp/v1/accepted/provider-reduction.webp");

    #[test]
    fn validates_static_webp_and_rejects_changed_dimensions() {
        let source = validate_source(BASELINE).unwrap();
        assert_eq!(source.encoded_bytes(), BASELINE.len());
        validate_source(include_bytes!(
            "../tests/corpus/webp/v1/accepted/metadata.webp"
        ))
        .unwrap();
        assert_eq!(
            validate_candidate(
                &source,
                include_bytes!("../tests/corpus/webp/v1/changed/dimensions.webp")
            )
            .unwrap_err()
            .message(),
            "candidate changes WebP dimensions"
        );
    }

    #[test]
    fn rejects_animation_xmp_c2pa_trailing_and_truncated_data() {
        for bytes in [
            include_bytes!("../tests/corpus/webp/v1/rejected/animated.webp").as_slice(),
            include_bytes!("../tests/corpus/webp/v1/rejected/xmp.webp").as_slice(),
            include_bytes!("../tests/corpus/webp/v1/rejected/c2pa.webp").as_slice(),
            include_bytes!("../tests/corpus/webp/v1/rejected/trailing.webp").as_slice(),
            include_bytes!("../tests/corpus/webp/v1/rejected/truncated.webp").as_slice(),
        ] {
            assert!(validate_source(bytes).is_err());
        }
    }

    #[test]
    fn every_truncation_is_rejected() {
        for length in 0..BASELINE.len() {
            assert!(
                validate_source(&BASELINE[..length]).is_err(),
                "length {length}"
            );
        }
    }
}
