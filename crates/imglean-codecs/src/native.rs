use std::ffi::{CString, c_int, c_ulong, c_void};
use std::mem;
use std::path::Path;
use std::ptr;
use std::slice;

use mozjpeg_sys::{
    JCOPY_OPTION_JCOPYOPT_ALL, JCOPY_OPTION_JCOPYOPT_NONE, jcopy_markers_execute,
    jcopy_markers_setup, jpeg_common_struct, jpeg_compress_struct, jpeg_create_compress,
    jpeg_create_decompress, jpeg_decompress_struct, jpeg_destroy_compress, jpeg_destroy_decompress,
    jpeg_error_mgr, jpeg_finish_compress, jpeg_finish_decompress, jpeg_mem_dest, jpeg_mem_src,
    jpeg_read_coefficients, jpeg_read_header, jpeg_simple_progression, jpeg_std_error,
    jpeg_write_coefficients,
};

unsafe extern "C" {
    fn imglean_optipng_optimize(
        input: *const libc::c_char,
        output: *const libc::c_char,
        strip_metadata: c_int,
    ) -> c_int;
}

pub fn optimize_optipng(input: &Path, output: &Path, strip_metadata: bool) -> Result<(), ()> {
    let input = path_to_c_string(input)?;
    let output = path_to_c_string(output)?;
    // SAFETY: Both pointers refer to live NUL-terminated strings for the duration of the call.
    // OptiPNG runs once in ImgLean's disposable provider worker, and the C wrapper configures
    // distinct input and output paths before invoking the non-thread-safe engine.
    let status = unsafe {
        imglean_optipng_optimize(input.as_ptr(), output.as_ptr(), c_int::from(strip_metadata))
    };
    (status == 0).then_some(()).ok_or(())
}

pub fn optimize_jpegtran(source: &[u8], strip_metadata: bool) -> Result<Vec<u8>, ()> {
    let source_len = c_ulong::try_from(source.len()).map_err(|_| ())?;
    let mut source_error = jpeg_error_manager();
    let mut destination_error = jpeg_error_manager();
    // SAFETY: All libjpeg structures are initialized before use and destroyed before return.
    // Source memory remains live throughout the transform. The destination is allocated by
    // libjpeg, copied into Rust-owned memory, and freed with the matching C allocator. Fatal
    // codec errors abort only the already-isolated provider worker via the installed handler.
    unsafe {
        let mut source_info: jpeg_decompress_struct = mem::zeroed();
        source_info.common.err = &mut source_error;
        jpeg_create_decompress(&mut source_info);
        jpeg_mem_src(&mut source_info, source.as_ptr(), source_len);
        let marker_policy = if strip_metadata {
            JCOPY_OPTION_JCOPYOPT_NONE
        } else {
            JCOPY_OPTION_JCOPYOPT_ALL
        };
        jcopy_markers_setup(&mut source_info, marker_policy);
        if jpeg_read_header(&mut source_info, 1) != 1 {
            jpeg_destroy_decompress(&mut source_info);
            return Err(());
        }
        let coefficients = jpeg_read_coefficients(&mut source_info);
        if coefficients.is_null() {
            jpeg_destroy_decompress(&mut source_info);
            return Err(());
        }

        let mut destination_info: jpeg_compress_struct = mem::zeroed();
        destination_info.common.err = &mut destination_error;
        jpeg_create_compress(&mut destination_info);
        mozjpeg_sys::jpeg_copy_critical_parameters(&source_info, &mut destination_info);
        destination_info.optimize_coding = 1;
        jpeg_simple_progression(&mut destination_info);

        let mut output = ptr::null_mut();
        let mut output_len = 0;
        jpeg_mem_dest(&mut destination_info, &mut output, &mut output_len);
        jpeg_write_coefficients(&mut destination_info, coefficients);
        jcopy_markers_execute(&mut source_info, &mut destination_info, marker_policy);
        jpeg_finish_compress(&mut destination_info);
        let finished_source = jpeg_finish_decompress(&mut source_info) != 0;
        let result = if finished_source && !output.is_null() {
            usize::try_from(output_len)
                .ok()
                .map(|length| slice::from_raw_parts(output, length).to_vec())
                .ok_or(())
        } else {
            Err(())
        };
        libc::free(output.cast::<c_void>());
        jpeg_destroy_compress(&mut destination_info);
        jpeg_destroy_decompress(&mut source_info);
        result
    }
}

fn path_to_c_string(path: &Path) -> Result<CString, ()> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        CString::new(path.as_os_str().as_bytes()).map_err(|_| ())
    }
    #[cfg(windows)]
    {
        path.to_str()
            .filter(|path| path.is_ascii())
            .ok_or(())
            .and_then(|path| CString::new(path).map_err(|_| ()))
    }
}

fn jpeg_error_manager() -> jpeg_error_mgr {
    // SAFETY: jpeg_std_error fully initializes the zeroed public error-manager structure.
    unsafe {
        let mut error: jpeg_error_mgr = mem::zeroed();
        jpeg_std_error(&mut error);
        error.error_exit = Some(abort_on_codec_error);
        error.emit_message = Some(abort_on_codec_warning);
        error
    }
}

unsafe extern "C-unwind" fn abort_on_codec_error(_: &mut jpeg_common_struct) {
    std::process::abort();
}

unsafe extern "C-unwind" fn abort_on_codec_warning(_: &mut jpeg_common_struct, level: c_int) {
    if level < 0 {
        std::process::abort();
    }
}
