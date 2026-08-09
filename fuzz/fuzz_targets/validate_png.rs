#![no_main]
#![allow(dead_code)]

#[path = "../../src/limits.rs"]
mod limits;
#[path = "../../src/png.rs"]
mod png;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = png::validate_source(bytes);
});
