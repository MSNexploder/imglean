use std::ffi::OsStr;

pub fn escape_worker_text(bytes: &[u8]) -> String {
    let mut escaped = String::new();
    let mut remaining = bytes;

    while !remaining.is_empty() {
        match std::str::from_utf8(remaining) {
            Ok(text) => {
                push_text(&mut escaped, text);
                break;
            }
            Err(error) => {
                let valid = error.valid_up_to();
                if let Ok(valid_text) = std::str::from_utf8(&remaining[..valid]) {
                    push_text(&mut escaped, valid_text);
                }
                let invalid_length = error.error_len().unwrap_or(remaining.len() - valid);
                for byte in &remaining[valid..valid + invalid_length] {
                    push_byte_escape(&mut escaped, *byte);
                }
                remaining = &remaining[valid + invalid_length..];
            }
        }
    }

    escaped
}

#[cfg(unix)]
pub fn escape_path(path: &OsStr) -> String {
    use std::os::unix::ffi::OsStrExt;

    escape_worker_text(path.as_bytes())
}

#[cfg(windows)]
pub fn escape_path(path: &OsStr) -> String {
    use std::os::windows::ffi::OsStrExt;

    let mut escaped = String::new();
    for item in char::decode_utf16(path.encode_wide()) {
        match item {
            Ok(character) => push_character(&mut escaped, character),
            Err(error) => {
                escaped.push_str("\\x{");
                use std::fmt::Write as _;
                let _ = write!(escaped, "{:04X}", error.unpaired_surrogate());
                escaped.push('}');
            }
        }
    }
    escaped
}

fn push_text(output: &mut String, text: &str) {
    for character in text.chars() {
        push_character(output, character);
    }
}

fn push_character(output: &mut String, character: char) {
    match character {
        '\n' => output.push_str("\\n"),
        '\r' => output.push_str("\\r"),
        '\t' => output.push_str("\\t"),
        '\0' => output.push_str("\\0"),
        value if value.is_control() => {
            output.push_str("\\u{");
            use std::fmt::Write as _;
            let _ = write!(output, "{:X}", value as u32);
            output.push('}');
        }
        value => output.push(value),
    }
}

fn push_byte_escape(output: &mut String, byte: u8) {
    output.push_str("\\x");
    use std::fmt::Write as _;
    let _ = write!(output, "{byte:02X}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_text_is_one_physical_line() {
        assert_eq!(
            escape_worker_text(b"first\nsecond\r\t\0"),
            "first\\nsecond\\r\\t\\0"
        );
    }

    #[test]
    fn invalid_utf8_uses_hexadecimal_bytes() {
        assert_eq!(escape_worker_text(b"a\xFFb"), "a\\xFFb");
    }

    #[test]
    fn unicode_control_uses_visible_escape() {
        assert_eq!(escape_worker_text("a\u{0085}b".as_bytes()), "a\\u{85}b");
    }

    #[cfg(unix)]
    #[test]
    fn invalid_unix_path_unit_uses_hexadecimal_byte() {
        use std::os::unix::ffi::OsStrExt;

        assert_eq!(escape_path(OsStr::from_bytes(b"a\xFF.png")), "a\\xFF.png");
    }
}
