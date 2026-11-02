use ikigai_fn::html::form_pairs;

#[test]
fn strict_form_parser_preserves_pairs_and_refuses_bad_input() {
    assert_eq!(
        form_pairs("tag=c%2B%2B&tag=rust+lang&flag").unwrap(),
        [
            ("tag".into(), "c++".into()),
            ("tag".into(), "rust lang".into()),
            ("flag".into(), "".into())
        ]
    );
    assert!(form_pairs("ok=1&bad=%FF").is_err());
    assert!(form_pairs("ok=1&bad=%G0").is_err());
}

#[test]
fn invalid_utf8_offsets_track_original_bytes_in_names_and_values() {
    use ikigai_fn::html::PercentDecodeError;
    // Prefixes mix one-byte output from escapes with literal multibyte UTF-8.
    for prefix in ["", "%41", "a+", "日", "%E6%97%A5", "é%2B日+"] {
        for byte in 128..=255u8 {
            for (input, offset) in [
                (format!("ok=1&&{prefix}%{byte:02X}=v"), 6 + prefix.len()),
                (format!("ok=1&&k={prefix}%{byte:02X}"), 8 + prefix.len()),
            ] {
                let error = form_pairs(&input).unwrap_err();
                assert_eq!(error.offset, offset, "{input}");
                assert!(matches!(error.source, PercentDecodeError::InvalidUtf8(_)));
            }
        }
    }
}
