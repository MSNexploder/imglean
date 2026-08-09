#![no_main]
#![allow(dead_code)]

#[path = "../../src/avif.rs"]
mod avif;
#[path = "../../src/limits.rs"]
mod limits;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = avif::validate_source(bytes);
});
