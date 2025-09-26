use crate::errors::TransadifError;
use std::str;

/// Check if text appears to be meaningful (not just control characters or garbage)
fn is_meaningful_text(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return false;
    }

    // Count meaningful characters (letters, digits, common punctuation, CJK characters)
    let meaningful_count = chars.iter().filter(|&&c| {
        c.is_alphabetic() ||
        c.is_numeric() ||
        c.is_whitespace() ||
        matches!(c, '.' | ',' | '!' | '?' | ':' | ';' | '-' | '_' | '(' | ')' | '[' | ']' | '{' | '}' | '\'' | '"' | '@' | '#' | '$' | '%' | '&' | '*' | '+' | '=' | '/' | '\\' | '|' | '~' | '`' | '^') ||
        (c as u32 >= 0x1100 && c as u32 <= 0x11FF) || // Hangul Jamo
        (c as u32 >= 0x3130 && c as u32 <= 0x318F) || // Hangul Compatibility Jamo
        (c as u32 >= 0xAC00 && c as u32 <= 0xD7AF) || // Hangul Syllables
        (c as u32 >= 0x4E00 && c as u32 <= 0x9FFF) || // CJK Unified Ideographs
        (c as u32 >= 0x3040 && c as u32 <= 0x309F) || // Hiragana
        (c as u32 >= 0x30A0 && c as u32 <= 0x30FF) || // Katakana
        (c as u32 >= 0x1F600 && c as u32 <= 0x1F64F)   // Emoticons
    }).count();

    // Text is meaningful if most characters are recognizable
    // Also check that it doesn't contain too many unusual characters
    let unusual_count = chars.iter().filter(|&&c| {
        let code = c as u32;
        // Cyrillic characters that might indicate over-correction
        (code >= 0x0400 && code <= 0x04FF) ||
        // Other suspicious ranges
        (code >= 0x0100 && code <= 0x017F && !matches!(c, 'À'..='ÿ'))
    }).count();

    (meaningful_count as f64 / chars.len() as f64) > 0.8 &&
    (unusual_count as f64 / chars.len() as f64) < 0.1
}

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
    // Simple and reliable approach: try to convert entire string back to bytes and decode as UTF-8
    // This works because ISO-8859-1 mojibake has all characters with codes ≤ 255
    
    // Check if all characters can be converted to bytes (code ≤ 255)
    let can_convert_to_bytes = text.chars().all(|c| (c as u32) <= 255);
    
    if can_convert_to_bytes {
        // Convert to bytes and try to decode as UTF-8
        let bytes: Vec<u8> = text.chars().map(|c| c as u8).collect();
        
        if let Ok(decoded) = std::str::from_utf8(&bytes) {
            // Only use the decoded version if it's different and meaningful
            if decoded != text && is_meaningful_text(&decoded) {
                return decoded.to_string();
            }
        }
    }
    
    // If the whole string approach doesn't work, try word-by-word
    // This handles mixed content where some words are mojibake and others aren't
    let words: Vec<&str> = text.split(' ').collect();
    if words.len() > 1 {
        let mut fixed_words = Vec::new();
        let mut any_changed = false;
        
        for word in words {
            // Skip empty words
            if word.is_empty() {
                fixed_words.push(word.to_string());
                continue;
            }
            
            // Check if this word can be fixed
            let word_can_convert = word.chars().all(|c| (c as u32) <= 255);
            
            if word_can_convert {
                let word_bytes: Vec<u8> = word.chars().map(|c| c as u8).collect();
                if let Ok(decoded_word) = std::str::from_utf8(&word_bytes) {
                    if decoded_word != word && is_meaningful_text(&decoded_word) {
                        fixed_words.push(decoded_word.to_string());
                        any_changed = true;
                        continue;
                    }
                }
            }
            
            // Keep original word if no fix applied
            fixed_words.push(word.to_string());
        }
        
        if any_changed {
            return fixed_words.join(" ");
        }
    }

    // If the general approach doesn't work, fall back to pattern-based detection
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        // Check for various mojibake patterns

        // Pattern 1: Standard UTF-8 mojibake (192-223 followed by 128-191)
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

        // Pattern 1b: Korean/CJK UTF-8 mojibake (including control characters)
        if (ch as u32 >= 128) && (ch as u32 <= 255) {
            // Look for sequences that could be UTF-8 bytes interpreted as ISO-8859-1
            // Korean UTF-8 often creates sequences like: high-char, control-char, high-char
            let mut sequence_chars = vec![ch];
            let mut j = i + 1;

            // Collect characters that could form a UTF-8 sequence
            while j < chars.len() && sequence_chars.len() < 6 {
                let next_char = chars[j];
                let next_code = next_char as u32;

                // Stop at spaces (but not other whitespace like \n, \t)
                if next_char == ' ' {
                    break;
                }

                // Include any character that could be a UTF-8 byte (0x80-0xFF)
                if next_code >= 128 && next_code <= 255 {
                    sequence_chars.push(next_char);
                    j += 1;
                } else if next_code < 32 {
                    // Include control characters that might be UTF-8 continuation bytes
                    sequence_chars.push(next_char);
                    j += 1;
                } else {
                    break;
                }
            }

            // Try to decode as UTF-8 if we have at least 2 characters
            if sequence_chars.len() >= 2 {
                let bytes: Vec<u8> = sequence_chars.iter().map(|&c| c as u8).collect();

                // Try to interpret these bytes as UTF-8
                if let Ok(decoded) = str::from_utf8(&bytes) {
                    let original_string: String = sequence_chars.iter().collect();
                    // Check if this produces meaningful text and is more compact
                    if decoded != original_string &&
                       decoded.chars().count() < original_string.chars().count() &&
                       is_meaningful_text(&decoded) {
                        result.push_str(decoded);
                        i = j;
                        continue;
                    }
                }
            }
        }

        // Pattern 2: Nested mojibake with specific sequences
        // Look for patterns like ÃƒÂ¡ (which represents doubly-encoded á)
        if ch == 'Ã' && i + 3 < chars.len() {
            let seq = [chars[i], chars[i+1], chars[i+2], chars[i+3]];

            // Check for specific nested mojibake patterns
            match seq {
                ['Ã', 'ƒ', 'Â', '¡'] => { // á double-encoded
                    result.push('á');
                    i += 4;
                    continue;
                }
                ['Ã', 'ƒ', 'Â', '±'] => { // ñ double-encoded
                    result.push('ñ');
                    i += 4;
                    continue;
                }
                ['Ã', 'ƒ', 'Â', '©'] => { // é double-encoded
                    result.push('é');
                    i += 4;
                    continue;
                }
                ['Ã', 'ƒ', 'Â', '³'] => { // ó double-encoded
                    result.push('ó');
                    i += 4;
                    continue;
                }
                ['Ã', 'ƒ', 'Â', 'º'] => { // ú double-encoded
                    result.push('ú');
                    i += 4;
                    continue;
                }
                ['Ã', 'ƒ', 'Â', '­'] => { // í double-encoded
                    result.push('í');
                    i += 4;
                    continue;
                }
                _ => {}
            }
        }

        // Pattern 3: Check for Ã followed by ± (which might be ñ single-encoded)
        if ch == 'Ã' && i + 1 < chars.len() && chars[i+1] == '±' {
            result.push('ñ');
            i += 2;
            continue;
        }

        // Pattern 4: Check for Ã followed by ¡ (which might be á single-encoded)
        if ch == 'Ã' && i + 1 < chars.len() && chars[i+1] == '¡' {
            result.push('á');
            i += 2;
            continue;
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
