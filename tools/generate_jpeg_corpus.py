#!/usr/bin/env python3
"""Generate the bounded JPEG validation corpus from fixed binary fixtures."""

from __future__ import annotations

import base64
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1] / "tests" / "corpus" / "jpeg" / "v1"

BASELINE = base64.b64decode(
    "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAMCAgMCAgMDAwMEAwMEBQgFBQQEBQoHBwYIDAoMDAsKCwsNDhIQDQ4RDgsLEBYQERMUFRUVDA8XGBYUGBIUFRT/2wBDAQMEBAUEBQkFBQkUDQsNFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBT/wAARCAAQABADAREAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwD5Z8C/A7/V/wCj/pXo4OpscfDnFHw+8fQ/gX4Hf6v/AEf9K+4wdTY/qPhzij4feN3wL8UPhH+7/wCKl9P+YXef/Ga8/B8GZ/p/s/8A5PD/AOSP85+HMj4o93/Zv/J6f/yZ7Lo/xx+EUG21tfE/73o8g0y8+X2H7nr/AC+vT8i49zbP8v58kySl++2qVFOHud4xfN8f80vsbL37uH9d8IcL8UT5alTDadFz09f/ACfb8/Tf/9k="
)
PROGRESSIVE = base64.b64decode(
    "/9j/4AAQSkZJRgABAQAAAQABAAD/2wCEAAMDAwMDAwQEBAQFBQUFBQcHBgYHBwsICQgJCAsRCwwLCwwLEQ8SDw4PEg8bFRMTFRsfGhkaHyYiIiYwLTA+PlQBAwMDAwMDBAQEBAUFBQUFBwcGBgcHCwgJCAkICxELDAsLDAsRDxIPDg8SDxsVExMVGx8aGRofJiIiJjAtMD4+VP/CABEIABAAEAMBEQACEQEDEQH/xAAtAAEBAQAAAAAAAAAAAAAAAAAIBQcBAAIDAQAAAAAAAAAAAAAAAAUIBAYHCf/aAAwDAQACEAMQAAAALJGGh7w0d0fzn2XIm6//xAAnEAAABAMGBwAAAAAAAAAAAAAABAYHBSIjAQIDETKhFCUxQmGk0//aAAgBAQABPwBMtpoo7BMtpoo7BMLVo5Oeeia+Qh7ltFhZFy8en77/AAJqXxZS6j//xAAoEQAABAQCCwAAAAAAAAAAAAADBAUGAAIHMSEkARITJTNCUVJxo9L/2gAIAQIBAT8ATxbQni2hPp29MN2e8H7iqK89ErbN5vFM7jIdOyGQcv1DD06/F7puTzb/xAAkEQABAgQFBQAAAAAAAAAAAAAGAAMCByMzBRMUISIlQqKk0v/aAAgBAwEBPwARNrdRCJtbqIRGZhU+l+0x9oBCZhR5Tr2Fce2HVMb+a//Z"
)
GRAYSCALE = base64.b64decode(
    "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAMCAgMCAgMDAwMEAwMEBQgFBQQEBQoHBwYIDAoMDAsKCwsNDhIQDQ4RDgsLEBYQERMUFRUVDA8XGBYUGBIUFRT/wAALCAAIAAgBAREA/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/9oACAEBAAA/AD4SwaX+yr8DpfGd7Z+dqsmNP0K0a1aaO51J43aBJArLiIeWzuS6/IjBSXKq3//Z"
)
DIMENSIONS = base64.b64decode(
    "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAMCAgMCAgMDAwMEAwMEBQgFBQQEBQoHBwYIDAoMDAsKCwsNDhIQDQ4RDgsLEBYQERMUFRUVDA8XGBYUGBIUFRT/2wBDAQMEBAUEBQkFBQkUDQsNFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBT/wAARCAAQABEDAREAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwD5Z8C/A7/V/wCj/pXo4OpscfDnFHw+8fQ/gX4Hf6v/AEf9K+4wdTY/qPhzij4fePn/AP4Ud/07/pX9J+0P4F/1o/vH074F+KHwj/d/8VL6f8wu8/8AjNfy5g+DM/0/2f8A8nh/8keDw5kfFHu/7N/5PT/+TPZdH+OPwig22tr4n/e9HkGmXny+w/c9f5fXp+Rce5tn+X8+SZJS/fbVKinD3O8Yvm+P+aX2Nl793D+u+EOF+KJ8tSphtOi56ev/AJPt+fpv8p/8LQ+EX/Qzf+Uy8/8AjNfI/wCpmf8A/QP/AOTw/wDkj+JP7D4o/wCgb/ypT/8Akz//2Q=="
)


def insert_segment(jpeg: bytes, marker: int, payload: bytes) -> bytes:
    segment = b"\xff" + bytes([marker]) + (len(payload) + 2).to_bytes(2, "big") + payload
    return jpeg[:2] + segment + jpeg[2:]


def write(group: str, name: str, contents: bytes) -> None:
    directory = ROOT / group
    directory.mkdir(parents=True, exist_ok=True)
    (directory / name).write_bytes(contents)


def main() -> None:
    write("accepted", "baseline.jpg", BASELINE)
    write("accepted", "progressive.jpg", PROGRESSIVE)
    write("accepted", "grayscale.jpg", GRAYSCALE)
    write("accepted", "provider-reduction.jpg", BASELINE)
    write("changed", "dimensions.jpg", DIMENSIONS)

    write(
        "rejected",
        "xmp.jpg",
        insert_segment(BASELINE, 0xE1, b"http://ns.adobe.com/xap/1.0/\x00payload"),
    )
    write("rejected", "app11.jpg", insert_segment(BASELINE, 0xEB, b"JUMBF/c2pa"))
    write("rejected", "trailing.jpg", BASELINE + b"trailing")
    write("rejected", "truncated.jpg", BASELINE[:-3])

    invalid_scan = bytearray(BASELINE)
    sos = invalid_scan.index(b"\xff\xda")
    scan_start = sos + 2 + int.from_bytes(invalid_scan[sos + 2 : sos + 4], "big")
    invalid_scan[scan_start : scan_start + 2] = b"\xff\xd8"
    write("rejected", "invalid-scan.jpg", bytes(invalid_scan))


if __name__ == "__main__":
    main()
