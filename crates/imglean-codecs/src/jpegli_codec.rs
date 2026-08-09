use jpegli::{ColorSpace, Compress, Decompress, Marker};

const PRESERVED_MARKERS: &[Marker] = &[
    Marker::APP(0),
    Marker::APP(1),
    Marker::APP(2),
    Marker::APP(3),
    Marker::APP(4),
    Marker::APP(5),
    Marker::APP(6),
    Marker::APP(7),
    Marker::APP(8),
    Marker::APP(9),
    Marker::APP(10),
    Marker::APP(11),
    Marker::APP(12),
    Marker::APP(13),
    Marker::APP(14),
    Marker::APP(15),
    Marker::COM,
];

pub fn optimize(source: &[u8], quality: u8, strip_metadata: bool) -> Result<Vec<u8>, ()> {
    let decompressor = Decompress::with_markers(PRESERVED_MARKERS)
        .from_mem(source)
        .map_err(|_| ())?;
    let size = decompressor.size();
    let grayscale = decompressor.color_space() == ColorSpace::JCS_GRAYSCALE;
    let markers: Vec<_> = if strip_metadata {
        Vec::new()
    } else {
        decompressor
            .markers()
            .map(|marker| (marker.marker, marker.data.to_vec()))
            .collect()
    };
    let pixels = if grayscale {
        let mut started = decompressor.grayscale().map_err(|_| ())?;
        let pixels = started.read_scanlines::<u8>().map_err(|_| ())?;
        started.finish().map_err(|_| ())?;
        pixels
    } else {
        let mut started = decompressor.rgb().map_err(|_| ())?;
        let pixels = started.read_scanlines::<u8>().map_err(|_| ())?;
        started.finish().map_err(|_| ())?;
        pixels
    };

    let mut compressor = Compress::new(if grayscale {
        ColorSpace::JCS_GRAYSCALE
    } else {
        ColorSpace::JCS_RGB
    });
    compressor.set_size(size.0, size.1);
    compressor.set_quality(f32::from(quality));
    compressor.set_progressive_mode();
    compressor.set_optimize_coding(true);
    let mut started = compressor.start_compress(Vec::new()).map_err(|_| ())?;
    for (marker, data) in markers {
        if !is_structural_marker(marker, &data) {
            started.write_marker(marker, &data);
        }
    }
    started.write_scanlines(&pixels).map_err(|_| ())?;
    started.finish().map_err(|_| ())
}

fn is_structural_marker(marker: Marker, data: &[u8]) -> bool {
    marker == Marker::APP(14) && data.starts_with(b"Adobe")
}
