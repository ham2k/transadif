use crate::error::{Result, TransAdifError};
use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use regex::Regex;
use unicode_normalization::UnicodeNormalization;
use unidecode::unidecode;

#[derive(Debug, Clone)]
pub struct EncodingOptions {
    pub input_encoding: Option<String>,
    pub output_encoding: String,
    pub transcode: bool,
    pub replace_char: String,
    pub delete_incompatible: bool,
    pub ascii_transliterate: bool,
    pub strict_mode: bool,
}

pub struct EncodingDetector {
    entity_regex: Regex,
}

impl EncodingDetector {
    pub fn new() -> Self {
        Self {
            entity_regex: Regex::new(r"&0x([0-9A-Fa-f]{2});").unwrap(),
        }
    }

    /// Detect the encoding of input data
    pub fn detect_encoding(&self, data: &[u8], suggested: Option<&str>) -> Result<&'static Encoding> {
        // If encoding is suggested, try to use it
        if let Some(encoding_name) = suggested {
            if let Some(encoding) = self.get_encoding_by_name(encoding_name) {
                return Ok(encoding);
            }
        }

        // Check if it's valid UTF-8
        if std::str::from_utf8(data).is_ok() {
            // Check if it contains non-ASCII characters
            if data.iter().any(|&b| b > 127) {
                return Ok(UTF_8);
            } else {
                // Pure ASCII
                return Ok(UTF_8); // Use UTF-8 for ASCII compatibility
            }
        }

        // Check for UTF-8 sequences that might be mojibake
        if self.has_utf8_sequences(data) {
            return Ok(UTF_8);
        }

        // Default to Windows-1252 for 8-bit data
        Ok(WINDOWS_1252)
    }

    /// Check if data contains UTF-8 byte sequences
    fn has_utf8_sequences(&self, data: &[u8]) -> bool {
        let mut i = 0;
        while i < data.len() {
            let byte = data[i];
            if byte > 127 {
                // Check for UTF-8 multi-byte sequence
                let sequence_len = if byte & 0b11100000 == 0b11000000 {
                    2
                } else if byte & 0b11110000 == 0b11100000 {
                    3
                } else if byte & 0b11111000 == 0b11110000 {
                    4
                } else {
                    i += 1;
                    continue;
                };

                // Check if we have enough bytes and they're valid continuation bytes
                if i + sequence_len <= data.len() {
                    let mut valid = true;
                    for j in 1..sequence_len {
                        if data[i + j] & 0b11000000 != 0b10000000 {
                            valid = false;
                            break;
                        }
                    }
                    if valid {
                        return true;
                    }
                }
            }
            i += 1;
        }
        false
    }

    /// Convert data to Unicode string, applying corrections
    pub fn decode_to_unicode(&self, data: &[u8], encoding: &'static Encoding, strict: bool) -> Result<String> {
        let (decoded, _, had_errors) = encoding.decode(data);
        
        if had_errors && strict {
            return Err(TransAdifError::StrictMode(
                "Invalid characters found in strict mode".to_string()
            ));
        }

        let mut result = decoded.into_owned();

        // Process entity references
        result = self.process_entity_references(&result)?;

        // Check for mojibake and correct if not in strict mode
        if !strict {
            result = self.correct_mojibake(&result, encoding)?;
        }

        Ok(result)
    }

    /// Process entity references like &0x41;
    fn process_entity_references(&self, text: &str) -> Result<String> {
        let result = self.entity_regex.replace_all(text, |caps: &regex::Captures| {
            if let Ok(byte_val) = u8::from_str_radix(&caps[1], 16) {
                if byte_val < 128 {
                    // ASCII character
                    char::from(byte_val).to_string()
                } else {
                    // Extended character - decode as Windows-1252
                    let bytes = [byte_val];
                    let (decoded, _, _) = WINDOWS_1252.decode(&bytes);
                    decoded.into_owned()
                }
            } else {
                caps[0].to_string() // Keep original if can't parse
            }
        });
        Ok(result.into_owned())
    }

    /// Detect and correct mojibake
    fn correct_mojibake(&self, text: &str, original_encoding: &'static Encoding) -> Result<String> {
        // If original encoding is not UTF-8, look for UTF-8 sequences
        if original_encoding != UTF_8 {
            // Look for sequences that might be UTF-8 encoded as the original encoding
            let mut result = text.to_string();
            let bytes = text.as_bytes();
            
            // Look for potential UTF-8 sequences
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] > 127 {
                    // Try to find a UTF-8 sequence
                    if let Some((utf8_str, len)) = self.extract_utf8_sequence(&bytes[i..]) {
                        // Replace the mojibake with the correct UTF-8 character
                        let start_char_idx = text.char_indices().nth(i).map(|(idx, _)| idx).unwrap_or(i);
                        let end_char_idx = text.char_indices().nth(i + len).map(|(idx, _)| idx).unwrap_or(text.len());
                        
                        result.replace_range(start_char_idx..end_char_idx, &utf8_str);
                        i += len;
                        continue;
                    }
                }
                i += 1;
            }
            return Ok(result);
        }

        // If original encoding is UTF-8, look for invalid sequences that might be ISO-8859-1
        if original_encoding == UTF_8 {
            // Check for invalid UTF-8 that might be ISO-8859-1
            let bytes = text.as_bytes();
            if let Ok(_) = std::str::from_utf8(bytes) {
                // Valid UTF-8, no correction needed
                return Ok(text.to_string());
            }

            // Try interpreting as ISO-8859-1
            let (decoded, _, _) = WINDOWS_1252.decode(bytes);
            return Ok(decoded.into_owned());
        }

        Ok(text.to_string())
    }

    /// Try to extract a UTF-8 sequence from bytes
    fn extract_utf8_sequence(&self, bytes: &[u8]) -> Option<(String, usize)> {
        if bytes.is_empty() || bytes[0] < 128 {
            return None;
        }

        let sequence_len = if bytes[0] & 0b11100000 == 0b11000000 {
            2
        } else if bytes[0] & 0b11110000 == 0b11100000 {
            3
        } else if bytes[0] & 0b11111000 == 0b11110000 {
            4
        } else {
            return None;
        };

        if bytes.len() < sequence_len {
            return None;
        }

        // Check if all continuation bytes are valid
        for i in 1..sequence_len {
            if bytes[i] & 0b11000000 != 0b10000000 {
                return None;
            }
        }

        // Try to decode as UTF-8
        if let Ok(utf8_str) = std::str::from_utf8(&bytes[..sequence_len]) {
            Some((utf8_str.to_string(), sequence_len))
        } else {
            None
        }
    }

    /// Encode Unicode string to target encoding
    pub fn encode_from_unicode(&self, text: &str, target_encoding: &str, opts: &EncodingOptions) -> Result<Vec<u8>> {
        let encoding = self.get_encoding_by_name(target_encoding)
            .ok_or_else(|| TransAdifError::InvalidEncoding(target_encoding.to_string()))?;

        let mut processed_text = text.to_string();

        // Apply ASCII transliteration if requested
        if opts.ascii_transliterate && target_encoding.to_lowercase() == "ascii" {
            processed_text = unidecode(&processed_text);
        }

        // Normalize Unicode
        processed_text = processed_text.nfc().collect::<String>();

        // Encode to target
        let (encoded, _, had_errors) = encoding.encode(&processed_text);

        if had_errors {
            if opts.strict_mode {
                return Err(TransAdifError::StrictMode(
                    "Characters incompatible with target encoding".to_string()
                ));
            }

            // Handle incompatible characters
            processed_text = self.handle_incompatible_chars(&processed_text, encoding, opts)?;
            let (final_encoded, _, _) = encoding.encode(&processed_text);
            Ok(final_encoded.into_owned())
        } else {
            Ok(encoded.into_owned())
        }
    }

    /// Handle characters incompatible with target encoding
    fn handle_incompatible_chars(&self, text: &str, encoding: &'static Encoding, opts: &EncodingOptions) -> Result<String> {
        let mut result = String::new();
        
        for ch in text.chars() {
            let ch_str = ch.to_string();
            let (_, _, had_errors) = encoding.encode(&ch_str);
            
            if had_errors {
                if opts.delete_incompatible {
                    // Skip the character
                    continue;
                } else {
                    // Replace with specified character or entity reference
                    if opts.replace_char.is_empty() {
                        // Use entity reference
                        let code_point = ch as u32;
                        if code_point <= 0xFF {
                            result.push_str(&format!("&0x{:02X};", code_point));
                        } else {
                            result.push_str(&format!("&0x{:04X};", code_point));
                        }
                    } else {
                        result.push_str(&opts.replace_char);
                    }
                }
            } else {
                result.push(ch);
            }
        }
        
        Ok(result)
    }

    /// Get encoding by name
    fn get_encoding_by_name(&self, name: &str) -> Option<&'static Encoding> {
        match name.to_lowercase().as_str() {
            "ascii" | "us-ascii" => Some(UTF_8), // Use UTF-8 for ASCII compatibility
            "utf-8" | "utf8" => Some(UTF_8),
            "iso-8859-1" | "iso8859-1" | "latin1" => Some(WINDOWS_1252),
            "windows-1252" | "cp1252" => Some(WINDOWS_1252),
            _ => None,
        }
    }
}
