#!/usr/bin/env python3
"""Generate the Cargo SBOM and add native sources vendored outside Cargo."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

from prepare_native import MANIFEST


ROOT = Path(__file__).resolve().parent.parent
OPTIPNG_VERSION = (ROOT / "ci/optipng-version.txt").read_text().strip()
LIBWEBP_VERSION = (ROOT / "ci/libwebp-version.txt").read_text().strip()


def main() -> int:
    document = json.loads(
        subprocess.run(
            ["cargo", "sbom", "--output-format", "spdx_json_2_3"],
            cwd=ROOT,
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        ).stdout
    )
    document["packages"].extend(
        [
            {
                "SPDXID": f"SPDXRef-Package-libwebp-{LIBWEBP_VERSION}",
                "downloadLocation": f"https://storage.googleapis.com/downloads.webmproject.org/releases/webp/libwebp-{LIBWEBP_VERSION}.tar.gz",
                "externalRefs": [
                    {
                        "referenceCategory": "PACKAGE-MANAGER",
                        "referenceLocator": f"pkg:generic/libwebp@{LIBWEBP_VERSION}",
                        "referenceType": "purl",
                    }
                ],
                "homepage": "https://developers.google.com/speed/webp",
                "copyrightText": "NOASSERTION",
                "filesAnalyzed": False,
                "licenseConcluded": "BSD-3-Clause",
                "licenseDeclared": "BSD-3-Clause",
                "name": "libwebp",
                "versionInfo": LIBWEBP_VERSION,
            },
            {
                "SPDXID": "SPDXRef-Package-cexcept-2.99-optipng",
                "downloadLocation": "https://sourceforge.net/projects/cexcept/",
                "externalRefs": [
                    {
                        "referenceCategory": "PACKAGE-MANAGER",
                        "referenceLocator": "pkg:generic/cexcept@2.99-optipng",
                        "referenceType": "purl",
                    }
                ],
                "homepage": "https://sourceforge.net/projects/cexcept/",
                "copyrightText": "NOASSERTION",
                "filesAnalyzed": False,
                "licenseConcluded": "Zlib",
                "licenseDeclared": "Zlib",
                "name": "cexcept",
                "versionInfo": "2.99-optipng",
            },
            {
                "SPDXID": f"SPDXRef-Package-optipng-{OPTIPNG_VERSION}",
                "downloadLocation": (
                    "https://downloads.sourceforge.net/project/optipng/OptiPNG/"
                    f"optipng-{OPTIPNG_VERSION}/optipng-{OPTIPNG_VERSION}.tar.gz"
                ),
                "externalRefs": [
                    {
                        "referenceCategory": "PACKAGE-MANAGER",
                        "referenceLocator": f"pkg:generic/optipng@{OPTIPNG_VERSION}",
                        "referenceType": "purl",
                    }
                ],
                "homepage": "https://optipng.sourceforge.net/",
                "copyrightText": "NOASSERTION",
                "filesAnalyzed": False,
                "licenseConcluded": "Zlib AND Libpng",
                "licenseDeclared": "Zlib AND Libpng",
                "name": "optipng",
                "versionInfo": OPTIPNG_VERSION,
            },
        ]
    )
    document["relationships"].extend(
        [
            {
                "relatedSpdxElement": f"SPDXRef-Package-libwebp-{LIBWEBP_VERSION}",
                "relationshipType": "DEPENDS_ON",
                "spdxElementId": "SPDXRef-Package-libwebp-sys-0.14.4",
            },
            {
                "relatedSpdxElement": f"SPDXRef-Package-optipng-{OPTIPNG_VERSION}",
                "relationshipType": "DEPENDS_ON",
                "spdxElementId": "SPDXRef-Package-imglean-codecs-0.1.0",
            },
            {
                "relatedSpdxElement": "SPDXRef-Package-cexcept-2.99-optipng",
                "relationshipType": "DEPENDS_ON",
                "spdxElementId": f"SPDXRef-Package-optipng-{OPTIPNG_VERSION}",
            },
        ]
    )
    cargo_ids = {package["name"]: package["SPDXID"] for package in document["packages"]}
    sources = json.loads(MANIFEST.read_text())
    native_ids = {source["name"]: f"SPDXRef-Native-{source['name']}" for source in sources}
    for source in sources:
        document["packages"].append({
            "SPDXID": native_ids[source["name"]], "name": source["name"],
            "versionInfo": source["version"], "downloadLocation": source["url"],
            "filesAnalyzed": False, "copyrightText": "NOASSERTION",
            "licenseConcluded": source["license"], "licenseDeclared": source["license"],
            "checksums": [{"algorithm": "SHA256", "checksumValue": source["sha256"]}],
        })
    for parent, child in [
        (cargo_ids["imglean-codecs"], "libpng"),
        (f"SPDXRef-Package-optipng-{OPTIPNG_VERSION}", "libpng"),
        (cargo_ids["imglean-avif"], "libavif"),
        (native_ids["libavif"], "libaom"),
        (cargo_ids["imglean-codecs"], "jpegli"),
        (native_ids["jpegli"], "highway"),
        (native_ids["jpegli"], "libjpeg-turbo-headers"),
        (native_ids["jpegli"], "skcms"),
    ]:
        document["relationships"].append({
            "spdxElementId": parent, "relatedSpdxElement": native_ids[child],
            "relationshipType": "DEPENDS_ON",
        })
    document["relationships"].append({
        "spdxElementId": native_ids["libpng"],
        "relatedSpdxElement": cargo_ids["libz-sys"],
        "relationshipType": "DEPENDS_ON",
    })
    for name, version, license_id, owner in [
        ("libavif", "0.14.0", "BSD-2-Clause", "imglean-avif"),
        ("jpegli", "0.1.0", "BSD-3-Clause", "imglean-codecs"),
    ]:
        adapted_id = f"SPDXRef-Adapted-{name}"
        document["packages"].append({
            "SPDXID": adapted_id, "name": f"{name}-rust-adapted", "versionInfo": version,
            "downloadLocation": f"https://crates.io/crates/{name}/{version}",
            "filesAnalyzed": False, "copyrightText": "NOASSERTION",
            "licenseConcluded": license_id, "licenseDeclared": license_id,
            "comment": "Only the operations used by ImgLean are adapted into local source.",
        })
        document["relationships"].append({
            "spdxElementId": cargo_ids[owner], "relatedSpdxElement": adapted_id,
            "relationshipType": "CONTAINS",
        })
    print(json.dumps(document, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
