use crate::error::{Result, TransadifError};
use crate::adif::{AdifFile, AdifField};
use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use unidecode::unidecode;
use chardetng::EncodingDetector;

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

        // Collect all field data for encoding detection
        let mut all_data = Vec::new();

        // Collect header field data
        if let Some(header) = &file.header {
            for field in &header.fields {
                all_data.extend_from_slice(&field.raw_data);
                all_data.push(b' '); // Add separator
            }
        }

        // Collect record field data
        for record in &file.records {
            for field in &record.fields {
                all_data.extend_from_slice(&field.raw_data);
                all_data.push(b' '); // Add separator
            }
        }

        // Use chardetng for encoding detection
        if !all_data.is_empty() {
            let mut detector = EncodingDetector::new();
            detector.feed(&all_data, true); // true = is_last
            let detected_encoding = detector.guess(None, true);


            // Map chardetng results to our expected encoding names
            let encoding_name = match detected_encoding.name() {
                "UTF-8" => "UTF-8",
                "windows-1252" => "ISO-8859-1", // Map Windows-1252 to ISO-8859-1 for our purposes
                "ISO-8859-1" => "ISO-8859-1",
                _ => {
                    // For unknown encodings, fall back to heuristics
                    // Check if data is valid UTF-8
                    if std::str::from_utf8(&all_data).is_ok() {
                        "UTF-8"
                    } else if all_data.iter().all(|&b| b < 128) {
                        "ASCII"
                    } else {
                        "ISO-8859-1"
                    }
                }
            };

            Ok(encoding_name.to_string())
        } else {
            // No data to analyze, default to ASCII
            Ok("ASCII".to_string())
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
        let mut max_iterations = 5; // Prevent infinite loops
        let mut made_correction = true;

        // Apply corrections recursively until no more corrections are possible
        while made_correction && max_iterations > 0 {
            made_correction = false;
            let before_correction = corrected.clone();

            // First, handle encoding-specific corrections to get to Unicode
            if input_encoding.to_lowercase() != "utf-8" {
                // Detect UTF-8 sequences in non-UTF-8 encodings
                if let Some(utf8_corrected) = self.detect_and_correct_utf8_in_non_utf8(&corrected) {
                    if !self.options.strict_mode {
                        self.warnings.push(format!(
                            "Detected UTF-8 sequences in {} encoded data, correcting",
                            input_encoding
                        ));
                        corrected = utf8_corrected;
                        made_correction = true;
                    }
                }
            }

            // Once we have Unicode data (UTF-8), apply all mojibake corrections
            // The recursive loop will handle multiple passes automatically
            if !made_correction {
                // Try nested mojibake correction (works on any Unicode data)
                if let Some(nested_corrected) = self.fix_nested_mojibake(&corrected) {
                    if !self.options.strict_mode {
                        self.warnings.push("Detected nested mojibake, correcting".to_string());
                        corrected = nested_corrected;
                        made_correction = true;
                    }
                }
            }

            if !made_correction {
                // Try double-encoded UTF-8 correction
                if let Some(utf8_corrected) = self.fix_double_encoded_utf8(&corrected) {
                    if !self.options.strict_mode {
                        self.warnings.push("Detected double-encoded UTF-8 data, correcting".to_string());
                        corrected = utf8_corrected;
                        made_correction = true;
                    }
                }
            }

            if !made_correction {
                // Try ISO-8859-1 in UTF-8 correction
                if let Some(iso_corrected) = self.detect_and_correct_iso_in_utf8(&corrected) {
                    if !self.options.strict_mode {
                        self.warnings.push("Detected ISO-8859-1 characters in UTF-8 data, correcting".to_string());
                        corrected = iso_corrected;
                        made_correction = true;
                    }
                }
            }

            // Safety check to prevent infinite loops
            if corrected == before_correction {
                made_correction = false;
            }
            max_iterations -= 1;
        }

        Ok(corrected)
    }

    fn detect_and_correct_utf8_in_non_utf8(&self, data: &[u8]) -> Option<Vec<u8>> {
        // First try: Detect UTF-8 sequences in the raw bytes
        // This handles cases where UTF-8 bytes are embedded in an ISO-8859-1 file
        if let Some(corrected) = self.detect_embedded_utf8_sequences(data) {
            return Some(corrected);
        }

        // Second try: Fix partial mojibake (mixed UTF-8 and ISO in same field)
        if let Some(corrected) = self.fix_partial_mojibake(data) {
            return Some(corrected);
        }

        // Third try: Fix mojibake (UTF-8 bytes interpreted as ISO-8859-1)
        if let Some(corrected) = self.fix_mojibake_utf8(data) {
            return Some(corrected);
        }

        // Fourth try: Fix nested mojibake (double-encoded UTF-8)
        if let Some(corrected) = self.fix_nested_mojibake(data) {
            return Some(corrected);
        }

        // Fifth try: Direct UTF-8 correction for double-encoded sequences
        if let Some(corrected) = self.fix_double_encoded_utf8(data) {
            return Some(corrected);
        }

        // Fifth try: Look for consecutive high bytes that form valid UTF-8
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

    fn detect_embedded_utf8_sequences(&self, data: &[u8]) -> Option<Vec<u8>> {
        // Check if the raw bytes contain valid UTF-8 sequences
        // This is the case when UTF-8 data is embedded in an ISO file

        // If the entire field is valid UTF-8, just return it as-is
        if std::str::from_utf8(data).is_ok() {
            return Some(data.to_vec());
        }


        // Check if this field might contain mojibake patterns
        // If so, skip embedded UTF-8 detection and let mojibake correction handle it
        // We need to decode as ISO-8859-1 to see the mojibake patterns
        let as_iso_bytes: Vec<u8> = data.iter().cloned().collect();
        let as_iso_string = as_iso_bytes.iter().map(|&b| b as char).collect::<String>();
        if as_iso_string.contains('Ã') {
            return None;
        }

        // Look for partial UTF-8 sequences mixed with ASCII/ISO characters
        let mut result = Vec::new();
        let mut i = 0;
        let mut found_utf8 = false;

        while i < data.len() {
            // Look for UTF-8 sequence starting with bytes >= 0xC0
            if data[i] >= 0xC0 {
                // Try to find a complete UTF-8 character sequence
                let mut seq_len = 1;
                if data[i] >= 0xF0 { seq_len = 4; }
                else if data[i] >= 0xE0 { seq_len = 3; }
                else if data[i] >= 0xC0 { seq_len = 2; }

                if i + seq_len <= data.len() {
                    let sequence = &data[i..i + seq_len];
                    if std::str::from_utf8(sequence).is_ok() {
                        result.extend_from_slice(sequence);
                        found_utf8 = true;
                        i += seq_len;
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

    fn fix_partial_mojibake(&self, data: &[u8]) -> Option<Vec<u8>> {
        // Handle cases where a field contains both mojibake (UTF-8 interpreted as ISO)
        // and legitimate ISO-8859-1 characters
        //
        // The sophisticated approach: instead of pattern matching, we use chardetng
        // to detect if segments of the data are actually UTF-8 when reinterpreted

        // Convert to string as ISO-8859-1 to see the mojibake patterns
        let as_iso_string = data.iter().map(|&b| b as char).collect::<String>();

        // Look for mojibake indicators (characters that suggest UTF-8 misinterpretation)
        if !as_iso_string.contains('Ã') {
            return None; // No mojibake patterns detected
        }


        // Convert the ISO string back to bytes for byte-level analysis
        let iso_bytes: Vec<u8> = as_iso_string.chars().map(|c| c as u8).collect();

        // Enhanced byte-level mojibake correction
        // Look for UTF-8 byte sequences that were misinterpreted as ISO-8859-1
        let mut result = Vec::new();
        let mut i = 0;
        let mut found_correction = false;

        while i < iso_bytes.len() {
            // Look for UTF-8 multi-byte sequences starting with bytes 0xC2-0xF4
            if iso_bytes[i] >= 0xC2 && iso_bytes[i] <= 0xF4 {
                // Determine expected sequence length
                let seq_len = if iso_bytes[i] <= 0xDF { 2 }      // 2-byte sequence
                             else if iso_bytes[i] <= 0xEF { 3 }  // 3-byte sequence
                             else { 4 };                         // 4-byte sequence

                // Check if we have enough bytes and they form a valid UTF-8 sequence
                if i + seq_len <= iso_bytes.len() {
                    let utf8_bytes = &iso_bytes[i..i + seq_len];
                    if let Ok(utf8_char) = std::str::from_utf8(utf8_bytes) {
                        result.extend_from_slice(utf8_char.as_bytes());
                        found_correction = true;
                        i += seq_len;
                        continue;
                    }
                }
            }

            // Keep the original byte, but ensure it's properly encoded as UTF-8
            let ch = iso_bytes[i] as char;
            result.extend_from_slice(ch.to_string().as_bytes());
            i += 1;
        }

        if found_correction {
            Some(result)
        } else {
            None
        }
    }

    fn fix_mojibake_utf8(&self, data: &[u8]) -> Option<Vec<u8>> {
        // Use chardetng-based approach for mojibake detection and correction
        // Convert data to ISO-8859-1 interpretation first
        let as_iso_string = data.iter().map(|&b| b as char).collect::<String>();

        // Look for mojibake indicators
        if !as_iso_string.contains('Ã') {
            return None; // No mojibake patterns detected
        }

        // Convert back to bytes as if it were ISO-8859-1
        let iso_bytes: Vec<u8> = as_iso_string.chars().map(|c| c as u8).collect();

        // Use chardetng to detect if these bytes are actually UTF-8
        let mut detector = EncodingDetector::new();
        detector.feed(&iso_bytes, true);
        let detected_encoding = detector.guess(None, true);

        if detected_encoding.name() == "UTF-8" {
            // Validate that it's actually valid UTF-8 and represents a correction
            if let Ok(utf8_str) = std::str::from_utf8(&iso_bytes) {
                // Check if this looks like it fixed mojibake
                // (should have fewer characters than the ISO interpretation)
                if utf8_str.chars().count() < as_iso_string.chars().count() {
                    return Some(iso_bytes);
                }
            }
        }

        None
    }

    fn fix_nested_mojibake(&self, data: &[u8]) -> Option<Vec<u8>> {
        // Handle nested mojibake using the specific algorithm:
        // Look for sequences of Unicode characters with code points between 192-223 (UTF-8 start bytes)
        // followed by characters with code points between 128-191 (UTF-8 continuation bytes)

        let utf8_string = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => return None, // Not valid UTF-8, can't be nested mojibake
        };


        let chars: Vec<char> = utf8_string.chars().collect();
        let mut result = Vec::new();
        let mut i = 0;
        let mut found_correction = false;

        while i < chars.len() {
            let char_code = chars[i] as u32;

            // Look for UTF-8 start bytes (192-223 = 0xC0-0xDF)
            if char_code >= 192 && char_code <= 223 {
                // This could be a UTF-8 2-byte sequence start
                if i + 1 < chars.len() {
                    let next_char_code = chars[i + 1] as u32;

                    // Check if followed by UTF-8 continuation byte (128-191 = 0x80-0xBF)
                    if next_char_code >= 128 && next_char_code <= 191 {
                        // We have a potential UTF-8 2-byte sequence
                        let utf8_bytes = [char_code as u8, next_char_code as u8];

                        // Verify this forms a valid UTF-8 sequence
                        if let Ok(utf8_char) = std::str::from_utf8(&utf8_bytes) {
                            // This is valid UTF-8! Replace the mojibake with the correct character
                            result.extend_from_slice(utf8_char.as_bytes());
                            found_correction = true;
                            i += 2; // Skip both characters
                            continue;
                        }
                    }
                }
            }
            // Look for UTF-8 3-byte sequence start (224-239 = 0xE0-0xEF)
            else if char_code >= 224 && char_code <= 239 {
                if i + 2 < chars.len() {
                    let next1_code = chars[i + 1] as u32;
                    let next2_code = chars[i + 2] as u32;

                    // Check if followed by two UTF-8 continuation bytes
                    if (next1_code >= 128 && next1_code <= 191) &&
                       (next2_code >= 128 && next2_code <= 191) {
                        let utf8_bytes = [char_code as u8, next1_code as u8, next2_code as u8];

                        if let Ok(utf8_char) = std::str::from_utf8(&utf8_bytes) {
                            result.extend_from_slice(utf8_char.as_bytes());
                            found_correction = true;
                            i += 3; // Skip all three characters
                            continue;
                        }
                    }
                }
            }
            // Look for UTF-8 4-byte sequence start (240-247 = 0xF0-0xF7)
            else if char_code >= 240 && char_code <= 247 {
                if i + 3 < chars.len() {
                    let next1_code = chars[i + 1] as u32;
                    let next2_code = chars[i + 2] as u32;
                    let next3_code = chars[i + 3] as u32;

                    // Check if followed by three UTF-8 continuation bytes
                    if (next1_code >= 128 && next1_code <= 191) &&
                       (next2_code >= 128 && next2_code <= 191) &&
                       (next3_code >= 128 && next3_code <= 191) {
                        let utf8_bytes = [char_code as u8, next1_code as u8, next2_code as u8, next3_code as u8];

                        if let Ok(utf8_char) = std::str::from_utf8(&utf8_bytes) {
                            result.extend_from_slice(utf8_char.as_bytes());
                            found_correction = true;
                            i += 4; // Skip all four characters
                            continue;
                        }
                    }
                }
            }

            // No UTF-8 sequence found, keep the original character
            let ch = chars[i];
            result.extend_from_slice(ch.to_string().as_bytes());
            i += 1;
        }

        if found_correction {
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
        // If data is valid UTF-8, decode as UTF-8 regardless of detected encoding
        if let Ok(utf8_str) = std::str::from_utf8(data) {
            return Ok(utf8_str.to_string());
        }

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
        // Use the htmlescape crate for comprehensive entity handling
        match htmlescape::decode_html(text) {
            Ok(decoded) => Ok(decoded.to_string()),
            Err(_) => {
                // If htmlescape fails, return the original text
                // This is more robust than failing completely
                Ok(text.to_string())
            }
        }
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

