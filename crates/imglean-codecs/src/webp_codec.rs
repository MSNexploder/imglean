use std::ffi::c_void;
use std::io::Cursor;
use std::mem::MaybeUninit;
use std::slice;

use image_webp::{ColorType, WebPDecoder, WebPEncoder};
use libwebp_sys::*;

pub fn optimize_image_webp(source: &[u8], strip_metadata: bool) -> Result<Vec<u8>, ()> {
    let mut decoder = WebPDecoder::new(Cursor::new(source)).map_err(|_| ())?;
    let (width, height) = decoder.dimensions();
    let has_alpha = decoder.has_alpha();
    let icc = if strip_metadata {
        None
    } else {
        decoder.icc_profile().map_err(|_| ())?
    };
    let exif = if strip_metadata {
        None
    } else {
        decoder.exif_metadata().map_err(|_| ())?
    };
    let mut pixels = vec![0; decoder.output_buffer_size().ok_or(())?];
    decoder.read_image(&mut pixels).map_err(|_| ())?;

    let mut output = Vec::new();
    let mut encoder = WebPEncoder::new(&mut output);
    if let Some(icc) = &icc {
        encoder.set_icc_profile(icc.clone());
    }
    if let Some(exif) = &exif {
        encoder.set_exif_metadata(exif.clone());
    }
    encoder
        .encode(
            &pixels,
            width,
            height,
            if has_alpha {
                ColorType::Rgba8
            } else {
                ColorType::Rgb8
            },
        )
        .map_err(|_| ())?;
    Ok(output)
}

pub fn optimize_libwebp(
    source: &[u8],
    quality: Option<u8>,
    strip_metadata: bool,
) -> Result<Vec<u8>, ()> {
    let mut decoder = WebPDecoder::new(Cursor::new(source)).map_err(|_| ())?;
    let (width, height) = decoder.dimensions();
    let has_alpha = decoder.has_alpha();
    let icc = if strip_metadata {
        None
    } else {
        decoder.icc_profile().map_err(|_| ())?
    };
    let exif = if strip_metadata {
        None
    } else {
        decoder.exif_metadata().map_err(|_| ())?
    };
    let mut pixels = vec![0; decoder.output_buffer_size().ok_or(())?];
    decoder.read_image(&mut pixels).map_err(|_| ())?;
    let encoded = encode_pixels(&pixels, width, height, has_alpha, quality)?;
    attach_metadata(&encoded, icc.as_deref(), exif.as_deref())
}

// Safety: all libwebp structures are initialized with the matching vendored ABI. Pixel and
// metadata pointers remain valid for the duration of each call, libwebp copies imported data,
// and every libwebp-owned allocation is released on every initialized path.
fn encode_pixels(
    pixels: &[u8],
    width: u32,
    height: u32,
    has_alpha: bool,
    quality: Option<u8>,
) -> Result<Vec<u8>, ()> {
    let width = i32::try_from(width).map_err(|_| ())?;
    let height = i32::try_from(height).map_err(|_| ())?;
    let channels = if has_alpha { 4 } else { 3 };
    let stride = width.checked_mul(channels).ok_or(())?;
    unsafe {
        let mut config = MaybeUninit::<WebPConfig>::uninit();
        if WebPConfigInitInternal(
            config.as_mut_ptr(),
            WebPPreset::WEBP_PRESET_DEFAULT,
            quality.unwrap_or(100) as f32,
            WEBP_ENCODER_ABI_VERSION as i32,
        ) == 0
        {
            return Err(());
        }
        let mut config = config.assume_init();
        config.method = 6;
        config.thread_level = 0;
        config.alpha_quality = 100;
        config.exact = 1;
        if quality.is_none() {
            if WebPConfigLosslessPreset(&mut config, 9) == 0 {
                return Err(());
            }
            config.exact = 1;
        }
        if WebPValidateConfig(&config) == 0 {
            return Err(());
        }

        let mut picture = MaybeUninit::<WebPPicture>::uninit();
        if WebPPictureInitInternal(picture.as_mut_ptr(), WEBP_ENCODER_ABI_VERSION as i32) == 0 {
            return Err(());
        }
        let mut picture = picture.assume_init();
        picture.use_argb = 1;
        picture.width = width;
        picture.height = height;
        let imported = if has_alpha {
            WebPPictureImportRGBA(&mut picture, pixels.as_ptr(), stride)
        } else {
            WebPPictureImportRGB(&mut picture, pixels.as_ptr(), stride)
        };
        if imported == 0 {
            WebPPictureFree(&mut picture);
            return Err(());
        }
        let mut writer = MaybeUninit::<WebPMemoryWriter>::uninit();
        WebPMemoryWriterInit(writer.as_mut_ptr());
        let mut writer = writer.assume_init();
        picture.writer = Some(WebPMemoryWrite);
        picture.custom_ptr = (&mut writer as *mut WebPMemoryWriter).cast::<c_void>();
        let success = WebPEncode(&config, &mut picture) != 0;
        let output = if success && !writer.mem.is_null() {
            slice::from_raw_parts(writer.mem, writer.size).to_vec()
        } else {
            Vec::new()
        };
        WebPMemoryWriterClear(&mut writer);
        WebPPictureFree(&mut picture);
        if success { Ok(output) } else { Err(()) }
    }
}

fn attach_metadata(encoded: &[u8], icc: Option<&[u8]>, exif: Option<&[u8]>) -> Result<Vec<u8>, ()> {
    if icc.is_none() && exif.is_none() {
        return Ok(encoded.to_vec());
    }
    unsafe {
        let input = WebPData {
            bytes: encoded.as_ptr(),
            size: encoded.len(),
        };
        let mux = WebPMuxCreateInternal(&input, 1, WEBP_MUX_ABI_VERSION as i32);
        if mux.is_null() {
            return Err(());
        }
        for (fourcc, bytes) in [(b"ICCP\0", icc), (b"EXIF\0", exif)] {
            if let Some(bytes) = bytes {
                let data = WebPData {
                    bytes: bytes.as_ptr(),
                    size: bytes.len(),
                };
                if WebPMuxSetChunk(mux, fourcc.as_ptr().cast(), &data, 1)
                    != WebPMuxError::WEBP_MUX_OK
                {
                    WebPMuxDelete(mux);
                    return Err(());
                }
            }
        }
        let mut assembled = WebPData {
            bytes: std::ptr::null(),
            size: 0,
        };
        let success = WebPMuxAssemble(mux, &mut assembled) == WebPMuxError::WEBP_MUX_OK;
        WebPMuxDelete(mux);
        let output = if success && !assembled.bytes.is_null() {
            slice::from_raw_parts(assembled.bytes, assembled.size).to_vec()
        } else {
            Vec::new()
        };
        if !assembled.bytes.is_null() {
            WebPFree(assembled.bytes.cast_mut().cast());
        }
        if success { Ok(output) } else { Err(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REDUCTION_SOURCE: &[u8] =
        include_bytes!("../../../tests/corpus/webp/v1/accepted/provider-reduction.webp");

    #[test]
    fn every_webp_encoder_produces_a_real_reduction() {
        for candidate in [
            optimize_libwebp(REDUCTION_SOURCE, Some(60), false).unwrap(),
            optimize_image_webp(REDUCTION_SOURCE, false).unwrap(),
        ] {
            assert!(candidate.len() < REDUCTION_SOURCE.len());
        }
    }

    #[test]
    fn lossless_encoders_preserve_rgb_beneath_full_transparency() {
        let pixels = [17, 34, 51, 0, 200, 150, 100, 255];
        let mut source = Vec::new();
        WebPEncoder::new(&mut source)
            .encode(&pixels, 2, 1, ColorType::Rgba8)
            .unwrap();

        for candidate in [
            optimize_libwebp(&source, None, false).unwrap(),
            optimize_image_webp(&source, false).unwrap(),
        ] {
            let mut decoder = WebPDecoder::new(Cursor::new(candidate)).unwrap();
            let mut decoded = vec![0; decoder.output_buffer_size().unwrap()];
            decoder.read_image(&mut decoded).unwrap();
            assert_eq!(decoded, pixels);
        }
    }
}
