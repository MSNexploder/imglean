#!/usr/bin/env python3
"""Generate the Cargo SBOM and add native sources vendored outside Cargo."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path


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
                "SPDXID": "SPDXRef-Package-highway-1.1.0",
                "downloadLocation": "https://github.com/google/highway/releases/tag/1.1.0",
                "externalRefs": [
                    {
                        "referenceCategory": "PACKAGE-MANAGER",
                        "referenceLocator": "pkg:generic/highway@1.1.0",
                        "referenceType": "purl",
                    }
                ],
                "homepage": "https://github.com/google/highway",
                "copyrightText": "NOASSERTION",
                "filesAnalyzed": False,
                "licenseConcluded": "Apache-2.0",
                "licenseDeclared": "Apache-2.0",
                "name": "highway",
                "versionInfo": "1.1.0",
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
                "relatedSpdxElement": "SPDXRef-Package-highway-1.1.0",
                "relationshipType": "DEPENDS_ON",
                "spdxElementId": "SPDXRef-Package-jpegli-sys-0.1.0-plus-0.10.2",
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
            {
                "relatedSpdxElement": "SPDXRef-Package-libpng-sys-1.1.11",
                "relationshipType": "DEPENDS_ON",
                "spdxElementId": f"SPDXRef-Package-optipng-{OPTIPNG_VERSION}",
            },
        ]
    )
    print(json.dumps(document, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
