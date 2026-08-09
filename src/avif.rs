use std::fmt;
use std::time::Instant;

use crate::limits::{
    MAX_ANCILLARY_BYTES, MAX_AVIF_DIMENSION, MAX_CANDIDATE_BYTES, MAX_CHUNK_BYTES, MAX_CHUNKS,
    MAX_PIXELS, MAX_RECONSTRUCTED_BYTES, MAX_SOURCE_BYTES, VALIDATION_TIMEOUT,
};

const C2PA_UUID: &[u8; 16] = &[
    0xd8, 0xfe, 0xc3, 0xd6, 0x1b, 0x0e, 0x48, 0x3c, 0x92, 0x97, 0x58, 0x28, 0x87, 0x7e, 0xc4, 0x81,
];
const XMP_MIME: &[u8] = b"application/rdf+xml";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedAvif {
    encoded_bytes: usize,
    width: u32,
    height: u32,
}

impl ValidatedAvif {
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

pub fn validate_source(bytes: &[u8]) -> Result<ValidatedAvif, ValidationError> {
    validate(bytes, MAX_SOURCE_BYTES)
}

pub fn validate_candidate(
    source: &ValidatedAvif,
    bytes: &[u8],
) -> Result<ValidatedAvif, ValidationError> {
    let candidate = validate(bytes, MAX_CANDIDATE_BYTES)?;
    if source.width != candidate.width || source.height != candidate.height {
        return failure("candidate changes AVIF dimensions");
    }
    Ok(candidate)
}

fn validate(bytes: &[u8], maximum_encoded_bytes: u64) -> Result<ValidatedAvif, ValidationError> {
    let started = Instant::now();
    if bytes.len() as u64 > maximum_encoded_bytes {
        return failure("AVIF exceeds the encoded-byte limit");
    }
    inspect_container(bytes, started)?;
    let decoded = libavif::decode_rgb(bytes).map_err(|_| error("AVIF image data is invalid"))?;
    let (width, height) = (decoded.width(), decoded.height());
    validate_dimensions(width, height)?;
    if decoded.as_slice().len() > MAX_RECONSTRUCTED_BYTES {
        return failure("AVIF exceeds the reconstructed-byte limit");
    }
    check_time(started)?;
    Ok(ValidatedAvif {
        encoded_bytes: bytes.len(),
        width,
        height,
    })
}

fn inspect_container(bytes: &[u8], started: Instant) -> Result<(), ValidationError> {
    if bytes
        .windows(XMP_MIME.len())
        .any(|window| window == XMP_MIME)
    {
        return failure("XMP-bearing AVIF is not supported");
    }
    let mut position = 0usize;
    let mut boxes = 0usize;
    let mut ancillary = 0usize;
    let mut found_ftyp = false;
    while position < bytes.len() {
        check_time(started)?;
        if boxes == MAX_CHUNKS || position.checked_add(8).is_none_or(|end| end > bytes.len()) {
            return failure("truncated AVIF box header");
        }
        boxes += 1;
        let size32 = u32::from_be_bytes(bytes[position..position + 4].try_into().unwrap());
        let name: &[u8; 4] = bytes[position + 4..position + 8].try_into().unwrap();
        let (header, size) = if size32 == 1 {
            if position.checked_add(16).is_none_or(|end| end > bytes.len()) {
                return failure("truncated AVIF extended box header");
            }
            let size = u64::from_be_bytes(bytes[position + 8..position + 16].try_into().unwrap());
            (
                16usize,
                usize::try_from(size).map_err(|_| error("AVIF box size overflows"))?,
            )
        } else if size32 == 0 {
            (8usize, bytes.len() - position)
        } else {
            (
                8usize,
                usize::try_from(size32).map_err(|_| error("AVIF box size overflows"))?,
            )
        };
        if size < header || size > MAX_CHUNK_BYTES {
            return failure("AVIF box size is outside the accepted limits");
        }
        let end = position
            .checked_add(size)
            .ok_or(error("AVIF box size overflows"))?;
        if end > bytes.len() {
            return failure("truncated AVIF box");
        }
        let payload = &bytes[position + header..end];
        match name {
            b"ftyp" => {
                if found_ftyp || payload.len() < 8 {
                    return failure("invalid AVIF file-type box");
                }
                found_ftyp = true;
                let mut brands = std::iter::once(&payload[..4]).chain(payload[8..].chunks_exact(4));
                let mut has_avif = false;
                for brand in brands.by_ref() {
                    if brand == b"avis" {
                        return failure("animated AVIF is not supported");
                    }
                    has_avif |= brand == b"avif";
                }
                if !has_avif || !payload[8..].chunks_exact(4).remainder().is_empty() {
                    return failure("AVIF file-type box has unsupported brands");
                }
            }
            b"moov" => return failure("animated AVIF is not supported"),
            b"uuid" if payload.starts_with(C2PA_UUID) => {
                return failure("C2PA-bearing AVIF is not supported");
            }
            b"mdat" => {}
            _ => {
                ancillary = ancillary
                    .checked_add(payload.len())
                    .ok_or(error("AVIF ancillary size overflows"))?;
                if ancillary > MAX_ANCILLARY_BYTES {
                    return failure("AVIF exceeds the ancillary-byte limit");
                }
            }
        }
        position = end;
    }
    if boxes == 0 || !found_ftyp {
        return failure("AVIF file-type box is missing");
    }
    Ok(())
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), ValidationError> {
    if width == 0 || height == 0 || width > MAX_AVIF_DIMENSION || height > MAX_AVIF_DIMENSION {
        return failure("AVIF dimensions are outside the accepted limits");
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(error("AVIF pixel count overflows"))?;
    if pixels > MAX_PIXELS {
        return failure("AVIF exceeds the pixel limit");
    }
    let reconstructed = pixels
        .checked_mul(4)
        .ok_or(error("AVIF decoded size overflows"))?;
    if reconstructed > MAX_RECONSTRUCTED_BYTES as u64 {
        return failure("AVIF exceeds the reconstructed-byte limit");
    }
    Ok(())
}

fn check_time(started: Instant) -> Result<(), ValidationError> {
    if started.elapsed() > VALIDATION_TIMEOUT {
        failure("AVIF validation exceeded the elapsed-time limit")
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
        include_bytes!("../tests/corpus/avif/v1/accepted/provider-reduction.avif");

    #[test]
    fn validates_static_avif_and_rejects_changed_dimensions() {
        let source = validate_source(BASELINE).unwrap();
        assert_eq!(source.encoded_bytes(), BASELINE.len());
        assert_eq!(
            validate_candidate(
                &source,
                include_bytes!("../tests/corpus/avif/v1/changed/dimensions.avif")
            )
            .unwrap_err()
            .message(),
            "candidate changes AVIF dimensions"
        );
    }

    #[test]
    fn rejects_xmp_c2pa_trailing_and_truncated_data() {
        for bytes in [
            include_bytes!("../tests/corpus/avif/v1/rejected/xmp.avif").as_slice(),
            include_bytes!("../tests/corpus/avif/v1/rejected/c2pa.avif").as_slice(),
            include_bytes!("../tests/corpus/avif/v1/rejected/trailing.avif").as_slice(),
            include_bytes!("../tests/corpus/avif/v1/rejected/truncated.avif").as_slice(),
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
