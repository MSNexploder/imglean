use libavif::Encoder;
use ravif::{AlphaColorMode, BitDepth, Encoder as RavifEncoder, Img, RGBA8};

pub fn optimize_aom(source: &[u8], quality: u8) -> Result<Vec<u8>, ()> {
    let decoded = libavif::decode(source).map_err(|_| ())?;
    let mut encoder = Encoder::new();
    encoder
        .set_quality(quality)
        .set_alpha_quality(100)
        .set_speed(6)
        .set_max_threads(1);

    encoder
        .encode(&decoded)
        .map(|data| data.to_vec())
        .map_err(|_| ())
}

pub fn optimize_rav1e(source: &[u8], quality: u8) -> Result<Vec<u8>, ()> {
    let decoded = libavif::decode_rgb(source).map_err(|_| ())?;
    let width = usize::try_from(decoded.width()).map_err(|_| ())?;
    let height = usize::try_from(decoded.height()).map_err(|_| ())?;
    let pixels = decoded
        .as_slice()
        .chunks_exact(4)
        .map(|pixel| RGBA8::new(pixel[0], pixel[1], pixel[2], pixel[3]))
        .collect::<Vec<_>>();
    let encoded = RavifEncoder::new()
        .with_quality(f32::from(quality))
        .with_alpha_quality(100.0)
        .with_speed(6)
        .with_bit_depth(BitDepth::Eight)
        .with_alpha_color_mode(AlphaColorMode::UnassociatedDirty)
        .with_num_threads(Some(1))
        .encode_rgba(Img::new(&pixels, width, height))
        .map_err(|_| ())?;
    Ok(encoded.avif_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REDUCTION_SOURCE: &[u8] =
        include_bytes!("../../../tests/corpus/avif/v1/accepted/provider-reduction.avif");

    #[test]
    fn every_avif_encoder_produces_a_real_reduction() {
        for candidate in [
            optimize_aom(REDUCTION_SOURCE, 60).unwrap(),
            optimize_rav1e(REDUCTION_SOURCE, 60).unwrap(),
        ] {
            assert!(candidate.len() < REDUCTION_SOURCE.len());
        }
    }

    #[test]
    fn every_avif_encoder_preserves_dimensions_and_transparency() {
        let pixels = [
            RGBA8::new(17, 34, 51, 0),
            RGBA8::new(67, 84, 101, 64),
            RGBA8::new(117, 134, 151, 128),
            RGBA8::new(217, 234, 251, 255),
        ];
        let source = RavifEncoder::new()
            .with_quality(80.0)
            .with_alpha_quality(100.0)
            .with_speed(10)
            .with_bit_depth(BitDepth::Eight)
            .with_alpha_color_mode(AlphaColorMode::UnassociatedDirty)
            .with_num_threads(Some(1))
            .encode_rgba(Img::new(&pixels, 2, 2))
            .unwrap()
            .avif_file;
        let source_alpha = libavif::decode_rgb(&source)
            .unwrap()
            .as_slice()
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>();
        assert!(
            source_alpha
                .iter()
                .any(|alpha| *alpha != 0 && *alpha != 255)
        );
        for (strategy, candidate) in [
            ("libaom", optimize_aom(&source, 80).unwrap()),
            ("rav1e", optimize_rav1e(&source, 80).unwrap()),
        ] {
            let decoded = libavif::decode_rgb(&candidate).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (2, 2));
            let candidate_alpha = decoded
                .as_slice()
                .chunks_exact(4)
                .map(|pixel| pixel[3])
                .collect::<Vec<_>>();
            assert_eq!(candidate_alpha.len(), source_alpha.len(), "{strategy}");
            assert_eq!(
                (candidate_alpha[0], candidate_alpha[3]),
                (0, 255),
                "{strategy}"
            );
            assert!(
                candidate_alpha
                    .iter()
                    .any(|alpha| *alpha != 0 && *alpha != 255),
                "{strategy} discarded partial transparency"
            );
        }
    }
}
