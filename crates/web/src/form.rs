//! Percent-encoding, which is how a query string and a form body arrive.
//!
//! Two rules and one difference: `%xx` is a byte, and inside a query or a form
//! body `+` is a space, while inside a path it is a plus sign. Anything that
//! is not valid UTF-8 once decoded is replaced rather than refused, because a
//! browser sending nonsense is not a reason to lose a comment somebody typed.

/// Where an encoded string came from, which is what decides what `+` means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
    /// A path, where `+` is a plus sign.
    Path,
    /// A query or a form body, where `+` is a space.
    Form,
}

/// Split `a=1&b=2` into decoded pairs, keeping the order and any repeats.
///
/// A name with no `=` is a name with an empty value, which is what a checkbox
/// posted bare looks like.
pub(crate) fn pairs(raw: &str) -> Vec<(String, String)> {
    raw.split('&')
        .filter(|part| !part.is_empty())
        .map(|part| match part.split_once('=') {
            Some((name, value)) => (decode(name, Encoding::Form), decode(value, Encoding::Form)),
            None => (decode(part, Encoding::Form), String::new()),
        })
        .collect()
}

/// The first value in those pairs under that name.
pub(crate) fn pick<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value.as_str())
}

/// Decode one encoded piece.
pub(crate) fn decode(raw: &str, encoding: Encoding) -> String {
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut at = 0;

    while at < bytes.len() {
        let byte = bytes[at];

        // A `%` that is not a whole escape is a `%`, which is what a browser
        // sends when somebody types one into a comment.
        if byte == b'%'
            && let Some(decoded) = escaped(bytes, at)
        {
            out.push(decoded);
            at += 3;
            continue;
        }

        out.push(if byte == b'+' && encoding == Encoding::Form {
            b' '
        } else {
            byte
        });
        at += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
}

/// The byte the `%xx` at that position stands for, if it is one.
fn escaped(bytes: &[u8], at: usize) -> Option<u8> {
    let high = hex(*bytes.get(at + 1)?)?;
    let low = hex(*bytes.get(at + 2)?)?;

    Some(high * 16 + low)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- decode ---

    #[test]
    fn decodes_an_escape_into_its_byte() {
        assert_eq!(decode("crates%2Fweb", Encoding::Path), "crates/web");
    }

    #[test]
    fn reads_a_plus_as_a_space_in_a_form_and_as_a_plus_in_a_path() {
        assert_eq!(decode("one+two", Encoding::Form), "one two");
        assert_eq!(decode("one+two", Encoding::Path), "one+two");
    }

    #[test]
    fn puts_utf8_back_together_from_its_bytes() {
        assert_eq!(decode("caf%C3%A9", Encoding::Form), "café");
    }

    #[test]
    fn a_stray_percent_stays_a_percent() {
        assert_eq!(decode("100%", Encoding::Form), "100%");
        assert_eq!(decode("%zz", Encoding::Form), "%zz");
        assert_eq!(decode("%4", Encoding::Form), "%4");
    }

    // --- pairs ---

    #[test]
    fn keeps_the_order_and_the_repeats() {
        assert_eq!(
            pairs("side=new&side=old&start=12&flag&"),
            vec![
                ("side".to_owned(), "new".to_owned()),
                ("side".to_owned(), "old".to_owned()),
                ("start".to_owned(), "12".to_owned()),
                ("flag".to_owned(), String::new()),
            ]
        );
    }

    #[test]
    fn picks_the_first_of_a_repeated_name() {
        assert_eq!(pick(&pairs("side=new&side=old"), "side"), Some("new"));
        assert_eq!(pick(&pairs("side=new"), "file"), None);
    }
}
