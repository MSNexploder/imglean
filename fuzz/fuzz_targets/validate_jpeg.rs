#![no_main]
#![allow(dead_code)]

#[path = "../../src/jpeg.rs"]
mod jpeg;
#[path = "../../src/limits.rs"]
mod limits;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = jpeg::validate_source(bytes);
});
