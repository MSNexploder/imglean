#!/usr/bin/env python3
"""Generate only ImgLean's native API declarations with bindgen-cli 0.73.2.

Pass --jpegli-include the lib/include/jpegli directory from the pinned native
CMake build. Normal builds use the checked-in declarations and need no bindgen.
"""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path

from prepare_native import ROOT, SOURCES


def generate(header: Path, functions: list[str], constants: list[str], options: list[str]) -> str:
    return subprocess.check_output([
        "bindgen", str(header), "--allowlist-function", "^(" + "|".join(functions) + ")$",
        "--allowlist-var", "^(" + "|".join(constants) + ")$",
        "--no-prepend-enum-name", "--no-layout-tests", "--no-doc-comments",
        "--with-derive-default", *options,
    ], text=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jpegli-include", required=True, type=Path)
    arguments = parser.parse_args()
    if subprocess.check_output(["bindgen", "--version"], text=True).strip() != "bindgen 0.73.2":
        raise RuntimeError("native declarations require bindgen-cli 0.73.2")
    avif = generate(
        SOURCES / "libavif/include/avif/avif.h",
        ["avifImageCreate", "avifImageCreateEmpty", "avifImageDestroy", "avifImageAllocatePlanes",
         "avifDecoderCreate", "avifDecoderReadMemory", "avifDecoderDestroy",
         "avifRGBImageSetDefaults", "avifRGBImageAllocatePixels", "avifImageYUVToRGB",
         "avifRGBImageFreePixels", "avifEncoderCreate", "avifEncoderDestroy",
         "avifEncoderSetCodecSpecificOption", "avifEncoderWrite", "avifRWDataFree"],
        ["AVIF_RESULT_OK", "AVIF_RGB_FORMAT_RGBA", "AVIF_PIXEL_FORMAT_YUV400", "AVIF_PLANES_YUV"],
        ["--allowlist-type", "avifPlanesFlag", "--blocklist-type",
         "avif(Decoder|GainMap|EncoderData|CodecSpecificOptions)"],
    )
    jpegli = generate(
        SOURCES / "jpegli/lib/jpegli/decode.h",
        ["jpegli_CreateCompress", "jpegli_CreateDecompress", "jpegli_destroy_compress",
         "jpegli_destroy_decompress", "jpegli_finish_compress", "jpegli_finish_decompress",
         "jpegli_read_header", "jpegli_read_scanlines", "jpegli_resync_to_restart",
         "jpegli_save_markers", "jpegli_set_defaults", "jpegli_set_quality",
         "jpegli_simple_progression", "jpegli_start_compress", "jpegli_start_decompress",
         "jpegli_std_error", "jpegli_write_marker", "jpegli_write_scanlines"],
        ["JPEG_LIB_VERSION", "JPEG_APP0", "JPEG_COM", "JMSG_LENGTH_MAX",
         "JERR_BUFFER_SIZE", "JERR_FILE_READ", "JWRN_JPEG_EOF"],
        ["--rustified-enum", "J_COLOR_SPACE", "--override-abi", ".*=C-unwind",
         "--allowlist-type", "J_MESSAGE_CODE",
         "--blocklist-type", "jpeg_(component_info|scan_info|memory_mgr|progress_mgr)",
         "--blocklist-type", "JQUANT_TBL|JHUFF_TBL", "--", "-x", "c++", "-std=c++17",
         "-I" + str(SOURCES / "jpegli"), "-I" + str(arguments.jpegli_include.resolve()),
         "-I" + str(SOURCES / "jpegli/third_party/libjpeg-turbo/src"),
         "-include", str(SOURCES / "jpegli/lib/jpegli/encode.h"),
         "-include", str(SOURCES / "jpegli/third_party/libjpeg-turbo/src/jerror.h")],
    )
    # jconfig.h.in selects an unsigned-char boolean on Windows and int elsewhere.
    # Keep that native platform contract while generating on a Unix host.
    boolean = "pub type boolean = ::std::os::raw::c_int;"
    if jpegli.count(boolean) != 1:
        raise RuntimeError("native Jpegli boolean declaration changed")
    jpegli = jpegli.replace(boolean, '#[cfg(windows)]\npub type boolean = ::std::os::raw::c_uchar;\n'
                            '#[cfg(not(windows))]\n' + boolean)
    # bindgen emits every member of the C error-code enum when one is requested.
    # Retain its integer representation and only the codes our callbacks use.
    jpegli = re.sub(r"^pub const (JERR_\w+|JWRN_\w+|JTRC_\w+|JMSG_\w+): J_MESSAGE_CODE = .*;\n",
                    lambda match: match[0] if match[1] in {"JERR_BUFFER_SIZE", "JERR_FILE_READ", "JWRN_JPEG_EOF"} else "",
                    jpegli, flags=re.MULTILINE)
    # These native objects occur only behind pointers. ImgLean never allocates
    # or dereferences their layouts, so use genuinely opaque forward declarations.
    for name in ["avifDecoder", "avifGainMap", "avifEncoderData", "avifCodecSpecificOptions"]:
        avif += f"\n#[repr(C)]\npub struct {name} {{ _private: [u8; 0] }}\n"
    for name in ["jpeg_component_info", "jpeg_scan_info", "jpeg_memory_mgr", "jpeg_progress_mgr",
                 "JQUANT_TBL", "JHUFF_TBL"]:
        jpegli += f"\n#[repr(C)]\npub struct {name} {{ _private: [u8; 0] }}\n"
    for relative, contents in [("crates/imglean-avif/src/ffi.rs", avif),
                               ("crates/imglean-codecs/src/jpegli_ffi.rs", jpegli)]:
        (ROOT / relative).write_text(contents)


if __name__ == "__main__":
    main()
