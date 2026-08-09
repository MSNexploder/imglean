use std::path::Path;

use crate::jpeg::{self, ValidatedJpeg};
use crate::png::{self, ValidatedPng};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImageFormat {
    Png,
    Jpeg,
}

impl ImageFormat {
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?;
        if extension.eq_ignore_ascii_case("png") {
            Some(Self::Png)
        } else if extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg") {
            Some(Self::Jpeg)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidatedImage {
    Png(ValidatedPng),
    Jpeg(ValidatedJpeg),
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
        }
    }

    pub fn encoded_bytes(self) -> usize {
        match self {
            Self::Png(image) => image.encoded_bytes(),
            Self::Jpeg(image) => image.encoded_bytes(),
        }
    }
}
