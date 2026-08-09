use std::path::Path;

use crate::avif::{self, ValidatedAvif};
use crate::jpeg::{self, ValidatedJpeg};
use crate::png::{self, ValidatedPng};
use crate::webp::{self, ValidatedWebp};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
    Avif,
}

impl ImageFormat {
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?;
        if extension.eq_ignore_ascii_case("png") {
            Some(Self::Png)
        } else if extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg") {
            Some(Self::Jpeg)
        } else if extension.eq_ignore_ascii_case("webp") {
            Some(Self::Webp)
        } else if extension.eq_ignore_ascii_case("avif") {
            Some(Self::Avif)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidatedImage {
    Png(ValidatedPng),
    Jpeg(ValidatedJpeg),
    Webp(ValidatedWebp),
    Avif(ValidatedAvif),
}

impl ValidatedImage {
    pub fn validate_source(format: ImageFormat, bytes: &[u8]) -> Result<Self, &'static str> {
        match format {
            ImageFormat::Png => png::validate_source(bytes)
                .map(Self::Png)
                .map_err(|error| error.message()),
            ImageFormat::Jpeg => jpeg::validate_source(bytes)
                .map(Self::Jpeg)
                .map_err(|error| error.message()),
            ImageFormat::Webp => webp::validate_source(bytes)
                .map(Self::Webp)
                .map_err(|error| error.message()),
            ImageFormat::Avif => avif::validate_source(bytes)
                .map(Self::Avif)
                .map_err(|error| error.message()),
        }
    }

    pub fn validate_candidate(self, bytes: &[u8]) -> Result<Self, &'static str> {
        match self {
            Self::Png(source) => png::validate_candidate(&source, bytes)
                .map(Self::Png)
                .map_err(|error| error.message()),
            Self::Jpeg(source) => jpeg::validate_candidate(&source, bytes)
                .map(Self::Jpeg)
                .map_err(|error| error.message()),
            Self::Webp(source) => webp::validate_candidate(&source, bytes)
                .map(Self::Webp)
                .map_err(|error| error.message()),
            Self::Avif(source) => avif::validate_candidate(&source, bytes)
                .map(Self::Avif)
                .map_err(|error| error.message()),
        }
    }

    pub fn encoded_bytes(self) -> usize {
        match self {
            Self::Png(image) => image.encoded_bytes(),
            Self::Jpeg(image) => image.encoded_bytes(),
            Self::Webp(image) => image.encoded_bytes(),
            Self::Avif(image) => image.encoded_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_every_supported_extension_without_ascii_case_sensitivity() {
        for (path, expected) in [
            ("image.png", ImageFormat::Png),
            ("image.JPG", ImageFormat::Jpeg),
            ("image.jpeg", ImageFormat::Jpeg),
            ("image.WebP", ImageFormat::Webp),
            ("image.AVIF", ImageFormat::Avif),
        ] {
            assert_eq!(ImageFormat::from_path(Path::new(path)), Some(expected));
        }
        assert_eq!(ImageFormat::from_path(Path::new("image.gif")), None);
    }
}
