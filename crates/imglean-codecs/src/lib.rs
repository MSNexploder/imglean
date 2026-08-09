#![deny(unsafe_code)]
#![allow(
    clippy::result_unit_err,
    reason = "the worker protocol intentionally exposes only provider success or failure"
)]

use std::path::Path;

use libpng_sys as _;

mod avif_codec;
mod jpegli_codec;
mod mozjpeg_codec;
#[allow(unsafe_code)]
mod native;
#[allow(unsafe_code)]
mod webp_codec;

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

pub fn optimize_libwebp(
    source: &[u8],
    quality: Option<u8>,
    strip_metadata: bool,
) -> Result<Vec<u8>, ()> {
    webp_codec::optimize_libwebp(source, quality, strip_metadata)
}

pub fn optimize_image_webp(source: &[u8], strip_metadata: bool) -> Result<Vec<u8>, ()> {
    webp_codec::optimize_image_webp(source, strip_metadata)
}

pub fn optimize_avif_aom(source: &[u8], quality: u8) -> Result<Vec<u8>, ()> {
    avif_codec::optimize_aom(source, quality)
}

pub fn optimize_avif_rav1e(source: &[u8], quality: u8) -> Result<Vec<u8>, ()> {
    avif_codec::optimize_rav1e(source, quality)
}
