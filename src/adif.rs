use crate::encoding::{detect_encoding, convert_to_unicode, encode_string, OutputEncoding};
use crate::entities::decode_entities;
use crate::mojibake::fix_mojibake;
use crate::errors::TransadifError;
use crate::Config;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct AdifFile {
    pub header: AdifHeader,
    pub records: Vec<AdifRecord>,
}

#[derive(Debug, Clone)]
pub struct AdifHeader {
    pub preamble: String,
    pub fields: Vec<AdifField>,
    pub excess_data: String,
    pub encoding: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AdifRecord {
    pub fields: Vec<AdifField>,
    pub excess_data: String,
}

#[derive(Debug, Clone)]
pub struct AdifField {
    pub name: String,
    pub length: usize,
    pub data_type: Option<String>,
    pub data: String,
    pub excess_data: String,
    pub raw_data: Vec<u8>, // Keep original bytes for length analysis
}

impl AdifFile {
    pub fn process_encodings(&mut self, config: &Config) -> Result<(), TransadifError> {
        // Process header fields
        for field in &mut self.header.fields {
            field.process_encoding(config)?;
        }

        // Process record fields
        for record in &mut self.records {
            for field in &mut record.fields {
                field.process_encoding(config)?;
            }
        }

        Ok(())
    }

    pub fn generate_output(&self, config: &Config) -> Result<Vec<u8>, TransadifError> {
        let mut output = Vec::new();

        // Write header preamble
        output.extend_from_slice(self.header.preamble.as_bytes());

        // Write header fields
        for field in &self.header.fields {
            let field_bytes = field.generate_output(&config.output_encoding, config)?;
            output.extend_from_slice(&field_bytes);
        }

        // Add encoding field if not present
        if !self.header.fields.iter().any(|f| f.name.to_lowercase() == "encoding") {
            let encoding_field = format!("<encoding:{}>{}\n",
                config.output_encoding.as_str().len(),
                config.output_encoding.as_str()
            );
            output.extend_from_slice(encoding_field.as_bytes());
        }

        // Write header end
        output.extend_from_slice(b"<eoh>");

        // Write header excess data
        output.extend_from_slice(self.header.excess_data.as_bytes());

        // Write records
        for record in &self.records {
            for field in &record.fields {
                let field_bytes = field.generate_output(&config.output_encoding, config)?;
                output.extend_from_slice(&field_bytes);
            }
            output.extend_from_slice(b"<eor>");
            output.extend_from_slice(record.excess_data.as_bytes());
        }

        Ok(output)
    }
}

impl AdifField {
    fn process_encoding(&mut self, config: &Config) -> Result<(), TransadifError> {
        // First, decode HTML entities
        let mut processed_data = decode_entities(&self.data);

        // Apply mojibake correction if not in strict mode
        if !config.strict_mode {
            processed_data = fix_mojibake(&processed_data);
        }

        self.data = processed_data;
        Ok(())
    }

    fn generate_output(&self, output_encoding: &OutputEncoding, config: &Config) -> Result<Vec<u8>, TransadifError> {
        let mut output = Vec::new();

        // Encode the data
        let encoded_data = encode_string(
            &self.data,
            output_encoding,
            config.replace_char,
            config.delete_incompatible,
            config.ascii_transliterate
        )?;

        // Calculate length based on output encoding
        // UTF-8 uses character count, others use byte count
        let length = match output_encoding {
            OutputEncoding::Utf8 => self.data.chars().count(),
            _ => encoded_data.len(),
        };

        // Generate field
        output.push(b'<');
        output.extend_from_slice(self.name.as_bytes());
        output.push(b':');
        output.extend_from_slice(length.to_string().as_bytes());

        if let Some(ref data_type) = self.data_type {
            output.push(b':');
            output.extend_from_slice(data_type.as_bytes());
        }

        output.push(b'>');
        output.extend_from_slice(&encoded_data);
        output.extend_from_slice(self.excess_data.as_bytes());

        Ok(output)
    }
}

pub fn parse_adif(bytes: &[u8], config: &Config) -> Result<AdifFile, TransadifError> {
    let mut parser = AdifParser::new(bytes, config)?;
    parser.parse()
}

struct AdifParser<'a> {
    bytes: &'a [u8],
    pos: usize,
    config: &'a Config,
    detected_encoding: &'static encoding_rs::Encoding,
}

impl<'a> AdifParser<'a> {
    fn new(bytes: &'a [u8], config: &'a Config) -> Result<Self, TransadifError> {
        let detected_encoding = detect_encoding(bytes, config.input_encoding.as_deref())?;

        Ok(Self {
            bytes,
            pos: 0,
            config,
            detected_encoding,
        })
    }

    fn parse(&mut self) -> Result<AdifFile, TransadifError> {
        let header = self.parse_header()?;
        let mut records = Vec::new();

        while self.pos < self.bytes.len() {
            if let Some(record) = self.parse_record()? {
                records.push(record);
            } else {
                break;
            }
        }

        Ok(AdifFile { header, records })
    }

    fn parse_header(&mut self) -> Result<AdifHeader, TransadifError> {
        let mut preamble = String::new();
        let mut fields = Vec::new();
        let mut excess_data = String::new();
        let mut encoding = None;

        // Check if file starts with '<' (no header case)
        if self.pos < self.bytes.len() && self.bytes[self.pos] == b'<' {
            return Ok(AdifHeader {
                preamble,
                fields,
                excess_data,
                encoding,
            });
        }

        // Parse preamble until we find a field or <eoh>
        let start_pos = self.pos;
        while self.pos < self.bytes.len() {
            if self.bytes[self.pos] == b'<' {
                // Check if this is <eoh> or a field
                if self.is_at_sequence(b"<eoh>") {
                    // End of header
                    let preamble_bytes = &self.bytes[start_pos..self.pos];
                    preamble = convert_to_unicode(preamble_bytes, self.detected_encoding)?;
                    self.pos += 5; // Skip <eoh>
                    break;
                } else if self.is_field_start() {
                    // Found a field, parse preamble up to here
                    let preamble_bytes = &self.bytes[start_pos..self.pos];
                    preamble = convert_to_unicode(preamble_bytes, self.detected_encoding)?;

                    // Parse header fields
                    while self.pos < self.bytes.len() {
                        if self.is_at_sequence(b"<eoh>") {
                            self.pos += 5;
                            break;
                        } else if let Some(field) = self.parse_field()? {
                            if field.name.to_lowercase() == "encoding" {
                                encoding = Some(field.data.clone());
                            }
                            fields.push(field);
                        } else {
                            return Err(TransadifError::ParseError("Expected field or <eoh> in header".to_string()));
                        }
                    }
                    break;
                }
            }
            self.pos += 1;
        }

        // Parse excess data after <eoh>
        let excess_start = self.pos;
        while self.pos < self.bytes.len() && !self.is_field_start() {
            self.pos += 1;
        }
        if self.pos > excess_start {
            let excess_bytes = &self.bytes[excess_start..self.pos];
            excess_data = convert_to_unicode(excess_bytes, self.detected_encoding)?;
        }

        Ok(AdifHeader {
            preamble,
            fields,
            excess_data,
            encoding,
        })
    }

    fn parse_record(&mut self) -> Result<Option<AdifRecord>, TransadifError> {
        let mut fields = Vec::new();

        // Parse fields until <eor>
        while self.pos < self.bytes.len() {
            if self.is_at_sequence(b"<eor>") {
                self.pos += 5; // Skip <eor>

                // Parse excess data after <eor>
                let excess_start = self.pos;
                while self.pos < self.bytes.len() && !self.is_field_start() {
                    self.pos += 1;
                }

                let excess_data = if self.pos > excess_start {
                    let excess_bytes = &self.bytes[excess_start..self.pos];
                    convert_to_unicode(excess_bytes, self.detected_encoding)?
                } else {
                    String::new()
                };

                return Ok(Some(AdifRecord { fields, excess_data }));
            } else if let Some(field) = self.parse_field()? {
                fields.push(field);
            } else {
                // No more fields and no <eor> found
                break;
            }
        }

        if fields.is_empty() {
            Ok(None)
        } else {
            Ok(Some(AdifRecord { fields, excess_data: String::new() }))
        }
    }

    fn parse_field(&mut self) -> Result<Option<AdifField>, TransadifError> {
        if !self.is_field_start() {
            return Ok(None);
        }

        let field_re = Regex::new(r"^<([a-zA-Z][a-zA-Z0-9_]*):(\d+)(?::([a-zA-Z][a-zA-Z0-9_]*))?>")?;

        // Find the end of the field tag
        let tag_start = self.pos;
        let mut tag_end = self.pos;
        while tag_end < self.bytes.len() && self.bytes[tag_end] != b'>' {
            tag_end += 1;
        }

        if tag_end >= self.bytes.len() {
            return Err(TransadifError::ParseError("Incomplete field tag".to_string()));
        }

        tag_end += 1; // Include the '>'

        let tag_bytes = &self.bytes[tag_start..tag_end];
        let tag_str = convert_to_unicode(tag_bytes, self.detected_encoding)?;

        if let Some(captures) = field_re.captures(&tag_str) {
            let name = captures.get(1).unwrap().as_str().to_string();
            let length: usize = captures.get(2).unwrap().as_str().parse()
                .map_err(|_| TransadifError::ParseError("Invalid field length".to_string()))?;
            let data_type = captures.get(3).map(|m| m.as_str().to_string());

            self.pos = tag_end;

            // Parse field data
            let data_start = self.pos;
            let (data, _actual_length) = self.parse_field_data(length)?;
            let data_end = self.pos;

            // Store raw data for analysis
            let raw_data = self.bytes[data_start..data_end].to_vec();

            // Parse excess data until next field
            let excess_start = self.pos;
            while self.pos < self.bytes.len() &&
                  !self.is_field_start() &&
                  !self.is_at_sequence(b"<eor>") &&
                  !self.is_at_sequence(b"<eoh>") {
                self.pos += 1;
            }

            let excess_data = if self.pos > excess_start {
                let excess_bytes = &self.bytes[excess_start..self.pos];
                convert_to_unicode(excess_bytes, self.detected_encoding)?
            } else {
                String::new()
            };

            Ok(Some(AdifField {
                name,
                length,
                data_type,
                data,
                excess_data,
                raw_data,
            }))
        } else {
            Err(TransadifError::InvalidFieldFormat(tag_str))
        }
    }

    fn parse_field_data(&mut self, expected_length: usize) -> Result<(String, usize), TransadifError> {
        if self.pos >= self.bytes.len() {
            return Ok((String::new(), 0));
        }

        // Try interpreting length as bytes first
        let bytes_end = (self.pos + expected_length).min(self.bytes.len());
        let data_bytes = &self.bytes[self.pos..bytes_end];

        // Try to convert to unicode
        let data_str = if let Ok(utf8_str) = std::str::from_utf8(data_bytes) {
            // It's valid UTF-8
            utf8_str.to_string()
        } else {
            // Use detected encoding
            convert_to_unicode(data_bytes, self.detected_encoding)?
        };

        // Check if we need to reinterpret the length (bytes vs characters)
        // This implements the heuristic from GOALS.md about field count mismatches
        if !self.config.strict_mode && std::str::from_utf8(data_bytes).is_ok() {
            // The data is valid UTF-8, check if interpreting as character count makes more sense
            let char_count = data_str.chars().count();

            // Look ahead to see what comes after this field
            let lookahead_start = self.pos + expected_length;
            let lookahead_end = (lookahead_start + 20).min(self.bytes.len());
            let lookahead_bytes = &self.bytes[lookahead_start..lookahead_end];
            let lookahead_str = std::str::from_utf8(lookahead_bytes).unwrap_or("");

            // Only reinterpret if:
            // 1. Character count is less than expected length (suggesting byte count was used for UTF-8)
            // 2. The excess data after the field contains non-whitespace (suggesting truncation)
            // 3. We can successfully read the expected number of characters
            if char_count < expected_length {
                let excess_non_whitespace = lookahead_str.chars()
                    .take_while(|&c| c != '<')  // Stop at next field
                    .any(|c| !c.is_whitespace());

                if excess_non_whitespace {
                    // Try to read expected_length characters instead of bytes
                    let mut char_end = self.pos;
                    let mut chars_read = 0;

                    while char_end < self.bytes.len() && chars_read < expected_length {
                        if let Some(ch) = std::str::from_utf8(&self.bytes[char_end..]).ok()
                            .and_then(|s| s.chars().next()) {
                            if ch == '<' {  // Stop at next field
                                break;
                            }
                            char_end += ch.len_utf8();
                            chars_read += 1;
                        } else {
                            break;
                        }
                    }

                    if chars_read == expected_length {
                        // Successfully read the expected number of characters
                        let char_data_bytes = &self.bytes[self.pos..char_end];
                        if let Ok(char_data_str) = std::str::from_utf8(char_data_bytes) {
                            let original_pos = self.pos;
                            self.pos = char_end;
                            return Ok((char_data_str.to_string(), char_end - original_pos));
                        }
                    }
                }
            }
        }

        self.pos = bytes_end;
        Ok((data_str, data_bytes.len()))
    }

    fn is_field_start(&self) -> bool {
        if self.pos >= self.bytes.len() || self.bytes[self.pos] != b'<' {
            return false;
        }

        // Look for pattern <fieldname:length> or <fieldname:length:type>
        let mut i = self.pos + 1;

        // Field name (alphanumeric + underscore, starting with letter)
        if i >= self.bytes.len() || !self.bytes[i].is_ascii_alphabetic() {
            return false;
        }

        while i < self.bytes.len() && (self.bytes[i].is_ascii_alphanumeric() || self.bytes[i] == b'_') {
            i += 1;
        }

        // Should be followed by ':'
        if i >= self.bytes.len() || self.bytes[i] != b':' {
            return false;
        }
        i += 1;

        // Should be followed by digits (length)
        if i >= self.bytes.len() || !self.bytes[i].is_ascii_digit() {
            return false;
        }

        while i < self.bytes.len() && self.bytes[i].is_ascii_digit() {
            i += 1;
        }

        // Can be followed by ':' and type, or just '>'
        if i < self.bytes.len() && self.bytes[i] == b':' {
            i += 1;
            // Type name
            if i >= self.bytes.len() || !self.bytes[i].is_ascii_alphabetic() {
                return false;
            }
            while i < self.bytes.len() && (self.bytes[i].is_ascii_alphanumeric() || self.bytes[i] == b'_') {
                i += 1;
            }
        }

        // Should end with '>'
        i < self.bytes.len() && self.bytes[i] == b'>'
    }

    fn is_at_sequence(&self, sequence: &[u8]) -> bool {
        if self.pos + sequence.len() > self.bytes.len() {
            return false;
        }

        self.bytes[self.pos..self.pos + sequence.len()]
            .eq_ignore_ascii_case(sequence)
    }
}
