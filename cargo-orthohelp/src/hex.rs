//! Lowercase hexadecimal rendering for digest bytes.
//!
//! `sha2` 0.11 returns `hybrid_array::Array<u8, _>` from `finalize` and
//! `digest`, and that type does not implement [`core::fmt::LowerHex`], so the
//! `{:x}` formatting the cache keys previously relied on no longer compiles.
//! This helper renders the raw digest bytes without pulling in a dedicated
//! hex-encoding dependency.
//!
//! Scope and re-use policy: the encoder is crate-internal to
//! `cargo-orthohelp` and exists solely to render digests produced by
//! [`crate::cache`]. It owns no state and imposes no ordering requirements, so
//! any crate-internal call site that needs lowercase hex may use it; new
//! digest-rendering code must call it rather than reintroducing per-byte
//! `format!` calls. It deliberately stays private: `cargo-orthohelp` exposes no
//! hex-encoding contract to downstream crates, and a public helper would
//! commit the crate to one.

/// Encode `bytes` as a lowercase hexadecimal string.
///
/// Every byte is rendered as exactly two digits, including leading zeroes, so
/// the returned string is always twice the length of `bytes`. The output buffer
/// is preallocated and each nibble is mapped arithmetically, avoiding both a
/// per-byte `format!` allocation and a fallible table index.
///
/// # Examples
///
/// `to_lower_hex(&[0x0a, 0xff])` returns `"0aff"`, and `to_lower_hex(&[])`
/// returns `""`. The example is stated in prose rather than as a doctest
/// because the helper is crate-private, so rustdoc would not compile or run
/// it.
#[must_use]
pub(crate) fn to_lower_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        hex.push(hex_digit(byte >> 4));
        hex.push(hex_digit(byte & 0x0f));
    }
    hex
}

/// Map the low four bits of `byte` to a lowercase hexadecimal digit.
///
/// The mapping is arithmetic rather than a table lookup so that no indexing
/// operation can panic: masking to four bits keeps the value in `0..=15`, and
/// both branches then stay within the ASCII digit and `a`-`f` ranges. Any
/// higher bits are discarded, so the function is total over `u8`.
fn hex_digit(byte: u8) -> char {
    let nibble = byte & 0x0f;
    let digit = if nibble < 10 {
        b'0' + nibble
    } else {
        b'a' + nibble - 10
    };
    char::from(digit)
}

#[cfg(test)]
mod tests {
    //! Tests for the lowercase hexadecimal digest formatter.

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::empty(&[], "")]
    #[case::leading_zeroes(&[0x00, 0x0f, 0xff, 0xa0], "000fffa0")]
    #[case::ascending(&[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef], "0123456789abcdef")]
    fn renders_expected_lowercase_digits(#[case] bytes: &[u8], #[case] expected: &str) {
        assert_eq!(to_lower_hex(bytes), expected);
    }

    #[rstest]
    fn every_byte_renders_as_two_lowercase_round_tripping_digits() {
        // Bounded exhaustive check over the whole u8 range: each byte must
        // render as exactly two lowercase ASCII hex digits that parse back to
        // the original value. Example vectors alone would miss leading-zero
        // and nibble-ordering regressions for most of the range.
        for byte in u8::MIN..=u8::MAX {
            let rendered = to_lower_hex(&[byte]);
            assert_eq!(rendered.len(), 2, "byte {byte:#04x} must render two digits");
            assert!(
                rendered
                    .bytes()
                    .all(|digit| digit.is_ascii_digit() || (b'a'..=b'f').contains(&digit)),
                "byte {byte:#04x} rendered non-lowercase-hex output {rendered:?}",
            );
            let parsed = u8::from_str_radix(&rendered, 16).expect("two hex digits parse as a byte");
            assert_eq!(parsed, byte, "round-trip mismatch for byte {byte:#04x}");
        }
    }

    #[rstest]
    fn hex_digit_ignores_bits_above_the_low_nibble() {
        // The helper masks its input, so every byte must render the same digit
        // as its low nibble alone. That keeps the arithmetic mapping total over
        // `u8` without an indexing operation that could panic.
        for byte in u8::MIN..=u8::MAX {
            assert_eq!(
                hex_digit(byte),
                hex_digit(byte & 0x0f),
                "byte {byte:#04x} should render its low nibble"
            );
        }
    }

    #[rstest]
    fn output_length_is_twice_the_input_length() {
        let bytes: Vec<u8> = (0..64).collect();
        assert_eq!(to_lower_hex(&bytes).len(), bytes.len() * 2);
    }
}
