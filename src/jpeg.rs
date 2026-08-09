use std::fmt;
use std::io::Cursor;
use std::time::Instant;

use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::options::DecoderOptions;

use crate::limits::{
    MAX_ANCILLARY_BYTES, MAX_CANDIDATE_BYTES, MAX_CHUNKS, MAX_HEIGHT, MAX_PIXELS,
    MAX_RECONSTRUCTED_BYTES, MAX_SOURCE_BYTES, MAX_WIDTH, VALIDATION_TIMEOUT,
};

const XMP: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const EXTENDED_XMP: &[u8] = b"http://ns.adobe.com/xmp/extension/\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedJpeg {
    encoded_bytes: usize,
    width: u32,
    height: u32,
}

impl ValidatedJpeg {
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

pub fn validate_source(bytes: &[u8]) -> Result<ValidatedJpeg, ValidationError> {
    validate(bytes, MAX_SOURCE_BYTES)
}

pub fn validate_candidate(
    source: &ValidatedJpeg,
    bytes: &[u8],
) -> Result<ValidatedJpeg, ValidationError> {
    let candidate = validate(bytes, MAX_CANDIDATE_BYTES)?;
    if source.width != candidate.width || source.height != candidate.height {
        return failure("candidate changes JPEG dimensions");
    }
    Ok(candidate)
}

fn validate(bytes: &[u8], maximum_encoded_bytes: u64) -> Result<ValidatedJpeg, ValidationError> {
    let started = Instant::now();
    if u64::try_from(bytes.len()).map_or(true, |length| length > maximum_encoded_bytes) {
        return failure("JPEG exceeds the encoded-byte limit");
    }
    let dimensions = inspect_container(bytes, started)?;
    let pixels = u64::from(dimensions.0)
        .checked_mul(u64::from(dimensions.1))
        .ok_or(error("JPEG pixel count overflows"))?;
    if pixels > MAX_PIXELS {
        return failure("JPEG exceeds the pixel limit");
    }
    let reconstructed = pixels
        .checked_mul(4)
        .ok_or(error("JPEG decoded size overflows"))?;
    if reconstructed > MAX_RECONSTRUCTED_BYTES as u64 {
        return failure("JPEG exceeds the reconstructed-byte limit");
    }
    check_time(started)?;
    let options = DecoderOptions::default()
        .set_strict_mode(true)
        .set_max_width(MAX_WIDTH as usize)
        .set_max_height(MAX_HEIGHT as usize)
        .jpeg_set_max_scans(MAX_CHUNKS);
    let mut decoder = JpegDecoder::new_with_options(Cursor::new(bytes), options);
    let decoded = decoder
        .decode()
        .map_err(|_| error("JPEG image data is invalid"))?;
    if decoded.len() > MAX_RECONSTRUCTED_BYTES {
        return failure("JPEG exceeds the reconstructed-byte limit");
    }
    check_time(started)?;
    let decoded_dimensions = decoder
        .dimensions()
        .ok_or(error("JPEG dimensions are missing after decode"))?;
    if decoded_dimensions != (dimensions.0 as usize, dimensions.1 as usize) {
        return failure("JPEG decoded dimensions disagree with its frame header");
    }
    Ok(ValidatedJpeg {
        encoded_bytes: bytes.len(),
        width: dimensions.0,
        height: dimensions.1,
    })
}

fn inspect_container(bytes: &[u8], started: Instant) -> Result<(u32, u32), ValidationError> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return failure("invalid JPEG signature");
    }
    let mut position = 2usize;
    let mut segments = 0usize;
    let mut ancillary_bytes = 0usize;
    let mut dimensions = None;
    let mut in_scan = false;

    while position < bytes.len() {
        check_time(started)?;
        if in_scan {
            position = next_scan_marker(bytes, position)?;
        }
        if bytes.get(position) != Some(&0xff) {
            return failure("JPEG marker prefix is missing");
        }
        while bytes.get(position) == Some(&0xff) {
            position += 1;
        }
        let marker = *bytes.get(position).ok_or(error("truncated JPEG marker"))?;
        position += 1;
        if marker == 0x00 {
            return failure("unexpected stuffed JPEG marker");
        }
        if marker == 0xd9 {
            if position != bytes.len() {
                return failure("JPEG has trailing bytes");
            }
            return dimensions.ok_or(error("JPEG frame header is missing"));
        }
        if matches!(marker, 0xd0..=0xd7 | 0x01) {
            in_scan = true;
            continue;
        }
        if marker == 0xd8 {
            return failure("JPEG contains an unexpected SOI marker");
        }
        if segments == MAX_CHUNKS {
            return failure("JPEG exceeds the segment-count limit");
        }
        segments += 1;
        let length_end = position
            .checked_add(2)
            .ok_or(error("JPEG segment length overflows"))?;
        if length_end > bytes.len() {
            return failure("truncated JPEG segment length");
        }
        let length = usize::from(u16::from_be_bytes([bytes[position], bytes[position + 1]]));
        if length < 2 {
            return failure("invalid JPEG segment length");
        }
        let segment_end = position
            .checked_add(length)
            .ok_or(error("JPEG segment overflows"))?;
        if segment_end > bytes.len() {
            return failure("truncated JPEG segment");
        }
        let data = &bytes[length_end..segment_end];
        if matches!(marker, 0xe0..=0xef | 0xfe) {
            ancillary_bytes = ancillary_bytes
                .checked_add(data.len())
                .ok_or(error("JPEG ancillary size overflows"))?;
            if ancillary_bytes > MAX_ANCILLARY_BYTES {
                return failure("JPEG exceeds the ancillary-byte limit");
            }
        }
        match marker {
            0xe1 if data.starts_with(XMP) || data.starts_with(EXTENDED_XMP) => {
                return failure("XMP-bearing JPEG is not supported");
            }
            0xeb => return failure("APP11-bearing JPEG is not supported"),
            0xc0..=0xcf if !matches!(marker, 0xc4 | 0xc8 | 0xcc) => {
                if !matches!(marker, 0xc0..=0xc2) {
                    return failure("unsupported JPEG frame type");
                }
                if dimensions.is_some() {
                    return failure("JPEG contains multiple frame headers");
                }
                dimensions = Some(frame_dimensions(data)?);
            }
            0xda => in_scan = true,
            _ => {}
        }
        position = segment_end;
    }
    failure("JPEG is missing EOI")
}

fn next_scan_marker(bytes: &[u8], mut position: usize) -> Result<usize, ValidationError> {
    while position < bytes.len() {
        let Some(offset) = bytes[position..].iter().position(|byte| *byte == 0xff) else {
            return failure("JPEG is missing EOI");
        };
        position += offset;
        let marker_start = position;
        while bytes.get(position) == Some(&0xff) {
            position += 1;
        }
        match bytes.get(position).copied() {
            Some(0x00) => position += 1,
            Some(0xd0..=0xd7) => position += 1,
            Some(_) => return Ok(marker_start),
            None => return failure("truncated JPEG entropy marker"),
        }
    }
    failure("JPEG is missing EOI")
}

fn frame_dimensions(data: &[u8]) -> Result<(u32, u32), ValidationError> {
    if data.len() < 6 || data[0] != 8 {
        return failure("JPEG frame precision is unsupported");
    }
    let height = u32::from(u16::from_be_bytes([data[1], data[2]]));
    let width = u32::from(u16::from_be_bytes([data[3], data[4]]));
    let components = data[5];
    if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
        return failure("JPEG dimensions are outside the accepted limits");
    }
    if !matches!(components, 1 | 3 | 4) || data.len() != 6 + usize::from(components) * 3 {
        return failure("JPEG frame components are unsupported");
    }
    Ok((width, height))
}

fn check_time(started: Instant) -> Result<(), ValidationError> {
    if started.elapsed() > VALIDATION_TIMEOUT {
        failure("JPEG validation exceeded the elapsed-time limit")
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

    const BASELINE: &[u8] = include_bytes!("../tests/corpus/jpeg/v1/accepted/baseline.jpg");
    const PROGRESSIVE: &[u8] = include_bytes!("../tests/corpus/jpeg/v1/accepted/progressive.jpg");

    #[test]
    fn validates_baseline_progressive_grayscale_and_provider_fixture() {
        for bytes in [
            BASELINE,
            PROGRESSIVE,
            include_bytes!("../tests/corpus/jpeg/v1/accepted/grayscale.jpg"),
            include_bytes!("../tests/corpus/jpeg/v1/accepted/provider-reduction.jpg"),
        ] {
            validate_source(bytes).unwrap();
        }
    }

    #[test]
    fn accepts_changed_encoding_but_rejects_changed_dimensions() {
        let source = validate_source(BASELINE).unwrap();
        validate_candidate(&source, PROGRESSIVE).unwrap();
        assert_eq!(
            validate_candidate(
                &source,
                include_bytes!("../tests/corpus/jpeg/v1/changed/dimensions.jpg")
            )
            .unwrap_err()
            .message(),
            "candidate changes JPEG dimensions"
        );
    }

    #[test]
    fn rejects_xmp_app11_trailing_truncated_and_invalid_data() {
        for bytes in [
            include_bytes!("../tests/corpus/jpeg/v1/rejected/xmp.jpg").as_slice(),
            include_bytes!("../tests/corpus/jpeg/v1/rejected/app11.jpg").as_slice(),
            include_bytes!("../tests/corpus/jpeg/v1/rejected/trailing.jpg").as_slice(),
            include_bytes!("../tests/corpus/jpeg/v1/rejected/truncated.jpg").as_slice(),
            include_bytes!("../tests/corpus/jpeg/v1/rejected/invalid-scan.jpg").as_slice(),
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
