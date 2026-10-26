//! In-memory path adapted from jpegli 0.1.0 (BSD-3-Clause).
//! See licenses/jpegli-rs-LICENSE and licenses/jpegli-LICENSE.

use super::jpegli_ffi as ffi;
use std::mem;
use std::os::raw::{c_int, c_long};
use std::ptr;
use std::slice;

pub fn optimize(source: &[u8], quality: u8, strip_metadata: bool) -> Result<Vec<u8>, ()> {
    if source.is_empty() {
        return Err(());
    }
    let mut decoder = Decoder::new(source);
    // SAFETY: The guards own stable, correctly initialized C structures and managers.
    // Source bytes remain borrowed until decoder destruction. Every native call uses
    // C-unwind so fatal errors unwind through the guards to the provider worker.
    unsafe {
        let info = &mut *decoder.info;
        for marker in (ffi::JPEG_APP0..=ffi::JPEG_APP0 + 15).chain([ffi::JPEG_COM]) {
            ffi::jpegli_save_markers(info, marker as c_int, 0xffff);
        }
        // As in the wrapper, tables-only input returns an error without a fatal exit.
        if ffi::jpegli_read_header(info, 0) != 1 {
            return Err(());
        }
        let (width, height) = (info.image_width, info.image_height);
        let grayscale = info.jpeg_color_space == ffi::J_COLOR_SPACE::JCS_GRAYSCALE;
        let color_space = if grayscale {
            ffi::J_COLOR_SPACE::JCS_GRAYSCALE
        } else {
            ffi::J_COLOR_SPACE::JCS_RGB
        };
        let components = if grayscale { 1 } else { 3 };
        let mut markers = Vec::new();
        if !strip_metadata {
            let mut marker = info.marker_list;
            while let Some(saved) = marker.as_ref() {
                // Saved marker payloads belong to the decoder; copy before finish.
                let data = if saved.data_length == 0 {
                    Vec::new()
                } else {
                    slice::from_raw_parts(saved.data, saved.data_length as usize).to_vec()
                };
                markers.push((c_int::from(saved.marker), data));
                marker = saved.next;
            }
        }
        info.out_color_space = color_space;
        if ffi::jpegli_start_decompress(info) == 0 {
            return Err(());
        }
        let row_bytes = (info.output_width as usize)
            .checked_mul(components)
            .filter(|&size| size > 0)
            .ok_or(())?;
        let pixel_bytes = row_bytes
            .checked_mul(info.output_height as usize)
            .ok_or(())?;
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(pixel_bytes).map_err(|_| ())?;
        pixels.resize(pixel_bytes, 0);
        for row in pixels.chunks_exact_mut(row_bytes) {
            let mut row_ptr = row.as_mut_ptr();
            if info.output_scanline >= info.output_height
                || ffi::jpegli_read_scanlines(info, &mut row_ptr, 1) != 1
            {
                return Err(());
            }
        }
        if ffi::jpegli_finish_decompress(info) == 0 {
            return Err(());
        }
        drop(decoder);

        let mut encoder = Encoder::new();
        let info = &mut *encoder.info;
        info.in_color_space = color_space;
        info.input_components = components as c_int;
        ffi::jpegli_set_defaults(info);
        info.image_width = width;
        info.image_height = height;
        ffi::jpegli_set_quality(info, c_int::from(quality), 0);
        ffi::jpegli_simple_progression(info);
        info.optimize_coding = 1;
        ffi::jpegli_start_compress(info, 1);
        for (marker, data) in markers {
            if marker == (ffi::JPEG_APP0 + 14) as c_int && data.starts_with(b"Adobe") {
                continue;
            }
            // Jpegli defaults suppress automatic JFIF output; replay the source once.
            ffi::jpegli_write_marker(info, marker, data.as_ptr(), data.len() as _);
        }
        for row in pixels.chunks_exact_mut(row_bytes) {
            let mut row_ptr = row.as_mut_ptr();
            if ffi::jpegli_write_scanlines(info, &mut row_ptr, 1) != 1 {
                return Err(());
            }
        }
        ffi::jpegli_finish_compress(info);
        Ok(mem::take(&mut encoder.destination.output))
    }
}

struct Decoder<'a> {
    info: Box<ffi::jpeg_decompress_struct>,
    _error: Box<ffi::jpeg_error_mgr>,
    _source: Box<ffi::jpeg_source_mgr>,
    _bytes: &'a [u8],
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        let mut error = error_manager();
        let source = Box::new(ffi::jpeg_source_mgr {
            next_input_byte: bytes.as_ptr(),
            bytes_in_buffer: bytes.len(),
            init_source: Some(source_noop),
            fill_input_buffer: Some(source_eof),
            skip_input_data: Some(source_skip),
            resync_to_restart: Some(ffi::jpegli_resync_to_restart),
            term_source: Some(source_noop),
        });
        // SAFETY: Zero is valid for the C structure before CreateDecompress. The guard
        // is installed before creation, so partial initialization is destroyed on unwind.
        unsafe {
            let mut info: Box<ffi::jpeg_decompress_struct> = Box::new(mem::zeroed());
            info.err = &mut *error;
            let mut decoder = Self {
                info,
                _error: error,
                _source: source,
                _bytes: bytes,
            };
            ffi::jpegli_CreateDecompress(
                &mut *decoder.info,
                ffi::JPEG_LIB_VERSION as c_int,
                mem::size_of::<ffi::jpeg_decompress_struct>(),
            );
            decoder.info.src = &mut *decoder._source;
            decoder
        }
    }
}

impl Drop for Decoder<'_> {
    fn drop(&mut self) {
        // SAFETY: Destroy accepts a zeroed or partially created object. The error and
        // source managers outlive this call; native allocations are released exactly once.
        unsafe { ffi::jpegli_destroy_decompress(&mut *self.info) };
    }
}

struct Encoder {
    info: Box<ffi::jpeg_compress_struct>,
    _error: Box<ffi::jpeg_error_mgr>,
    destination: Box<Destination>,
}

impl Encoder {
    fn new() -> Self {
        let mut error = error_manager();
        let destination = Box::new(Destination {
            interface: ffi::jpeg_destination_mgr {
                next_output_byte: ptr::null_mut(),
                free_in_buffer: 0,
                init_destination: Some(destination_init),
                empty_output_buffer: Some(destination_full),
                term_destination: Some(destination_finish),
            },
            buffer: [0; 4096],
            output: Vec::new(),
        });
        // SAFETY: As for Decoder, the guard exists before native creation. Boxes keep
        // both callback managers stable until native destruction, including on unwind.
        unsafe {
            let mut info: Box<ffi::jpeg_compress_struct> = Box::new(mem::zeroed());
            info.err = &mut *error;
            let mut encoder = Self {
                info,
                _error: error,
                destination,
            };
            ffi::jpegli_CreateCompress(
                &mut *encoder.info,
                ffi::JPEG_LIB_VERSION as c_int,
                mem::size_of::<ffi::jpeg_compress_struct>(),
            );
            encoder.info.dest = &mut encoder.destination.interface;
            encoder
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: The native object is uniquely owned and its error manager is live.
        // Detach the output manager before destruction, as in the original wrapper.
        unsafe {
            self.info.dest = ptr::null_mut();
            ffi::jpegli_destroy_compress(&mut *self.info);
        }
    }
}

fn error_manager() -> Box<ffi::jpeg_error_mgr> {
    // SAFETY: jpegli_std_error initializes the zeroed C error manager. Both overridden
    // callbacks have its declared ABI; error_exit never returns to native execution.
    unsafe {
        let mut error = Box::new(mem::zeroed());
        ffi::jpegli_std_error(&mut *error);
        error.error_exit = Some(fatal_error);
        error.emit_message = Some(silence_message);
        error
    }
}

unsafe extern "C-unwind" fn silence_message(_: *mut ffi::jpeg_common_struct, _: c_int) {}

unsafe extern "C-unwind" fn fatal_error(info: *mut ffi::jpeg_common_struct) {
    // SAFETY: Native calls supply the live error manager installed above. The buffer
    // has the native JMSG_LENGTH_MAX capacity and is writable for format_message.
    let message = unsafe {
        let error = &*(*info).err;
        if let Some(format) = error.format_message {
            let mut buffer = [0; ffi::JMSG_LENGTH_MAX as usize];
            format(info, buffer.as_mut_ptr().cast());
            let end = buffer
                .iter()
                .position(|&byte| byte == 0)
                .unwrap_or(buffer.len());
            format!(
                "libjpeg fatal error: {}",
                String::from_utf8_lossy(&buffer[..end])
            )
        } else {
            format!("libjpeg fatal error: code {}", error.msg_code)
        }
    };
    // Keep the upstream panic boundary without invoking the panic hook.
    std::panic::resume_unwind(Box::new(message));
}

unsafe extern "C-unwind" fn source_noop(_: *mut ffi::jpeg_decompress_struct) {}

unsafe extern "C-unwind" fn source_eof(info: *mut ffi::jpeg_decompress_struct) -> ffi::boolean {
    // SAFETY: src and err point to our live boxed managers. The synthetic EOI bytes
    // have static storage. Warnings remain silent, matching the slice-reader wrapper.
    unsafe {
        (*(*info).src).next_input_byte = [0xff, 0xd9, 0xff, 0xd9].as_ptr();
        (*(*info).src).bytes_in_buffer = 4;
        (*(*info).err).msg_code = ffi::JWRN_JPEG_EOF as c_int;
    }
    1
}

unsafe extern "C-unwind" fn source_skip(info: *mut ffi::jpeg_decompress_struct, count: c_long) {
    if count <= 0 {
        return;
    }
    // SAFETY: The source manager describes either the borrowed input or static EOI
    // storage. Advance only within that buffer. The original slice reader fails when
    // a skip needs another buffer, rather than synthesizing EOI for the skip.
    unsafe {
        let source = &mut *(*info).src;
        let count = count as usize;
        if count > source.bytes_in_buffer {
            (*(*info).err).msg_code = ffi::JERR_FILE_READ as c_int;
            fatal_error(info.cast());
        }
        source.next_input_byte = source.next_input_byte.add(count);
        source.bytes_in_buffer -= count;
    }
}

// The interface must be first so native callback pointers can recover their owner.
#[repr(C)]
struct Destination {
    interface: ffi::jpeg_destination_mgr,
    buffer: [u8; 4096],
    output: Vec<u8>,
}

impl Destination {
    fn reset(&mut self) {
        self.interface.next_output_byte = self.buffer.as_mut_ptr();
        self.interface.free_in_buffer = self.buffer.len();
    }
}

unsafe extern "C-unwind" fn destination_init(info: *mut ffi::jpeg_compress_struct) {
    // SAFETY: dest is the first field of our live, uniquely borrowed boxed Destination.
    unsafe { (&mut *(*info).dest.cast::<Destination>()).reset() };
}

unsafe extern "C-unwind" fn destination_full(info: *mut ffi::jpeg_compress_struct) -> ffi::boolean {
    // SAFETY: As above. This callback means the whole buffer was written; Jpegli may
    // leave its position fields stale here, so do not infer the length from them.
    unsafe {
        let destination = &mut *(*info).dest.cast::<Destination>();
        destination.output.extend_from_slice(&destination.buffer);
        destination.reset();
    }
    1
}

unsafe extern "C-unwind" fn destination_finish(info: *mut ffi::jpeg_compress_struct) {
    // SAFETY: dest is our live Destination. At finish, free_in_buffer is current and
    // the prefix contains native-written bytes. Bounds are checked before copying.
    unsafe {
        let destination = &mut *(*info).dest.cast::<Destination>();
        let Some(used) = destination
            .buffer
            .len()
            .checked_sub(destination.interface.free_in_buffer)
        else {
            (*(*info).err).msg_code = ffi::JERR_BUFFER_SIZE as c_int;
            fatal_error(info.cast());
            return;
        };
        destination
            .output
            .extend_from_slice(&destination.buffer[..used]);
        destination.reset();
        destination.interface.free_in_buffer = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::optimize;

    #[test]
    fn fatal_native_errors_unwind_through_the_owned_managers() {
        let result = std::panic::catch_unwind(|| optimize(b"not a JPEG", 80, false));
        assert!(result.is_err());
    }

    #[test]
    fn output_crossing_multiple_buffers_remains_a_complete_jpeg() {
        let mut state = 17_u32;
        let pixels: Vec<u8> = (0..128 * 128 * 3)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        let mut source = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
        source.set_size(128, 128);
        source.set_quality(100.0);
        let mut source = source.start_compress(Vec::new()).unwrap();
        source.write_scanlines(&pixels).unwrap();
        let source = source.finish().unwrap();

        let encoded = optimize(&source, 95, false).unwrap();
        assert!(encoded.len() > 2 * 4096);
        assert!(encoded.ends_with(&[0xff, 0xd9]));
        let decoded = mozjpeg::Decompress::new_mem(&encoded).unwrap();
        assert_eq!(decoded.size(), (128, 128));
        let mut decoded = decoded.rgb().unwrap();
        assert_eq!(decoded.read_scanlines::<u8>().unwrap().len(), pixels.len());
        decoded.finish().unwrap();
    }
}
