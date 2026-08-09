use std::fmt;
use std::time::Instant;

use flate2::{Decompress, FlushDecompress, Status};

use crate::limits::{
    MAX_ANCILLARY_BYTES, MAX_CANDIDATE_BYTES, MAX_CHUNK_BYTES, MAX_CHUNKS, MAX_HEIGHT, MAX_PIXELS,
    MAX_RECONSTRUCTED_BYTES, MAX_SOURCE_BYTES, MAX_WIDTH, VALIDATION_TIMEOUT,
};

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const SRGB_CHROMATICITIES: [u32; 8] = [
    31_270, 32_900, 64_000, 33_000, 30_000, 60_000, 15_000, 6_000,
];

#[derive(Debug)]
pub struct ValidatedPng {
    encoded_bytes: usize,
    ihdr: Vec<u8>,
    plte: Option<Vec<u8>>,
    ancillary_before_idat: Vec<Vec<u8>>,
    ancillary_after_idat: Vec<Vec<u8>>,
    samples: Vec<u8>,
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
    if source.ihdr != candidate.ihdr
        || source.plte != candidate.plte
        || source.ancillary_before_idat != candidate.ancillary_before_idat
        || source.ancillary_after_idat != candidate.ancillary_after_idat
        || source.samples != candidate.samples
    {
        return failure("candidate changes protected PNG content");
    }
    Ok(candidate)
}

fn validate(bytes: &[u8], maximum_encoded_bytes: u64) -> Result<ValidatedPng, ValidationError> {
    let started = Instant::now();
    if u64::try_from(bytes.len()).map_or(true, |length| length > maximum_encoded_bytes) {
        return failure("PNG exceeds the encoded-byte limit");
    }
    if !bytes.starts_with(SIGNATURE) {
        return failure("invalid PNG signature");
    }

    let mut state = ParseState::new();
    let mut position = SIGNATURE.len();
    while position < bytes.len() {
        check_time(started)?;
        if state.chunk_count == MAX_CHUNKS {
            return failure("PNG exceeds the chunk-count limit");
        }
        state.chunk_count += 1;

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
        validate_chunk_name(chunk_type)?;
        let data = &bytes[header_end..header_end + length];
        let stored_crc = u32::from_be_bytes(
            bytes[header_end + length..chunk_end]
                .try_into()
                .map_err(|_| error("invalid PNG chunk CRC"))?,
        );
        let mut crc = crc32fast::Hasher::new();
        crc.update(&chunk_type);
        crc.update(data);
        if crc.finalize() != stored_crc {
            return failure("PNG chunk CRC mismatch");
        }

        let serialized = bytes[position..chunk_end].to_vec();
        state.accept_chunk(chunk_type, data, serialized)?;
        position = chunk_end;
        if chunk_type == *b"IEND" {
            break;
        }
    }

    if position != bytes.len() {
        return failure("PNG has trailing bytes");
    }
    state.finish(started, bytes.len())
}

#[derive(Clone, Copy)]
struct Header {
    width: u32,
    height: u32,
    bit_depth: u8,
    color_type: u8,
    row_bytes: usize,
    filter_bytes_per_pixel: usize,
}

struct ParseState {
    chunk_count: usize,
    header: Option<Header>,
    ihdr: Option<Vec<u8>>,
    plte: Option<Vec<u8>>,
    palette_entries: Option<usize>,
    idat: Vec<u8>,
    seen_idat: bool,
    idat_ended: bool,
    seen_iend: bool,
    ancillary_before_idat: Vec<Vec<u8>>,
    ancillary_after_idat: Vec<Vec<u8>>,
    ancillary_bytes: usize,
    seen_trns: bool,
    seen_chrm: bool,
    seen_gama: bool,
    seen_sbit: bool,
    seen_srgb: bool,
    seen_bkgd: bool,
    seen_phys: bool,
    seen_time: bool,
    gama: Option<u32>,
    chrm: Option<[u32; 8]>,
}

impl ParseState {
    fn new() -> Self {
        Self {
            chunk_count: 0,
            header: None,
            ihdr: None,
            plte: None,
            palette_entries: None,
            idat: Vec::new(),
            seen_idat: false,
            idat_ended: false,
            seen_iend: false,
            ancillary_before_idat: Vec::new(),
            ancillary_after_idat: Vec::new(),
            ancillary_bytes: 0,
            seen_trns: false,
            seen_chrm: false,
            seen_gama: false,
            seen_sbit: false,
            seen_srgb: false,
            seen_bkgd: false,
            seen_phys: false,
            seen_time: false,
            gama: None,
            chrm: None,
        }
    }

    fn accept_chunk(
        &mut self,
        chunk_type: [u8; 4],
        data: &[u8],
        serialized: Vec<u8>,
    ) -> Result<(), ValidationError> {
        if self.seen_iend {
            return failure("chunk appears after IEND");
        }
        if self.seen_idat && chunk_type != *b"IDAT" {
            self.idat_ended = true;
        }

        match &chunk_type {
            b"IHDR" => self.accept_ihdr(data, serialized),
            b"PLTE" => self.accept_plte(data, serialized),
            b"IDAT" => self.accept_idat(data),
            b"IEND" => self.accept_iend(data),
            b"tRNS" | b"cHRM" | b"gAMA" | b"sBIT" | b"sRGB" | b"tEXt" | b"bKGD" | b"pHYs"
            | b"tIME" => self.accept_ancillary(chunk_type, data, serialized),
            b"acTL" | b"fcTL" | b"fdAT" => failure("APNG is not supported"),
            b"iCCP" | b"zTXt" | b"iTXt" | b"eXIf" | b"caBX" => {
                failure("PNG chunk is refused by the version 0.1 policy")
            }
            _ => failure("PNG chunk is outside the accepted version 0.1 subset"),
        }
    }

    fn accept_ihdr(&mut self, data: &[u8], serialized: Vec<u8>) -> Result<(), ValidationError> {
        if self.chunk_count != 1 || self.header.is_some() || data.len() != 13 {
            return failure("invalid IHDR");
        }
        let width = read_u32(&data[0..4])?;
        let height = read_u32(&data[4..8])?;
        let bit_depth = data[8];
        let color_type = data[9];
        if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
            return failure("PNG dimensions are outside the accepted limits");
        }
        let channels = match (color_type, bit_depth) {
            (0, 8) => 1usize,
            (2, 8) => 3,
            (3, 1 | 2 | 4 | 8) => 1,
            (4, 8) => 2,
            (6, 8) => 4,
            _ => return failure("unsupported PNG color type or bit depth"),
        };
        if data[10] != 0 || data[11] != 0 || data[12] != 0 {
            return failure("unsupported PNG compression, filter, or interlace method");
        }
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or(error("PNG pixel count overflows"))?;
        if pixels > MAX_PIXELS {
            return failure("PNG exceeds the pixel limit");
        }
        let bits_per_row = usize::try_from(width)
            .ok()
            .and_then(|value| value.checked_mul(channels))
            .and_then(|value| value.checked_mul(usize::from(bit_depth)))
            .ok_or(error("PNG row size overflows"))?;
        let row_bytes = bits_per_row
            .checked_add(7)
            .ok_or(error("PNG row size overflows"))?
            / 8;
        let reconstructed = row_bytes
            .checked_mul(usize::try_from(height).map_err(|_| error("PNG height is unsupported"))?)
            .ok_or(error("PNG reconstructed size overflows"))?;
        if reconstructed > MAX_RECONSTRUCTED_BYTES {
            return failure("PNG exceeds the reconstructed-byte limit");
        }
        let filter_bytes_per_pixel = channels
            .checked_mul(usize::from(bit_depth))
            .and_then(|bits| bits.checked_add(7))
            .ok_or(error("PNG filter width overflows"))?
            / 8;
        self.header = Some(Header {
            width,
            height,
            bit_depth,
            color_type,
            row_bytes,
            filter_bytes_per_pixel: filter_bytes_per_pixel.max(1),
        });
        self.ihdr = Some(serialized);
        Ok(())
    }

    fn accept_plte(&mut self, data: &[u8], serialized: Vec<u8>) -> Result<(), ValidationError> {
        let header = self.required_header()?;
        if self.plte.is_some() || self.seen_idat || matches!(header.color_type, 0 | 4) {
            return failure("invalid PLTE placement or multiplicity");
        }
        if data.is_empty() || !data.len().is_multiple_of(3) || data.len() > 256 * 3 {
            return failure("invalid PLTE length");
        }
        let entries = data.len() / 3;
        if header.color_type == 3 && entries > 1usize << header.bit_depth {
            return failure("PLTE has too many entries for indexed bit depth");
        }
        self.palette_entries = Some(entries);
        self.plte = Some(serialized);
        Ok(())
    }

    fn accept_idat(&mut self, data: &[u8]) -> Result<(), ValidationError> {
        let header = self.required_header()?;
        if self.idat_ended {
            return failure("IDAT chunks are not consecutive");
        }
        if header.color_type == 3 && self.plte.is_none() {
            return failure("indexed PNG is missing PLTE before IDAT");
        }
        self.seen_idat = true;
        let combined = self
            .idat
            .len()
            .checked_add(data.len())
            .ok_or(error("IDAT size overflows"))?;
        if combined > MAX_SOURCE_BYTES as usize {
            return failure("IDAT data exceeds the encoded-byte limit");
        }
        self.idat.extend_from_slice(data);
        Ok(())
    }

    fn accept_iend(&mut self, data: &[u8]) -> Result<(), ValidationError> {
        self.required_header()?;
        if !data.is_empty() || !self.seen_idat || self.seen_iend {
            return failure("invalid IEND");
        }
        self.seen_iend = true;
        Ok(())
    }

    fn accept_ancillary(
        &mut self,
        chunk_type: [u8; 4],
        data: &[u8],
        serialized: Vec<u8>,
    ) -> Result<(), ValidationError> {
        let header = self.required_header()?;
        self.ancillary_bytes = self
            .ancillary_bytes
            .checked_add(data.len())
            .ok_or(error("ancillary byte count overflows"))?;
        if self.ancillary_bytes > MAX_ANCILLARY_BYTES {
            return failure("PNG exceeds the ancillary-byte limit");
        }

        match &chunk_type {
            b"tRNS" => self.validate_trns(header, data)?,
            b"cHRM" => self.validate_chrm(data)?,
            b"gAMA" => self.validate_gama(data)?,
            b"sBIT" => self.validate_sbit(header, data)?,
            b"sRGB" => self.validate_srgb(data)?,
            b"tEXt" => validate_text(data)?,
            b"bKGD" => self.validate_bkgd(header, data)?,
            b"pHYs" => self.validate_phys(data)?,
            b"tIME" => self.validate_time(data)?,
            _ => return failure("internal ancillary validation error"),
        }

        if self.seen_idat {
            self.ancillary_after_idat.push(serialized);
        } else {
            self.ancillary_before_idat.push(serialized);
        }
        Ok(())
    }

    fn validate_trns(&mut self, header: Header, data: &[u8]) -> Result<(), ValidationError> {
        take_once(&mut self.seen_trns, "duplicate tRNS")?;
        if self.seen_idat {
            return failure("tRNS must precede IDAT");
        }
        match header.color_type {
            0 => validate_sample_values(data, 1, header.bit_depth),
            2 => validate_sample_values(data, 3, header.bit_depth),
            3 => {
                let Some(entries) = self.palette_entries else {
                    return failure("indexed tRNS must follow PLTE");
                };
                if data.is_empty() || data.len() > entries {
                    return failure("invalid indexed tRNS length");
                }
                Ok(())
            }
            _ => failure("tRNS is not allowed for this color type"),
        }
    }

    fn validate_chrm(&mut self, data: &[u8]) -> Result<(), ValidationError> {
        take_once(&mut self.seen_chrm, "duplicate cHRM")?;
        self.require_before_palette_and_idat("cHRM")?;
        if data.len() != 32 {
            return failure("invalid cHRM length");
        }
        let mut values = [0u32; 8];
        for (index, value) in values.iter_mut().enumerate() {
            *value = read_u32(&data[index * 4..index * 4 + 4])?;
        }
        self.chrm = Some(values);
        Ok(())
    }

    fn validate_gama(&mut self, data: &[u8]) -> Result<(), ValidationError> {
        take_once(&mut self.seen_gama, "duplicate gAMA")?;
        self.require_before_palette_and_idat("gAMA")?;
        if data.len() != 4 {
            return failure("invalid gAMA length");
        }
        let value = read_u32(data)?;
        if value == 0 {
            return failure("gAMA value must be nonzero");
        }
        self.gama = Some(value);
        Ok(())
    }

    fn validate_sbit(&mut self, header: Header, data: &[u8]) -> Result<(), ValidationError> {
        take_once(&mut self.seen_sbit, "duplicate sBIT")?;
        self.require_before_palette_and_idat("sBIT")?;
        let (length, maximum) = match header.color_type {
            0 => (1, header.bit_depth),
            2 => (3, header.bit_depth),
            3 => (3, 8),
            4 => (2, header.bit_depth),
            6 => (4, header.bit_depth),
            _ => return failure("invalid color type for sBIT"),
        };
        if data.len() != length || data.iter().any(|value| *value == 0 || *value > maximum) {
            return failure("invalid sBIT values");
        }
        Ok(())
    }

    fn validate_srgb(&mut self, data: &[u8]) -> Result<(), ValidationError> {
        take_once(&mut self.seen_srgb, "duplicate sRGB")?;
        self.require_before_palette_and_idat("sRGB")?;
        if data.len() != 1 || data[0] > 3 {
            return failure("invalid sRGB rendering intent");
        }
        Ok(())
    }

    fn validate_bkgd(&mut self, header: Header, data: &[u8]) -> Result<(), ValidationError> {
        take_once(&mut self.seen_bkgd, "duplicate bKGD")?;
        if self.seen_idat {
            return failure("bKGD must precede IDAT");
        }
        match header.color_type {
            0 | 4 => validate_sample_values(data, 1, header.bit_depth),
            2 | 6 => validate_sample_values(data, 3, header.bit_depth),
            3 => {
                let Some(entries) = self.palette_entries else {
                    return failure("indexed bKGD must follow PLTE");
                };
                if data.len() != 1 || usize::from(data[0]) >= entries {
                    return failure("invalid indexed bKGD value");
                }
                Ok(())
            }
            _ => failure("invalid color type for bKGD"),
        }
    }

    fn validate_phys(&mut self, data: &[u8]) -> Result<(), ValidationError> {
        take_once(&mut self.seen_phys, "duplicate pHYs")?;
        if self.seen_idat || data.len() != 9 || data[8] > 1 {
            return failure("invalid pHYs");
        }
        Ok(())
    }

    fn validate_time(&mut self, data: &[u8]) -> Result<(), ValidationError> {
        take_once(&mut self.seen_time, "duplicate tIME")?;
        if data.len() != 7 {
            return failure("invalid tIME length");
        }
        let month = data[2];
        let day = data[3];
        if !(1..=12).contains(&month)
            || !(1..=31).contains(&day)
            || data[4] > 23
            || data[5] > 59
            || data[6] > 60
        {
            return failure("invalid tIME value");
        }
        Ok(())
    }

    fn require_before_palette_and_idat(&self, name: &'static str) -> Result<(), ValidationError> {
        if self.plte.is_some() || self.seen_idat {
            let _ = name;
            return failure("color-space chunk has invalid placement");
        }
        Ok(())
    }

    fn required_header(&self) -> Result<Header, ValidationError> {
        self.header.ok_or(error("IHDR must be the first chunk"))
    }

    fn finish(
        self,
        started: Instant,
        encoded_bytes: usize,
    ) -> Result<ValidatedPng, ValidationError> {
        check_time(started)?;
        if !self.seen_iend {
            return failure("PNG is missing IEND");
        }
        let header = self.header.ok_or(error("PNG is missing IHDR"))?;
        if header.color_type == 3 && self.plte.is_none() {
            return failure("indexed PNG is missing PLTE");
        }
        if self.seen_srgb {
            if self.gama.is_some_and(|value| value != 45_455) {
                return failure("gAMA conflicts with sRGB");
            }
            if self.chrm.is_some_and(|value| value != SRGB_CHROMATICITIES) {
                return failure("cHRM conflicts with sRGB");
            }
        }
        let filtered_bytes = header
            .row_bytes
            .checked_add(1)
            .and_then(|row| row.checked_mul(usize::try_from(header.height).ok()?))
            .ok_or(error("PNG decompressed size overflows"))?;
        if filtered_bytes > MAX_RECONSTRUCTED_BYTES {
            return failure("PNG exceeds the decompressed-byte limit");
        }
        let filtered = decompress_exact(&self.idat, filtered_bytes)?;
        let mut samples = unfilter(header, &filtered, started)?;
        if header.color_type == 3 {
            validate_palette_references(
                &mut samples,
                header,
                self.palette_entries
                    .ok_or(error("indexed PNG is missing PLTE"))?,
            )?;
        }
        Ok(ValidatedPng {
            encoded_bytes,
            ihdr: self.ihdr.ok_or(error("PNG is missing IHDR"))?,
            plte: self.plte,
            ancillary_before_idat: self.ancillary_before_idat,
            ancillary_after_idat: self.ancillary_after_idat,
            samples,
        })
    }
}

fn decompress_exact(input: &[u8], expected: usize) -> Result<Vec<u8>, ValidationError> {
    let capacity = expected
        .checked_add(1)
        .ok_or(error("PNG decompressed size overflows"))?;
    let mut output = vec![0; capacity];
    let mut decompressor = Decompress::new(true);
    let status = decompressor
        .decompress(input, &mut output, FlushDecompress::Finish)
        .map_err(|_| error("invalid PNG zlib stream"))?;
    let consumed = usize::try_from(decompressor.total_in())
        .map_err(|_| error("PNG compressed size is unsupported"))?;
    let produced = usize::try_from(decompressor.total_out())
        .map_err(|_| error("PNG decompressed size is unsupported"))?;
    if status != Status::StreamEnd || consumed != input.len() || produced != expected {
        return failure("PNG zlib stream has an invalid length or trailing data");
    }
    output.truncate(expected);
    Ok(output)
}

fn unfilter(header: Header, filtered: &[u8], started: Instant) -> Result<Vec<u8>, ValidationError> {
    let height = usize::try_from(header.height).map_err(|_| error("PNG height is unsupported"))?;
    let sample_bytes = header
        .row_bytes
        .checked_mul(height)
        .ok_or(error("PNG reconstructed size overflows"))?;
    let mut samples = vec![0u8; sample_bytes];
    let mut input_position = 0usize;

    for row in 0..height {
        check_time(started)?;
        let filter = filtered[input_position];
        input_position += 1;
        if filter > 4 {
            return failure("invalid PNG scanline filter");
        }
        let row_start = row * header.row_bytes;
        for column in 0..header.row_bytes {
            let encoded = filtered[input_position];
            input_position += 1;
            let left = if column >= header.filter_bytes_per_pixel {
                samples[row_start + column - header.filter_bytes_per_pixel]
            } else {
                0
            };
            let above = if row > 0 {
                samples[row_start + column - header.row_bytes]
            } else {
                0
            };
            let upper_left = if row > 0 && column >= header.filter_bytes_per_pixel {
                samples[row_start + column - header.row_bytes - header.filter_bytes_per_pixel]
            } else {
                0
            };
            let predictor = match filter {
                0 => 0,
                1 => left,
                2 => above,
                3 => ((u16::from(left) + u16::from(above)) / 2) as u8,
                4 => paeth(left, above, upper_left),
                _ => return failure("invalid PNG scanline filter"),
            };
            samples[row_start + column] = encoded.wrapping_add(predictor);
        }
    }
    Ok(samples)
}

fn validate_palette_references(
    samples: &mut [u8],
    header: Header,
    palette_entries: usize,
) -> Result<(), ValidationError> {
    let height = usize::try_from(header.height).map_err(|_| error("PNG height is unsupported"))?;
    let width = usize::try_from(header.width).map_err(|_| error("PNG width is unsupported"))?;
    let depth = usize::from(header.bit_depth);
    let mask = (1u16 << depth) - 1;
    for row in 0..height {
        let start = row * header.row_bytes;
        for pixel in 0..width {
            let bit = pixel * depth;
            let byte = samples[start + bit / 8];
            let shift = 8 - depth - bit % 8;
            let index = usize::from((u16::from(byte) >> shift) & mask);
            if index >= palette_entries {
                return failure("indexed pixel references a missing palette entry");
            }
        }
        let used = width * depth % 8;
        if used != 0 {
            let final_byte = start + header.row_bytes - 1;
            samples[final_byte] &= 0xFF << (8 - used);
        }
    }
    Ok(())
}

fn validate_sample_values(data: &[u8], count: usize, bit_depth: u8) -> Result<(), ValidationError> {
    if data.len() != count * 2 {
        return failure("invalid ancillary sample length");
    }
    let maximum = (1u32 << bit_depth) - 1;
    for bytes in data.chunks_exact(2) {
        if u32::from(u16::from_be_bytes([bytes[0], bytes[1]])) > maximum {
            return failure("ancillary sample exceeds the image bit depth");
        }
    }
    Ok(())
}

fn validate_text(data: &[u8]) -> Result<(), ValidationError> {
    let Some(separator) = data.iter().position(|byte| *byte == 0) else {
        return failure("tEXt is missing its keyword separator");
    };
    let keyword = &data[..separator];
    if keyword.is_empty() || keyword.len() > 79 {
        return failure("invalid tEXt keyword length");
    }
    if keyword.first() == Some(&b' ')
        || keyword.last() == Some(&b' ')
        || keyword.windows(2).any(|pair| pair == b"  ")
        || keyword
            .iter()
            .any(|byte| !matches!(*byte, 32..=126 | 161..=255))
    {
        return failure("invalid tEXt keyword");
    }
    if keyword == b"XML:com.adobe.xmp" {
        return failure("XMP tEXt is refused by the version 0.1 policy");
    }
    if data[separator + 1..]
        .iter()
        .any(|byte| !matches!(*byte, 10 | 32..=126 | 161..=255))
    {
        return failure("invalid Latin-1 byte in tEXt text");
    }
    Ok(())
}

fn validate_chunk_name(name: [u8; 4]) -> Result<(), ValidationError> {
    if name.iter().any(|byte| !byte.is_ascii_alphabetic()) || name[2].is_ascii_lowercase() {
        return failure("invalid PNG chunk type code");
    }
    Ok(())
}

fn take_once(seen: &mut bool, message: &'static str) -> Result<(), ValidationError> {
    if *seen {
        return failure(message);
    }
    *seen = true;
    Ok(())
}

fn read_u32(data: &[u8]) -> Result<u32, ValidationError> {
    Ok(u32::from_be_bytes(
        data.try_into()
            .map_err(|_| error("invalid four-byte PNG value"))?,
    ))
}

fn paeth(left: u8, above: u8, upper_left: u8) -> u8 {
    let left = i32::from(left);
    let above = i32::from(above);
    let upper_left = i32::from(upper_left);
    let estimate = left + above - upper_left;
    let left_distance = (estimate - left).abs();
    let above_distance = (estimate - above).abs();
    let upper_left_distance = (estimate - upper_left).abs();
    if left_distance <= above_distance && left_distance <= upper_left_distance {
        left as u8
    } else if above_distance <= upper_left_distance {
        above as u8
    } else {
        upper_left as u8
    }
}

fn check_time(started: Instant) -> Result<(), ValidationError> {
    if started.elapsed() > VALIDATION_TIMEOUT {
        failure("PNG validation exceeded its elapsed-time limit")
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
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    use super::*;

    #[test]
    fn validates_supported_grayscale_png() {
        let png = make_png(1, 1, 8, 0, None, &[0, 42], &[]);
        let validated = validate_source(&png).unwrap();
        assert_eq!(validated.encoded_bytes(), png.len());
    }

    #[test]
    fn validates_all_supported_color_and_depth_combinations() {
        for (depth, color, palette, row) in [
            (8, 0, None, vec![0, 7]),
            (8, 2, None, vec![0, 1, 2, 3]),
            (1, 3, Some(vec![0, 0, 0, 255, 255, 255]), vec![0, 0]),
            (2, 3, Some(vec![0; 12]), vec![0, 0]),
            (4, 3, Some(vec![0; 48]), vec![0, 0]),
            (8, 3, Some(vec![0; 3]), vec![0, 0]),
            (8, 4, None, vec![0, 5, 255]),
            (8, 6, None, vec![0, 1, 2, 3, 4]),
        ] {
            let png = make_png(1, 1, depth, color, palette.as_deref(), &row, &[]);
            validate_source(&png).unwrap();
        }
    }

    #[test]
    fn rejects_crc_trailing_data_and_unknown_chunks() {
        let mut bad_crc = make_png(1, 1, 8, 0, None, &[0, 42], &[]);
        bad_crc[20] ^= 1;
        assert_eq!(
            validate_source(&bad_crc).unwrap_err().message(),
            "PNG chunk CRC mismatch"
        );

        let mut trailing = make_png(1, 1, 8, 0, None, &[0, 42], &[]);
        trailing.push(0);
        assert_eq!(
            validate_source(&trailing).unwrap_err().message(),
            "PNG has trailing bytes"
        );

        let unknown = make_png(1, 1, 8, 0, None, &[0, 42], &[(b"vpAg", b"x")]);
        assert_eq!(
            validate_source(&unknown).unwrap_err().message(),
            "PNG chunk is outside the accepted version 0.1 subset"
        );
    }

    #[test]
    fn rejects_apng_and_xmp() {
        let apng = make_png(1, 1, 8, 0, None, &[0, 42], &[(b"acTL", &[0; 8])]);
        assert_eq!(
            validate_source(&apng).unwrap_err().message(),
            "APNG is not supported"
        );

        let xmp = make_png(
            1,
            1,
            8,
            0,
            None,
            &[0, 42],
            &[(b"tEXt", b"XML:com.adobe.xmp\0payload")],
        );
        assert_eq!(
            validate_source(&xmp).unwrap_err().message(),
            "XMP tEXt is refused by the version 0.1 policy"
        );
    }

    #[test]
    fn candidate_allows_idat_changes_but_not_samples_or_ancillary_order() {
        let ancillary = [(b"tEXt" as &[u8; 4], b"Key\0Value" as &[u8])];
        let source = make_png_with_compression(
            2,
            1,
            8,
            0,
            None,
            &[0, 42, 43],
            &ancillary,
            Compression::fast(),
        );
        let candidate = make_png_with_compression(
            2,
            1,
            8,
            0,
            None,
            &[0, 42, 43],
            &ancillary,
            Compression::best(),
        );
        let validated_source = validate_source(&source).unwrap();
        validate_candidate(&validated_source, &candidate).unwrap();

        let changed = make_png(2, 1, 8, 0, None, &[0, 42, 44], &ancillary);
        assert_eq!(
            validate_candidate(&validated_source, &changed)
                .unwrap_err()
                .message(),
            "candidate changes protected PNG content"
        );
    }

    #[test]
    fn indexed_padding_bits_do_not_affect_equivalence() {
        let palette = [0, 0, 0, 255, 255, 255];
        let source = make_png(1, 1, 1, 3, Some(&palette), &[0, 0b1000_0000], &[]);
        let candidate = make_png(1, 1, 1, 3, Some(&palette), &[0, 0b1111_1111], &[]);
        let validated_source = validate_source(&source).unwrap();
        validate_candidate(&validated_source, &candidate).unwrap();
    }

    #[test]
    fn validates_ancillary_structures_and_srgb_consistency() {
        let chrm = SRGB_CHROMATICITIES
            .iter()
            .flat_map(|value| value.to_be_bytes())
            .collect::<Vec<_>>();
        let gama = 45_455u32.to_be_bytes();
        let chunks = [
            (b"cHRM" as &[u8; 4], chrm.as_slice()),
            (b"gAMA", gama.as_slice()),
            (b"sBIT", &[8][..]),
            (b"sRGB", &[0][..]),
            (b"tRNS", &[0, 42][..]),
            (b"bKGD", &[0, 7][..]),
            (b"pHYs", &[0, 0, 0, 1, 0, 0, 0, 1, 1][..]),
            (b"tIME", &[0x07, 0xE8, 2, 29, 23, 59, 60][..]),
            (b"tEXt", b"Key\0opaque" as &[u8]),
        ];
        let png = make_png(1, 1, 8, 0, None, &[0, 1], &chunks);
        validate_source(&png).unwrap();
    }

    #[test]
    fn every_truncation_is_rejected() {
        let png = make_png(2, 1, 8, 2, None, &[0, 1, 2, 3, 4, 5, 6], &[]);
        for length in 0..png.len() {
            assert!(
                validate_source(&png[..length]).is_err(),
                "accepted length {length}"
            );
        }
    }

    #[test]
    fn checked_in_version_one_corpus_matches_its_expectations() {
        let accepted: &[&[u8]] = &[
            include_bytes!("../tests/corpus/png/v1/accepted/grayscale8.png"),
            include_bytes!("../tests/corpus/png/v1/accepted/truecolor8.png"),
            include_bytes!("../tests/corpus/png/v1/accepted/indexed1.png"),
            include_bytes!("../tests/corpus/png/v1/accepted/indexed2.png"),
            include_bytes!("../tests/corpus/png/v1/accepted/indexed4.png"),
            include_bytes!("../tests/corpus/png/v1/accepted/indexed8.png"),
            include_bytes!("../tests/corpus/png/v1/accepted/grayscale-alpha8.png"),
            include_bytes!("../tests/corpus/png/v1/accepted/truecolor-alpha8.png"),
            include_bytes!("../tests/corpus/png/v1/accepted/indexed-transparency.png"),
            include_bytes!("../tests/corpus/png/v1/accepted/transparent-nonzero-color.png"),
            include_bytes!("../tests/corpus/png/v1/accepted/ancillary-before-after.png"),
            include_bytes!("../tests/corpus/png/v1/accepted/oxipng-reduction.png"),
        ];
        for bytes in accepted {
            validate_source(bytes).unwrap();
        }

        let rejected: &[&[u8]] = &[
            include_bytes!("../tests/corpus/png/v1/rejected/apng.png"),
            include_bytes!("../tests/corpus/png/v1/rejected/bad-crc.png"),
            include_bytes!("../tests/corpus/png/v1/rejected/cabx.png"),
            include_bytes!("../tests/corpus/png/v1/rejected/interlaced.png"),
            include_bytes!("../tests/corpus/png/v1/rejected/invalid-filter.png"),
            include_bytes!("../tests/corpus/png/v1/rejected/oversized-dimensions.png"),
            include_bytes!("../tests/corpus/png/v1/rejected/palette-out-of-range.png"),
            include_bytes!("../tests/corpus/png/v1/rejected/sixteen-bit.png"),
            include_bytes!("../tests/corpus/png/v1/rejected/trailing.png"),
            include_bytes!("../tests/corpus/png/v1/rejected/truncated.png"),
            include_bytes!("../tests/corpus/png/v1/rejected/unknown.png"),
            include_bytes!("../tests/corpus/png/v1/rejected/xmp.png"),
        ];
        for bytes in rejected {
            assert!(validate_source(bytes).is_err());
        }

        let equivalent_source = validate_source(include_bytes!(
            "../tests/corpus/png/v1/equivalent/source.png"
        ))
        .unwrap();
        validate_candidate(
            &equivalent_source,
            include_bytes!("../tests/corpus/png/v1/equivalent/candidate.png"),
        )
        .unwrap();
        validate_candidate(
            &equivalent_source,
            include_bytes!("../tests/corpus/png/v1/equivalent/unchanged.png"),
        )
        .unwrap();

        let changed_source =
            validate_source(include_bytes!("../tests/corpus/png/v1/changed/source.png")).unwrap();
        assert!(
            validate_candidate(
                &changed_source,
                include_bytes!("../tests/corpus/png/v1/changed/candidate.png")
            )
            .is_err()
        );
    }

    fn make_png(
        width: u32,
        height: u32,
        depth: u8,
        color: u8,
        palette: Option<&[u8]>,
        filtered: &[u8],
        ancillary: &[(&[u8; 4], &[u8])],
    ) -> Vec<u8> {
        make_png_with_compression(
            width,
            height,
            depth,
            color,
            palette,
            filtered,
            ancillary,
            Compression::default(),
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the test fixture builder exposes the PNG fields varied by the corpus"
    )]
    fn make_png_with_compression(
        width: u32,
        height: u32,
        depth: u8,
        color: u8,
        palette: Option<&[u8]>,
        filtered: &[u8],
        ancillary: &[(&[u8; 4], &[u8])],
        compression: Compression,
    ) -> Vec<u8> {
        let mut png = SIGNATURE.to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[depth, color, 0, 0, 0]);
        push_chunk(&mut png, b"IHDR", &ihdr);
        for (name, data) in ancillary {
            if matches!(*name, b"tRNS" | b"bKGD") && color == 3 {
                continue;
            }
            push_chunk(&mut png, name, data);
        }
        if let Some(palette) = palette {
            push_chunk(&mut png, b"PLTE", palette);
        }
        if color == 3 {
            for (name, data) in ancillary {
                if matches!(*name, b"tRNS" | b"bKGD") {
                    push_chunk(&mut png, name, data);
                }
            }
        }
        let mut encoder = ZlibEncoder::new(Vec::new(), compression);
        encoder.write_all(filtered).unwrap();
        let idat = encoder.finish().unwrap();
        push_chunk(&mut png, b"IDAT", &idat);
        push_chunk(&mut png, b"IEND", &[]);
        png
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
