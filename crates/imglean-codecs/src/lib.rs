#![deny(unsafe_code)]

use std::path::Path;

use libpng_sys as _;

mod jpegli_codec;
mod mozjpeg_codec;
#[allow(unsafe_code)]
mod native;

pub fn optimize_optipng(input: &Path, output: &Path, strip_metadata: bool) -> Result<(), ()> {
    native::optimize_optipng(input, output, strip_metadata)
}

pub fn optimize_jpegtran(source: &[u8], strip_metadata: bool) -> Result<Vec<u8>, ()> {
    native::optimize_jpegtran(source, strip_metadata)
}

pub fn optimize_mozjpeg(source: &[u8], quality: u8, strip_metadata: bool) -> Result<Vec<u8>, ()> {
    mozjpeg_codec::optimize(source, quality, strip_metadata)
}

pub fn optimize_jpegli(source: &[u8], quality: u8, strip_metadata: bool) -> Result<Vec<u8>, ()> {
    jpegli_codec::optimize(source, quality, strip_metadata)
}
