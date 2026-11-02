use ikigai_fn::html::{percent_decode, percent_decode_bytes};

#[test]
fn shared_decoder_is_strict_and_preserves_bytes_on_request() {
    assert_eq!(percent_decode("caf%C3%A9+a", true).unwrap(), "café a");
    assert_eq!(percent_decode("a+b", false).unwrap(), "a+b");
    for input in ["%", "%G1", "%1"] {
        assert!(percent_decode(input, false).is_err());
        assert!(percent_decode_bytes(input.as_bytes(), false).is_err());
    }
    assert!(percent_decode("%FF", false).is_err());
    assert_eq!(percent_decode_bytes(b"%FF", false).unwrap(), [255]);
}

mod intake {
    include!("fixtures/intake_percent_decode.rs");
    pub fn decode(input: &str) -> String {
        percent_decode(input)
    }
}
mod embedded {
    include!("fixtures/embedded_percent_decode.rs");
    pub fn decode(input: &str) -> String {
        percent_decode(input)
    }
}
mod web {
    include!("fixtures/web_percent_decode.rs");
    pub fn decode(input: &[u8], plus: bool) -> Result<Vec<u8>, &'static str> {
        percent_decode(input, plus)
    }
}

// The intake and embedded copies always use form semantics. They differ only on
// malformed escapes (literal passthrough) and invalid UTF-8 (lossy replacement).
fn compare_cli_copies(input: &str) {
    use ikigai_fn::html::PercentDecodeError;
    let legacy = intake::decode(input);
    assert_eq!(legacy, embedded::decode(input), "{input:?}");
    match percent_decode(input, true) {
        Ok(decoded) => assert_eq!(decoded, legacy, "{input:?}"),
        Err(PercentDecodeError::InvalidUtf8(_)) => {
            let bytes = percent_decode_bytes(input.as_bytes(), true).unwrap();
            assert_eq!(legacy, String::from_utf8_lossy(&bytes), "{input:?}");
        }
        Err(PercentDecodeError::MalformedEscape { .. }) => {
            assert!(input.contains('%'));
            assert!(web::decode(input.as_bytes(), true).is_err());
        }
    }
    for plus in [false, true] {
        assert_eq!(
            percent_decode_bytes(input.as_bytes(), plus).ok(),
            web::decode(input.as_bytes(), plus).ok(),
            "web: {input:?}, plus_is_space={plus}"
        );
    }
}

#[test]
fn cli_copies_agree_or_have_the_documented_strictness_difference() {
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("vectors/html_vectors.json")).unwrap();
    for key in ["percent_decode", "url_segment"] {
        for case in vectors[key].as_array().unwrap() {
            compare_cli_copies(case["in"].as_str().unwrap());
            if key == "url_segment" {
                compare_cli_copies(case["out"].as_str().unwrap());
            }
        }
    }
    // Every encoded byte, both hex cases, alone and between literal Unicode/plus.
    for byte in 0..=255u8 {
        compare_cli_copies(&format!("%{byte:02X}"));
        compare_cli_copies(&format!("日+%{byte:02x}+é"));
    }
    // Every possible ASCII pair after a percent, including malformed pairs.
    for hi in 0..=127u8 {
        for lo in 0..=127u8 {
            compare_cli_copies(&format!("x%{}{}+%41", char::from(hi), char::from(lo)));
        }
    }
}

#[test]
fn legacy_malformed_passthrough_and_lossy_utf8_are_intentional_differences() {
    for (input, legacy) in [
        ("%", "%"),
        ("%G1", "%G1"),
        ("%1", "%1"),
        ("%41%+%42", "A% B"),
        ("%FF", "�"),
        ("%C3%28", "�("),
    ] {
        assert_eq!(intake::decode(input), legacy);
        assert_eq!(embedded::decode(input), legacy);
        assert!(percent_decode(input, true).is_err());
    }
}

#[test]
fn bytes_api_matches_web_for_raw_non_utf8_and_reports_original_offsets() {
    use ikigai_fn::html::PercentDecodeError;
    for byte in 0..=255u8 {
        for input in [vec![byte], vec![byte, b'+', b'%', b'2', b'B']] {
            for plus in [false, true] {
                assert_eq!(
                    percent_decode_bytes(&input, plus).ok(),
                    web::decode(&input, plus).ok()
                );
            }
        }
    }
    assert_eq!(
        percent_decode_bytes(b"%41\xFF%G0", false),
        Err(PercentDecodeError::MalformedEscape { offset: 4 })
    );
}
