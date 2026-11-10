//! O que vale para todo texto vindo de fora (a letra e a normalização).

/// Os formatadores bidi (Cf): as marcas (`LRM`, `RLM`, `ALM`), os
/// encaixes (`LRE`…`PDF`) e os isolamentos (`LRI`…`PDI`). Os outros Cf, como
/// o ZWJ dos emojis, ficam.
pub(crate) fn is_bidi_format(c: char) -> bool {
    matches!(
        c,
        '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}
