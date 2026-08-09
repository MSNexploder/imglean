#!/usr/bin/env python3
"""Generate Cargo notices and verify the separately licensed native source."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    metadata = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--locked", "--format-version", "1"],
            cwd=ROOT,
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        ).stdout
    )
    package = next(item for item in metadata["packages"] if item["name"] == "libdeflate-sys")
    vendored_notice = Path(package["manifest_path"]).parent / "libdeflate" / "COPYING"
    checked_notice = ROOT / "licenses" / "libdeflate-COPYING"
    if vendored_notice.read_bytes() != checked_notice.read_bytes():
        raise RuntimeError(
            "licenses/libdeflate-COPYING differs from the vendored libdeflate notice"
        )

    cargo_notices = subprocess.run(
        ["cargo", "about", "generate", "--locked", "about.hbs"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.rstrip()
    native_notice = checked_notice.read_text(encoding="utf-8").rstrip()
    complete = (
        f"{cargo_notices}\n\n"
        f"## libdeflate {package['version']} native code (MIT)\n\n"
        "Used through libdeflate-sys.\n\n"
        f"```text\n{native_notice}\n```\n"
    )
    (ROOT / "THIRD_PARTY_NOTICES.md").write_text(complete, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
