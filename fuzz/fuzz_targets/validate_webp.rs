#![no_main]
#![allow(dead_code)]

#[path = "../../src/limits.rs"]
mod limits;
#[path = "../../src/webp.rs"]
mod webp;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = webp::validate_source(bytes);
});
