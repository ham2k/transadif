use crate::error::{Result, TransadifError};
use crate::adif::{AdifFile, AdifField};
use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use regex::Regex;
use unidecode::unidecode;

#[derive(Debug, Clone, PartialEq)]
pub enum OutputEncoding {
    Utf8,
    Ascii,
    CodePage(String),
}

#[derive(Debug, Clone)]
pub struct EncodingOptions {
    pub output_encoding: OutputEncoding,
    pub transcode: bool,
    pub replace_char: Option<char>,
    pub delete_incompatible: bool,
    pub ascii_transliterate: bool,
    pub strict_mode: bool,
}

impl Default for EncodingOptions {
    fn default() -> Self {
        Self {
            output_encoding: OutputEncoding::Utf8,
            transcode: true,
            replace_char: Some('?'),
            delete_incompatible: false,
            ascii_transliterate: false,
            strict_mode: false,
        }
    }
}

pub struct EncodingProcessor {
    pub options: EncodingOptions,
    warnings: Vec<String>,
}

impl EncodingProcessor {
    pub fn new(options: EncodingOptions) -> Self {
        Self {
            options,
            warnings: Vec::new(),
        }
    }

    pub fn process_file(&mut self, mut file: AdifFile, input_encoding: Option<&str>) -> Result<AdifFile> {
        // Determine input encoding
        let detected_encoding = self.detect_encoding(&file, input_encoding)?;

        // Process header fields
        if let Some(ref mut header) = file.header {
            for field in &mut header.fields {
                self.process_field(field, &detected_encoding)?;
            }
        }

        // Process record fields
        for record in &mut file.records {
            for field in &mut record.fields {
                self.process_field(field, &detected_encoding)?;
            }
        }

        file.detected_encoding = Some(detected_encoding);
        Ok(file)
    }

    pub fn get_warnings(&self) -> &[String] {
        &self.warnings
    }

    fn detect_encoding(&self, file: &AdifFile, suggested_encoding: Option<&str>) -> Result<String> {
        // First check for encoding header field
        if let Some(header_encoding) = file.get_encoding_from_header() {
            return Ok(header_encoding);
        }

        // Use suggested encoding if provided
        if let Some(encoding) = suggested_encoding {
            return Ok(encoding.to_string());
        }

        // Try to detect encoding from data
        let mut utf8_valid_count = 0;
        let mut utf8_invalid_count = 0;
        let mut high_bytes_count = 0;
        let mut total_fields = 0;

        // Check header fields
        if let Some(header) = &file.header {
            for field in &header.fields {
                total_fields += 1;
                let stats = self.analyze_field_bytes(&field.raw_data);
                utf8_valid_count += stats.utf8_valid;
                utf8_invalid_count += stats.utf8_invalid;
                high_bytes_count += stats.high_bytes;
            }
        }

        // Check record fields
        for record in &file.records {
            for field in &record.fields {
                total_fields += 1;
                let stats = self.analyze_field_bytes(&field.raw_data);
                utf8_valid_count += stats.utf8_valid;
                utf8_invalid_count += stats.utf8_invalid;
                high_bytes_count += stats.high_bytes;
            }
        }

        // Improved heuristics for encoding detection
        let detected = if utf8_valid_count > 0 && utf8_invalid_count == 0 {
            // Contains valid UTF-8 sequences and no invalid ones
            "UTF-8".to_string()
        } else if utf8_invalid_count > 0 && utf8_valid_count == 0 {
            // Contains invalid UTF-8 but might be valid ISO-8859-1
            "ISO-8859-1".to_string()
        } else if high_bytes_count > 0 {
            // Has high bytes but mixed validity - default to ISO-8859-1
            "ISO-8859-1".to_string()
        } else {
            // Only ASCII characters
            "ASCII".to_string()
        };

        Ok(detected)
    }

    fn analyze_field_bytes(&self, data: &[u8]) -> FieldByteStats {
        let mut stats = FieldByteStats::default();

        // First, try to decode the entire field as UTF-8
        match std::str::from_utf8(data) {
            Ok(utf8_str) => {
                // Valid UTF-8, but check if it looks like double-encoded data
                if data.iter().any(|&b| b > 127) {
                    // Check if this looks like double-encoded UTF-8
                    if self.looks_like_double_encoded_utf8(utf8_str) {
                        stats.utf8_invalid = 1; // Treat as invalid to trigger correction
                        stats.high_bytes = data.iter().filter(|&&b| b > 127).count();
                    } else {
                        stats.utf8_valid = 1;
                    }
                }
            }
            Err(_) => {
                // Invalid UTF-8, but might contain high bytes that are valid in ISO-8859-1
                stats.utf8_invalid = 1;
                stats.high_bytes = data.iter().filter(|&&b| b > 127).count();
            }
        }

        stats
    }

    fn looks_like_double_encoded_utf8(&self, text: &str) -> bool {
        // Look for patterns that suggest double-encoded UTF-8
        // Be very conservative to avoid false positives
        for ch in text.chars() {
            let code = ch as u32;

            // Check for replacement character
            if code == 0xFFFD {
                return true;
            }

            // Only check for specific corruption patterns we've confirmed
            if code == 0xFE1D {  // This should be 0xFE0F (variation selector)
                return true;
            }

            // Look for other suspicious high Unicode code points in the variation selector range
            if code > 0xFE00 && code < 0xFE20 && code != 0xFE0F {
                return true;
            }
        }
        false
    }

    fn get_utf8_sequence_length(&self, first_byte: u8) -> Option<usize> {
        if first_byte & 0x80 == 0 {
            Some(1) // ASCII
        } else if first_byte & 0xE0 == 0xC0 {
            Some(2) // 110xxxxx
        } else if first_byte & 0xF0 == 0xE0 {
            Some(3) // 1110xxxx
        } else if first_byte & 0xF8 == 0xF0 {
            Some(4) // 11110xxx
        } else {
            None // Invalid UTF-8 start byte
        }
    }

    fn process_field(&mut self, field: &mut AdifField, input_encoding: &str) -> Result<()> {
        // First, apply data corrections
        let corrected_data = self.apply_data_corrections(&field.raw_data, input_encoding)?;

        // Convert to Unicode string
        let unicode_string = self.decode_to_unicode(&corrected_data, input_encoding)?;

        // Apply entity reference replacement
        let processed_string = self.replace_entity_references(&unicode_string)?;

        // Store the processed Unicode string
        field.data = processed_string;

        Ok(())
    }

    fn apply_data_corrections(&mut self, data: &[u8], input_encoding: &str) -> Result<Vec<u8>> {
        if self.options.strict_mode {
            return Ok(data.to_vec());
        }

        let mut corrected = data.to_vec();

        // Check for UTF-8 sequences in non-UTF-8 encodings
        if input_encoding.to_lowercase() != "utf-8" {
            if let Some(utf8_corrected) = self.detect_and_correct_utf8_in_non_utf8(&corrected) {
                if !self.options.strict_mode {
                    self.warnings.push(format!(
                        "Detected UTF-8 sequences in {} encoded data, correcting",
                        input_encoding
                    ));
                    corrected = utf8_corrected;
                }
            }
        } else {
            // Even for UTF-8 encoding, try to fix double-encoded sequences
            if let Some(utf8_corrected) = self.fix_double_encoded_utf8(&corrected) {
                if !self.options.strict_mode {
                    self.warnings.push(
                        "Detected double-encoded UTF-8 data, correcting".to_string()
                    );
                    corrected = utf8_corrected;
                }
            }
        }

        // Check for ISO-8859-1 in UTF-8
        if input_encoding.to_lowercase() == "utf-8" {
            if let Some(iso_corrected) = self.detect_and_correct_iso_in_utf8(&corrected) {
                if !self.options.strict_mode {
                    self.warnings.push(
                        "Detected ISO-8859-1 characters in UTF-8 data, correcting".to_string()
                    );
                    corrected = iso_corrected;
                }
            }
        }

        Ok(corrected)
    }

    fn detect_and_correct_utf8_in_non_utf8(&self, data: &[u8]) -> Option<Vec<u8>> {
        // First try: Direct UTF-8 correction for double-encoded sequences
        if let Some(corrected) = self.fix_double_encoded_utf8(data) {
            return Some(corrected);
        }

        // Second try: Look for consecutive high bytes that form valid UTF-8
        let mut result = Vec::new();
        let mut i = 0;
        let mut found_utf8 = false;

        while i < data.len() {
            if data[i] > 127 {
                // Look for consecutive high bytes that form valid UTF-8
                let mut utf8_end = i;
                while utf8_end < data.len() && data[utf8_end] > 127 {
                    utf8_end += 1;
                }

                if utf8_end > i + 1 {
                    // Multiple consecutive high bytes, check if valid UTF-8
                    let sequence = &data[i..utf8_end];
                    if std::str::from_utf8(sequence).is_ok() {
                        result.extend_from_slice(sequence);
                        found_utf8 = true;
                        i = utf8_end;
                        continue;
                    }
                }
            }
            result.push(data[i]);
            i += 1;
        }

        if found_utf8 {
            Some(result)
        } else {
            None
        }
    }

    fn fix_double_encoded_utf8(&self, data: &[u8]) -> Option<Vec<u8>> {
        // Fix specific corruption patterns we've identified
        let mut corrected = Vec::new();
        let mut i = 0;
        let mut found_correction = false;

        while i < data.len() {
            // Look for the specific corruption pattern: ef b8 9d e2 83 a3
            // This should be: f0 9f 94 9f e2 83 a3 (but the first part is corrupted)
            if i + 5 < data.len() &&
               data[i] >= b'0' && data[i] <= b'9' && // ASCII digit
               data[i + 1] == 0xEF && data[i + 2] == 0xB8 && data[i + 3] == 0x9D &&
               data[i + 4] == 0xE2 && data[i + 5] == 0x83 {
                // This is the corrupted keycap sequence
                corrected.push(data[i]); // Keep the digit
                // Add the correct variation selector-16 (U+FE0F)
                corrected.extend_from_slice(&[0xEF, 0xB8, 0x8F]);
                // Add the combining enclosing keycap (U+20E3)
                corrected.extend_from_slice(&[0xE2, 0x83, 0xA3]);
                i += 6; // Skip the corrupted sequence
                found_correction = true;
            } else {
                corrected.push(data[i]);
                i += 1;
            }
        }

        if found_correction {
            Some(corrected)
        } else {
            // Fallback: Try the original approach
            self.fix_double_encoded_utf8_fallback(data)
        }
    }

    fn fix_double_encoded_utf8_fallback(&self, data: &[u8]) -> Option<Vec<u8>> {
        // Try to fix UTF-8 that was double-encoded (UTF-8 -> ISO-8859-1 -> UTF-8)
        let as_string = String::from_utf8_lossy(data);

        // Convert the string back to bytes as if it were ISO-8859-1
        let mut iso_bytes = Vec::new();
        for ch in as_string.chars() {
            let ch_code = ch as u32;
            if ch_code <= 0xFF {
                iso_bytes.push(ch_code as u8);
            } else {
                // For characters that can't be ISO-8859-1, try to fix specific corruptions
                if ch_code == 0xFE1D {
                    // This should be 0xFE0F (variation selector-16)
                    iso_bytes.extend_from_slice(&[0xFE, 0x0F]);
                } else {
                    // Character can't be represented in ISO-8859-1, keep as UTF-8
                    let mut char_bytes = [0; 4];
                    let len = ch.encode_utf8(&mut char_bytes).len();
                    iso_bytes.extend_from_slice(&char_bytes[..len]);
                }
            }
        }

        // Check if the resulting bytes form valid UTF-8
        if let Ok(utf8_str) = std::str::from_utf8(&iso_bytes) {
            // Check if this looks like it contains the characters we expect
            if utf8_str.chars().any(|c| c as u32 > 127) && utf8_str != as_string {
                return Some(iso_bytes);
            }
        }

        None
    }

    fn detect_and_correct_iso_in_utf8(&self, data: &[u8]) -> Option<Vec<u8>> {
        // Check if this looks like ISO-8859-1 misinterpreted as UTF-8
        if std::str::from_utf8(data).is_err() {
        // Try to decode as ISO-8859-1 and re-encode as UTF-8
        let (decoded, _, had_errors) = WINDOWS_1252.decode(data);
            if !had_errors {
                let utf8_bytes = decoded.as_bytes().to_vec();
                return Some(utf8_bytes);
            }
        }
        None
    }

    fn decode_to_unicode(&self, data: &[u8], encoding_name: &str) -> Result<String> {
        let encoding = self.get_encoding_by_name(encoding_name)?;
        let (decoded, _, had_errors) = encoding.decode(data);

        if had_errors && self.options.strict_mode {
            return Err(TransadifError::Encoding(format!(
                "Invalid characters found in {} encoding",
                encoding_name
            )));
        }

        Ok(decoded.into_owned())
    }

    fn get_encoding_by_name(&self, name: &str) -> Result<&'static Encoding> {
        match name.to_lowercase().as_str() {
            "utf-8" | "utf8" => Ok(UTF_8),
            "iso-8859-1" | "iso8859-1" | "latin1" => Ok(WINDOWS_1252),
            "windows-1252" | "cp1252" => Ok(WINDOWS_1252),
            "ascii" | "us-ascii" => Ok(WINDOWS_1252), // Use Windows-1252 as ASCII superset
            _ => Err(TransadifError::Encoding(format!(
                "Unsupported encoding: {}",
                name
            ))),
        }
    }

    fn replace_entity_references(&self, text: &str) -> Result<String> {
        let entity_regex = Regex::new(r"&(#?)([0-9A-Fa-fx]+);").unwrap();
        let mut result = text.to_string();

        for captures in entity_regex.captures_iter(text) {
            let full_match = captures.get(0).unwrap().as_str();
            let is_numeric = captures.get(1).unwrap().as_str() == "#";
            let value_str = captures.get(2).unwrap().as_str();

            if is_numeric {
                // Numeric entity reference
                let code_point = if value_str.to_lowercase().starts_with("x") {
                    // Hexadecimal
                    u32::from_str_radix(&value_str[1..], 16)
                } else {
                    // Decimal
                    value_str.parse::<u32>()
                };

                if let Ok(code) = code_point {
                    if let Some(ch) = char::from_u32(code) {
                        result = result.replace(full_match, &ch.to_string());
                    }
                }
            } else {
                // Named entity reference - implement basic HTML entities
                let replacement = match value_str.to_lowercase().as_str() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "apos" => "'",
                    _ => continue,
                };
                result = result.replace(full_match, replacement);
            }
        }

        Ok(result)
    }

    pub fn encode_for_output(&self, text: &str) -> Result<(Vec<u8>, usize)> {
        match &self.options.output_encoding {
            OutputEncoding::Utf8 => {
                let bytes = text.as_bytes().to_vec();
                let char_count = text.chars().count();
                Ok((bytes, char_count))
            }
            OutputEncoding::Ascii => {
                let processed = if self.options.ascii_transliterate {
                    unidecode(text)
                } else {
                    text.to_string()
                };

                let (bytes, char_count) = self.encode_with_fallback(&processed, WINDOWS_1252, true)?;
                Ok((bytes, char_count))
            }
            OutputEncoding::CodePage(name) => {
                let encoding = self.get_encoding_by_name(name)?;
                let (bytes, char_count) = self.encode_with_fallback(text, encoding, false)?;
                Ok((bytes, char_count))
            }
        }
    }

    fn encode_with_fallback(
        &self,
        text: &str,
        encoding: &'static Encoding,
        ascii_only: bool,
    ) -> Result<(Vec<u8>, usize)> {
        let mut result = Vec::new();
        let mut char_count = 0;

        for ch in text.chars() {
            char_count += 1;

            // Check if character is compatible
            let ch_str = ch.to_string();
            let (encoded, _, had_errors) = encoding.encode(&ch_str);

            let is_ascii_compatible = !ascii_only || ch.is_ascii();

            if !had_errors && is_ascii_compatible {
                result.extend_from_slice(&encoded);
            } else {
                // Handle incompatible character
                if self.options.delete_incompatible {
                    char_count -= 1; // Don't count deleted characters
                } else if let Some(replacement) = self.options.replace_char {
                    let replacement_str = replacement.to_string();
                    let (repl_encoded, _, _) = encoding.encode(&replacement_str);
                    result.extend_from_slice(&repl_encoded);
                } else {
                    // Use entity reference
                    let entity = format!("&#{};", ch as u32);
                    let (ent_encoded, _, _) = encoding.encode(&entity);
                    result.extend_from_slice(&ent_encoded);
                }
            }
        }

        Ok((result, char_count))
    }
}

#[derive(Default)]
struct FieldByteStats {
    high_bytes: usize,
    utf8_valid: usize,
    utf8_invalid: usize,
}
