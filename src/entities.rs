use regex::Regex;

pub fn decode_entities(text: &str) -> String {
    // Create regex patterns for different entity types
    let _named_entity_re = Regex::new(r"&([a-zA-Z][a-zA-Z0-9]*);").unwrap();
    let numeric_entity_re = Regex::new(r"&#(\d+);").unwrap();
    let hex_entity_re = Regex::new(r"&#x([0-9a-fA-F]+);").unwrap();
    let custom_hex_re = Regex::new(r"&0x([0-9a-fA-F]+);").unwrap();

    let mut result = text.to_string();

    // Decode named entities using htmlescape
    result = htmlescape::decode_html(&result).unwrap_or(result);

    // Decode numeric entities
    result = numeric_entity_re.replace_all(&result, |caps: &regex::Captures| {
        if let Ok(num) = caps[1].parse::<u32>() {
            if let Some(ch) = char::from_u32(num) {
                return ch.to_string();
            }
        }
        caps[0].to_string() // Return original if conversion fails
    }).to_string();

    // Decode hex entities (&#xNN;)
    result = hex_entity_re.replace_all(&result, |caps: &regex::Captures| {
        if let Ok(num) = u32::from_str_radix(&caps[1], 16) {
            if let Some(ch) = char::from_u32(num) {
                return ch.to_string();
            }
        }
        caps[0].to_string() // Return original if conversion fails
    }).to_string();

    // Decode custom hex entities (&0xNN;)
    result = custom_hex_re.replace_all(&result, |caps: &regex::Captures| {
        if let Ok(num) = u32::from_str_radix(&caps[1], 16) {
            if let Some(ch) = char::from_u32(num) {
                return ch.to_string();
            }
        }
        caps[0].to_string() // Return original if conversion fails
    }).to_string();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_named_entities() {
        assert_eq!(decode_entities("&amp;"), "&");
        assert_eq!(decode_entities("&lt;"), "<");
        assert_eq!(decode_entities("&gt;"), ">");
        assert_eq!(decode_entities("&quot;"), "\"");
    }

    #[test]
    fn test_numeric_entities() {
        assert_eq!(decode_entities("&#65;"), "A");
        assert_eq!(decode_entities("&#241;"), "ñ");
    }

    #[test]
    fn test_hex_entities() {
        assert_eq!(decode_entities("&#x41;"), "A");
        assert_eq!(decode_entities("&#xF1;"), "ñ");
    }

    #[test]
    fn test_custom_hex_entities() {
        assert_eq!(decode_entities("&0x41;"), "A");
        assert_eq!(decode_entities("&0xF1;"), "ñ");
    }

    #[test]
    fn test_mixed_entities() {
        let input = "Test &amp; &#65; &#x42; &0x43; normal text";
        let expected = "Test & A B C normal text";
        assert_eq!(decode_entities(input), expected);
    }
}
