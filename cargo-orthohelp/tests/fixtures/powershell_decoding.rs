//! Byte-order-mark (BOM) aware decoders for `PowerShell` output.
//!
//! Windows `PowerShell` picks an output encoding per host and per
//! redirect, not per caller. Windows PowerShell 5.1 (`powershell.exe`)
//! writes UTF-16LE with a BOM when its output is captured, while
//! `pwsh` writes UTF-8. Any host may also drop the BOM when it tunnels a
//! pipeline through `-Command`, which leaves byte-order detection with
//! nothing to read.
//!
//! These decoders are free functions over `&[u8]` rather than part of the
//! Windows-only integration test. That test cannot run anywhere but
//! Windows, and when `pwsh` is absent it declines to exercise the
//! decoders at all, so the buffering rules below would be untested on
//! every other machine in the estate. Called from here they are compiled
//! and tested on every platform.

/// Byte order of a UTF-16 code unit stream.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Endianness {
    /// Least significant byte first, the order Windows `PowerShell` writes.
    Little,
    /// Most significant byte first, which matches the `U+FEFF` BOM bytes.
    Big,
}

/// Decodes `bytes` as UTF-16 of the given byte order, after verifying a BOM.
///
/// A missing or different BOM means the bytes are not UTF-16 in the order
/// the caller asked for, and the answer is `None` rather than a decoding
/// of whatever they are.
///
/// Trailing bytes that do not complete a code unit are discarded, and a
/// code unit with no valid decoding becomes `U+FFFD`. That is the policy a
/// terminal transcript wants: `PowerShell` help output is displayed, not
/// parsed, so a damaged glyph is preferable to losing the surrounding
/// text to an error.
pub(crate) fn decode_utf16(bytes: &[u8], bom: [u8; 2], endian: Endianness) -> Option<String> {
    let rest = bytes.strip_prefix(&bom)?;
    let decoded = rest
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| to_u16(*pair, endian))
        .collect::<Vec<_>>();
    Some(String::from_utf16_lossy(&decoded))
}

/// Decodes BOM-less output that is probably UTF-16LE.
///
/// Windows PowerShell 5.1 can emit UTF-16LE output without a BOM when
/// piping `Get-Help ... | Out-String` through `-Command`. Treating such
/// bytes as ASCII-heavy text would leave a NUL between every character,
/// so this fallback accepts them — but only when the evidence supports
/// UTF-16LE and the result decodes cleanly.
///
/// The evidence is the zero high bytes that Latin text produces in
/// UTF-16LE: at least half of the code units must carry one. A UTF-8
/// document satisfying that by coincidence is vanishingly unlikely to
/// also be valid UTF-16 and is still refused if it is not.
///
/// Unlike [`decode_utf16`], an odd trailing byte rejects the input rather
/// than being dropped. Here nothing has vouched for the encoding, so an
/// incomplete code unit is evidence against the guess rather than damage
/// within a known one.
pub(crate) fn decode_probable_utf16le_without_bom(bytes: &[u8]) -> Option<String> {
    let (pairs, remainder) = bytes.as_chunks::<2>();
    if pairs.is_empty() || !remainder.is_empty() {
        return None;
    }

    let pair_count = pairs.len();
    // A code unit is stored low byte then high byte, so pair[1] is the
    // high byte, and it is zero for every code point below U+0100.
    let zero_high_byte_count = pairs.iter().filter(|pair| pair[1] == 0).count();
    if zero_high_byte_count * 2 < pair_count {
        return None;
    }

    let decoded = pairs
        .iter()
        .map(|pair| to_u16(*pair, Endianness::Little))
        .collect::<Vec<_>>();
    String::from_utf16(&decoded).ok()
}

/// Joins two bytes into one UTF-16 code unit in the given byte order.
///
/// The pair is taken by value: it is two bytes, so a reference costs more
/// to pass than the array it points at.
pub(crate) const fn to_u16(pair: [u8; 2], endian: Endianness) -> u16 {
    let [first, second] = pair;
    match endian {
        Endianness::Little => (first as u16) | ((second as u16) << 8),
        Endianness::Big => ((first as u16) << 8) | (second as u16),
    }
}

#[cfg(test)]
mod tests {
    //! Decoder coverage that does not depend on a `PowerShell` host being
    //! installed, or on the platform being Windows at all.

    use super::{Endianness, decode_probable_utf16le_without_bom, decode_utf16};
    use rstest::rstest;

    /// Encodes text as UTF-16 in the given byte order, with a BOM.
    fn with_bom(text: &str, endian: Endianness) -> Vec<u8> {
        let bom: [u8; 2] = match endian {
            Endianness::Little => [0xFF, 0xFE],
            Endianness::Big => [0xFE, 0xFF],
        };
        let mut bytes = bom.to_vec();
        bytes.extend(encoded_units(text, endian));
        bytes
    }

    /// Encodes text as UTF-16 in the given byte order, without a BOM.
    fn without_bom(text: &str, endian: Endianness) -> Vec<u8> {
        encoded_units(text, endian)
    }

    /// Splits each code unit into bytes in the requested order.
    ///
    /// The split is written out rather than taken from `to_le_bytes` /
    /// `to_be_bytes`: the workspace denies those so that byte order is
    /// always an explicit, reviewed decision. Here it is the subject of
    /// the tests, so naming the order in the code is the point.
    fn encoded_units(text: &str, endian: Endianness) -> Vec<u8> {
        let low_first = |unit: u16| [(unit & 0xFF) as u8, (unit >> 8) as u8];
        let high_first = |unit: u16| [(unit >> 8) as u8, (unit & 0xFF) as u8];
        text.encode_utf16()
            .flat_map(|unit| match endian {
                Endianness::Little => low_first(unit),
                Endianness::Big => high_first(unit),
            })
            .collect()
    }

    #[rstest]
    #[case::little(Endianness::Little, [0xFF, 0xFE])]
    #[case::big(Endianness::Big, [0xFE, 0xFF])]
    fn bom_selects_the_byte_order(#[case] endian: Endianness, #[case] bom: [u8; 2]) {
        let bytes = with_bom("Get-Help", endian);

        assert_eq!(
            decode_utf16(&bytes, bom, endian).as_deref(),
            Some("Get-Help")
        );
    }

    #[rstest]
    #[case::little_bom_read_as_big([0xFF, 0xFE], Endianness::Big)]
    #[case::big_bom_read_as_little([0xFE, 0xFF], Endianness::Little)]
    fn a_bom_for_the_other_order_is_refused(#[case] bom: [u8; 2], #[case] endian: Endianness) {
        let bytes = with_bom("Get-Help", Endianness::Little);

        assert_eq!(decode_utf16(&bytes, bom, endian), None);
    }

    #[test]
    fn a_missing_bom_is_refused() {
        let bytes = without_bom("Get-Help", Endianness::Little);

        assert_eq!(decode_utf16(&bytes, [0xFF, 0xFE], Endianness::Little), None);
    }

    #[test]
    fn an_empty_payload_after_the_bom_decodes_to_nothing() {
        assert_eq!(
            decode_utf16(&[0xFF, 0xFE], [0xFF, 0xFE], Endianness::Little).as_deref(),
            Some("")
        );
    }

    #[test]
    fn a_trailing_half_unit_is_discarded() {
        let mut bytes = with_bom("ok", Endianness::Little);
        bytes.push(b'x');

        assert_eq!(
            decode_utf16(&bytes, [0xFF, 0xFE], Endianness::Little).as_deref(),
            Some("ok")
        );
    }

    #[test]
    fn an_unpaired_surrogate_decodes_lossily() {
        let mut bytes = with_bom("", Endianness::Little);
        // A high surrogate with no low surrogate after it.
        bytes.extend([0x00, 0xD8]);

        let decoded = decode_utf16(&bytes, [0xFF, 0xFE], Endianness::Little);

        assert_eq!(decoded.as_deref(), Some("\u{FFFD}"));
    }

    #[rstest]
    #[case::ascii("Get-Help fixture command", true)]
    #[case::accented("Caf\u{e9} cr\u{e8}me", true)]
    #[case::empty("", false)]
    #[case::cjk("\u{65e5}\u{672c}\u{8a9e}", false)]
    #[case::exactly_half_ascii("\u{65e5}a", true)]
    #[case::just_under_half_ascii("\u{65e5}a\u{672c}", false)]
    fn boms_less_detection_accepts_only_ascii_heavy_output(
        #[case] text: &str,
        #[case] is_probable: bool,
    ) {
        let bytes = without_bom(text, Endianness::Little);
        let decoded = decode_probable_utf16le_without_bom(&bytes);

        assert_eq!(decoded.is_some(), is_probable, "text: {text:?}");
    }

    #[test]
    fn boms_less_detection_rejects_a_single_odd_byte() {
        assert_eq!(decode_probable_utf16le_without_bom(b"a"), None);
    }

    #[test]
    fn boms_less_detection_rejects_an_odd_trailer() {
        let mut bytes = without_bom("Get-Help", Endianness::Little);
        bytes.push(b'x');

        assert_eq!(decode_probable_utf16le_without_bom(&bytes), None);
    }

    #[test]
    fn boms_less_detection_refuses_utf8_fallback_bytes() {
        // UTF-8 prose, the input that must reach `from_utf8_lossy` instead.
        assert_eq!(
            decode_probable_utf16le_without_bom(b"Orthohelp fixture"),
            None
        );
    }

    #[test]
    fn boms_less_detection_rejects_an_unpaired_surrogate_round_trip() {
        // Parity in length, zero high bytes, but not decodable UTF-16.
        assert_eq!(decode_probable_utf16le_without_bom(&[0x00, 0xD8]), None);
    }
}
