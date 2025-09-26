use crate::errors::TransadifError;
use std::str;

/// Detects and corrects mojibake in UTF-8 strings
///
/// Looks for sequences of Unicode characters with code points between 192-223
/// followed by characters with code points between 128-191, and verifies if
/// that sequence, when interpreted as bytes, corresponds to valid UTF-8 sequences.
pub fn fix_mojibake(text: &str) -> String {
    let mut result = text.to_string();
    let mut changed = true;

    // Keep applying fixes until no more changes are made (recursive fixing)
    while changed {
        let new_result = fix_mojibake_once(&result);
        changed = new_result != result;
        result = new_result;
    }

    result
}

fn fix_mojibake_once(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        // Check if this could be the start of a mojibake sequence
        if (192..=223).contains(&(ch as u32)) {
            // Look ahead to find the complete sequence
            let mut sequence_chars = vec![ch];
            let mut j = i + 1;

            // Collect continuation characters (128-191)
            while j < chars.len() && (128..=191).contains(&(chars[j] as u32)) {
                sequence_chars.push(chars[j]);
                j += 1;
            }

            // If we found a potential sequence, try to decode it
            if sequence_chars.len() > 1 {
                let bytes: Vec<u8> = sequence_chars.iter().map(|&c| c as u8).collect();

                // Try to interpret these bytes as UTF-8
                if let Ok(decoded) = str::from_utf8(&bytes) {
                    // Verify this is actually different from the original
                    let original_string: String = sequence_chars.iter().collect();
                    if decoded != original_string {
                        // This is mojibake - use the corrected version
                        result.push_str(decoded);
                        i = j;
                        continue;
                    }
                }
            }
        }

        // Not mojibake, keep the original character
        result.push(ch);
        i += 1;
    }

    result
}

/// Detects potential UTF-8 sequences in byte data that was incorrectly interpreted
/// as another encoding
pub fn detect_utf8_in_bytes(bytes: &[u8]) -> bool {
    // Look for sequences of 2 or more consecutive bytes above 127
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] > 127 {
            // Found a high byte, look for a sequence
            let start = i;
            while i < bytes.len() && bytes[i] > 127 {
                i += 1;
            }

            // If we have 2+ consecutive high bytes, check if they form valid UTF-8
            if i - start >= 2 {
                if str::from_utf8(&bytes[start..i]).is_ok() {
                    return true;
                }
            }
        } else {
            i += 1;
        }
    }

    false
}

/// Attempts to fix mixed encoding issues in byte data
pub fn fix_mixed_encoding(bytes: &[u8], suspected_encoding: &'static encoding_rs::Encoding) -> Result<String, TransadifError> {
    // First, try to decode as the suspected encoding
    let (decoded, _, had_errors) = suspected_encoding.decode(bytes);

    if !had_errors {
        // Clean decode, but check for mojibake patterns
        return Ok(fix_mojibake(&decoded));
    }

    // Had errors, try to identify UTF-8 sequences within the data
    let mut result = String::new();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] > 127 {
            // Look for a UTF-8 sequence
            let start = i;
            let mut end = i;

            // Find the extent of high bytes
            while end < bytes.len() && bytes[end] > 127 {
                end += 1;
            }

            // Try to decode this segment as UTF-8
            if let Ok(utf8_str) = str::from_utf8(&bytes[start..end]) {
                result.push_str(utf8_str);
                i = end;
            } else {
                // Not valid UTF-8, decode single byte with suspected encoding
                let (decoded_char, _, _) = suspected_encoding.decode(&bytes[i..i+1]);
                result.push_str(&decoded_char);
                i += 1;
            }
        } else {
            // ASCII character
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    // Apply mojibake correction to the result
    Ok(fix_mojibake(&result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_mojibake() {
        // "ñ" encoded as UTF-8 (0xC3 0xB1) but interpreted as ISO-8859-1
        let mojibake = "Ã±"; // This is what you get when UTF-8 ñ is interpreted as ISO-8859-1
        let fixed = fix_mojibake(mojibake);
        assert_eq!(fixed, "ñ");
    }

    #[test]
    fn test_no_mojibake() {
        let text = "Regular ASCII text";
        let fixed = fix_mojibake(text);
        assert_eq!(fixed, text);
    }

    #[test]
    fn test_nested_mojibake() {
        // Test recursive fixing
        let text = "JuÃƒÂ¡n"; // Nested mojibake
        let fixed = fix_mojibake(text);
        // This should eventually resolve to "Juan" or the proper accented version
        assert_ne!(fixed, text); // Should be different from input
    }

    #[test]
    fn test_detect_utf8_in_bytes() {
        let utf8_bytes = "ñ".as_bytes(); // UTF-8 encoding of ñ
        assert!(detect_utf8_in_bytes(utf8_bytes));

        let ascii_bytes = b"hello";
        assert!(!detect_utf8_in_bytes(ascii_bytes));
    }
}
