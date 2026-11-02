// Copied from ikigai-rs/ikigai-cli at 443901ef6118c134d8283fd2063e1801bfd1df8a
// crates/ikigai-web/src/lib.rs; kept verbatim for compatibility testing.
fn percent_decode(raw: &[u8], plus_is_space: bool) -> Result<Vec<u8>, &'static str> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        match raw[i] {
            b'%' => {
                let hi = raw.get(i + 1).copied().and_then(hex_digit);
                let lo = raw.get(i + 2).copied().and_then(hex_digit);
                match (hi, lo) {
                    (Some(hi), Some(lo)) => out.push(hi << 4 | lo),
                    _ => return Err("malformed percent-escape"),
                }
                i += 3;
            }
            b'+' if plus_is_space => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    Ok(out)
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
