use std::env;
use std::fs;
use std::path::Path;

use image_webp::{ColorType, WebPEncoder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args_os().nth(1).ok_or("missing output directory")?;
    let output = Path::new(&output);
    fs::create_dir_all(output)?;

    encode_webp(&output.join("source.webp"), 128, 128)?;
    encode_webp(&output.join("changed.webp"), 129, 128)?;
    encode_avif(&output.join("source.avif"), 128, 128)?;
    encode_avif(&output.join("changed.avif"), 129, 128)?;
    Ok(())
}

fn encode_webp(path: &Path, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
    let pixels = pixels(width, height)
        .into_iter()
        .flat_map(|value| [value, value.wrapping_add(37), value.wrapping_add(91)])
        .collect::<Vec<_>>();
    let mut encoded = Vec::new();
    WebPEncoder::new(&mut encoded).encode(&pixels, width, height, ColorType::Rgb8)?;
    fs::write(path, encoded)?;
    Ok(())
}

fn encode_avif(path: &Path, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
    let encoded = imglean_avif::encode_gray(width, height, &pixels(width, height), 72)
        .map_err(|()| "could not encode grayscale AVIF corpus image")?;
    fs::write(path, encoded)?;
    Ok(())
}

fn pixels(width: u32, height: u32) -> Vec<u8> {
    (0..height)
        .flat_map(|y| {
            (0..width).map(move |x| {
                let gradient = (x * 3 + y * 5) as u8;
                let checker = if (x / 16 + y / 16) % 2 == 0 { 24 } else { 0 };
                gradient.wrapping_add(checker)
            })
        })
        .collect()
}
