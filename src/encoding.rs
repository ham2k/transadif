use crate::errors::TransadifError;
use chardetng::EncodingDetector;
use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};

#[derive(Debug, Clone, PartialEq)]
pub enum OutputEncoding {
    Utf8,
    Iso88591,
    Windows1252,
    Ascii,
}

impl OutputEncoding {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputEncoding::Utf8 => "UTF-8",
            OutputEncoding::Iso88591 => "ISO-8859-1",
            OutputEncoding::Windows1252 => "Windows-1252",
            OutputEncoding::Ascii => "US-ASCII",
        }
    }

    pub fn encoding_rs(&self) -> &'static Encoding {
        match self {
            OutputEncoding::Utf8 => UTF_8,
        OutputEncoding::Iso88591 => WINDOWS_1252, // Using Windows-1252 as superset of ISO-8859-1
        OutputEncoding::Windows1252 => WINDOWS_1252,
            OutputEncoding::Ascii => UTF_8, // We'll handle ASCII specially
        }
    }
}

pub fn detect_encoding(bytes: &[u8], suggested_encoding: Option<&str>) -> Result<&'static Encoding, TransadifError> {
    // If encoding is suggested, try to use it
    if let Some(encoding_name) = suggested_encoding {
        if let Some(encoding) = Encoding::for_label(encoding_name.as_bytes()) {
            return Ok(encoding);
        }
    }

    // Auto-detect encoding
    let mut detector = EncodingDetector::new();
    detector.feed(bytes, true);
    let encoding = detector.guess(None, true);

    Ok(encoding)
}

pub fn convert_to_unicode(bytes: &[u8], encoding: &'static Encoding) -> Result<String, TransadifError> {
    let (decoded, _, had_errors) = encoding.decode(bytes);

    if had_errors {
        // Try common fallbacks
        if encoding != UTF_8 {
            // Try UTF-8 first
            if let Ok(utf8_str) = std::str::from_utf8(bytes) {
                return Ok(utf8_str.to_string());
            }
        }

        if encoding != WINDOWS_1252 {
            // Try Windows-1252 (which is a superset of ISO-8859-1)
            let (decoded, _, had_errors) = WINDOWS_1252.decode(bytes);
            if !had_errors {
                return Ok(decoded.into_owned());
            }
        }

        // If we still have errors, return the best attempt
        return Ok(decoded.into_owned());
    }

    Ok(decoded.into_owned())
}

pub fn encode_string(text: &str, target_encoding: &OutputEncoding, replace_char: Option<char>, delete_incompatible: bool, ascii_transliterate: bool) -> Result<Vec<u8>, TransadifError> {
    let processed_text = if ascii_transliterate && matches!(target_encoding, OutputEncoding::Ascii) {
        unidecode::unidecode(text)
    } else {
        text.to_string()
    };

    match target_encoding {
        OutputEncoding::Utf8 => Ok(processed_text.into_bytes()),
        OutputEncoding::Ascii => {
            let mut result = Vec::new();
            for ch in processed_text.chars() {
                if ch.is_ascii() {
                    result.push(ch as u8);
                } else if delete_incompatible {
                    // Skip the character
                } else if let Some(replacement) = replace_char {
                    if replacement.is_ascii() {
                        result.push(replacement as u8);
                    } else {
                        result.push(b'?');
                    }
                } else {
                    // Use entity reference
                    let entity = format!("&#{};", ch as u32);
                    result.extend_from_slice(entity.as_bytes());
                }
            }
            Ok(result)
        },
        _ => {
            let encoding = target_encoding.encoding_rs();
            let (encoded, _, had_errors) = encoding.encode(&processed_text);

            if had_errors && replace_char.is_some() {
                // Handle replacement manually
                let mut result = Vec::new();
                for ch in processed_text.chars() {
                    let ch_str = ch.to_string();
                    let (encoded_ch, _, had_error) = encoding.encode(&ch_str);

                    if had_error {
                        if delete_incompatible {
                            // Skip the character
                        } else                         if let Some(replacement) = replace_char {
                            let replacement_str = replacement.to_string();
                            let (encoded_replacement, _, _) = encoding.encode(&replacement_str);
                            result.extend_from_slice(&encoded_replacement);
                        } else {
                            // Use entity reference
                            let entity = format!("&#{};", ch as u32);
                            result.extend_from_slice(entity.as_bytes());
                        }
                    } else {
                        result.extend_from_slice(&encoded_ch);
                    }
                }
                Ok(result)
            } else {
                Ok(encoded.into_owned())
            }
        }
    }
}

pub fn count_characters_vs_bytes(text: &str) -> (usize, usize) {
    (text.chars().count(), text.len())
}
