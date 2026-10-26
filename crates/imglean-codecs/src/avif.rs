//! ImgLean's used subset of libavif-rs 0.14.0 (Charles Samuels and Paolo
//! Barbolini). See licenses/libavif-rs-LICENSE for the BSD-2-Clause notice.
use super::avif_ffi as sys;
use std::ptr::NonNull;
use std::slice;

/// An owned, eight-bit RGBA decode. The native allocation stays private.
pub struct RgbaImage(sys::avifRGBImage);

impl RgbaImage {
    pub fn width(&self) -> u32 {
        self.0.width
    }

    pub fn height(&self) -> u32 {
        self.0.height
    }

    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: decode_rgb checks the allocation and slice length before
        // constructing this owner. Only Drop frees it; the borrow cannot outlive us.
        unsafe {
            slice::from_raw_parts(
                self.0.pixels,
                self.0.rowBytes as usize * self.0.height as usize,
            )
        }
    }
}

/// Decode a complete image using the same libavif defaults as the old wrapper.
pub fn decode_rgb(source: &[u8]) -> Result<RgbaImage, ()> {
    let image = decode(source)?;
    let mut rgb = RgbaImage(sys::avifRGBImage::default());
    // SAFETY: image owns an initialized native image. The RGB configuration is
    // initialized by libavif; allocation and conversion use those same headers.
    unsafe {
        sys::avifRGBImageSetDefaults(&mut rgb.0, image.0.as_ptr());
        rgb.0.format = sys::AVIF_RGB_FORMAT_RGBA;
        rgb.0.depth = 8;
        sys::avifRGBImageAllocatePixels(&mut rgb.0);
        if rgb.0.pixels.is_null()
            || (rgb.0.rowBytes as usize)
                .checked_mul(rgb.0.height as usize)
                .is_none_or(|length| length == 0 || length > isize::MAX as usize)
            || sys::avifImageYUVToRGB(image.0.as_ptr(), &mut rgb.0) != sys::AVIF_RESULT_OK
        {
            return Err(());
        }
    }
    Ok(rgb)
}

/// Re-encode the decoded YUV image, retaining its native metadata and color data.
pub fn optimize_aom(source: &[u8], quality: u8) -> Result<Vec<u8>, ()> {
    encode(&decode(source)?, quality)
}

/// Produce the grayscale images used by the checked-in corpus generator.
pub fn encode_gray(width: u32, height: u32, pixels: &[u8], quality: u8) -> Result<Vec<u8>, ()> {
    let length = (width as usize).checked_mul(height as usize).ok_or(())?;
    if width == 0 || height == 0 || pixels.len() != length {
        return Err(());
    }
    // SAFETY: creation either returns null or an owned image. The requested
    // eight-bit grayscale plane is allocated before rows are copied into it.
    let image = unsafe {
        Image(
            NonNull::new(sys::avifImageCreate(
                width,
                height,
                8,
                sys::AVIF_PIXEL_FORMAT_YUV400,
            ))
            .ok_or(())?,
        )
    };
    unsafe {
        if sys::avifImageAllocatePlanes(image.0.as_ptr(), sys::AVIF_PLANES_YUV)
            != sys::AVIF_RESULT_OK
        {
            return Err(());
        }
        let raw = image.0.as_ref();
        if raw.yuvPlanes[0].is_null() || raw.yuvRowBytes[0] < width {
            return Err(());
        }
        for row in 0..height as usize {
            std::ptr::copy_nonoverlapping(
                pixels.as_ptr().add(row * width as usize),
                raw.yuvPlanes[0].add(row * raw.yuvRowBytes[0] as usize),
                width as usize,
            );
        }
    }
    encode(&image, quality)
}

struct Image(NonNull<sys::avifImage>);

fn decode(source: &[u8]) -> Result<Image, ()> {
    // SAFETY: the input slice remains live for the synchronous read. Both
    // allocations are checked and destroyed with their matching native API.
    unsafe {
        let image = Image(NonNull::new(sys::avifImageCreateEmpty()).ok_or(())?);
        let decoder = NonNull::new(sys::avifDecoderCreate()).ok_or(())?;
        let result = sys::avifDecoderReadMemory(
            decoder.as_ptr(),
            image.0.as_ptr(),
            source.as_ptr(),
            source.len(),
        );
        sys::avifDecoderDestroy(decoder.as_ptr());
        if result != sys::AVIF_RESULT_OK {
            return Err(());
        }
        Ok(image)
    }
}

fn encode(image: &Image, quality: u8) -> Result<Vec<u8>, ()> {
    // SAFETY: image remains live for the synchronous encode. The encoder is
    // exclusively owned here, and the returned data uses libavif's allocator.
    unsafe {
        let mut encoder = NonNull::new(sys::avifEncoderCreate()).ok_or(())?;
        // Retain the old wrapper's SSIM quality mapping for both planes.
        let color = sys::avifEncoderSetCodecSpecificOption(
            encoder.as_ptr(),
            c"color:tune".as_ptr(),
            c"ssim".as_ptr(),
        );
        let alpha = sys::avifEncoderSetCodecSpecificOption(
            encoder.as_ptr(),
            c"alpha:tune".as_ptr(),
            c"ssim".as_ptr(),
        );
        if color != sys::AVIF_RESULT_OK || alpha != sys::AVIF_RESULT_OK {
            sys::avifEncoderDestroy(encoder.as_ptr());
            return Err(());
        }
        let settings = encoder.as_mut();
        settings.quality = i32::from(quality.min(100));
        settings.qualityAlpha = 100;
        settings.speed = 6;
        settings.maxThreads = 1;
        let mut data = sys::avifRWData::default();
        let result = sys::avifEncoderWrite(encoder.as_ptr(), image.0.as_ptr(), &mut data);
        sys::avifEncoderDestroy(encoder.as_ptr());
        let output = if result == sys::AVIF_RESULT_OK
            && !data.data.is_null()
            && data.size <= isize::MAX as usize
        {
            Ok(slice::from_raw_parts(data.data, data.size).to_vec())
        } else {
            Err(())
        };
        sys::avifRWDataFree(&mut data);
        output
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        // SAFETY: this owner holds the sole non-null allocation from libavif.
        unsafe { sys::avifImageDestroy(self.0.as_ptr()) }
    }
}

impl Drop for RgbaImage {
    fn drop(&mut self) {
        // SAFETY: pixels are null or allocated by avifRGBImageAllocatePixels.
        unsafe { sys::avifRGBImageFreePixels(&mut self.0) }
    }
}
