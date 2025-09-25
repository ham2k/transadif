use crate::error::{Result, TransadifError};

#[derive(Debug, Clone, PartialEq)]
pub struct AdifField {
    pub name: String,
    pub length: usize,
    pub field_type: Option<String>,
    pub data: String,
    pub raw_data: Vec<u8>,
    pub excess_data: String,
}

#[derive(Debug, Clone)]
pub struct AdifRecord {
    pub fields: Vec<AdifField>,
    pub excess_data: String,
}

#[derive(Debug, Clone)]
pub struct AdifHeader {
    pub preamble: String,
    pub fields: Vec<AdifField>,
    pub excess_data: String,
}

#[derive(Debug, Clone)]
pub struct AdifFile {
    pub header: Option<AdifHeader>,
    pub records: Vec<AdifRecord>,
    pub detected_encoding: Option<String>,
}

impl AdifFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut parser = AdifParser::new(data);
        parser.parse()
    }

    pub fn get_encoding_from_header(&self) -> Option<String> {
        if let Some(header) = &self.header {
            for field in &header.fields {
                if field.name.to_lowercase() == "encoding" {
                    return Some(field.data.clone());
                }
            }
        }
        None
    }
}

struct AdifParser<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> AdifParser<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    fn parse(&mut self) -> Result<AdifFile> {
        let header = self.parse_header()?;
        let mut records = Vec::new();

        while self.position < self.data.len() {
            if let Some(record) = self.parse_record()? {
                records.push(record);
            } else {
                break;
            }
        }

        Ok(AdifFile {
            header,
            records,
            detected_encoding: None,
        })
    }

    fn parse_header(&mut self) -> Result<Option<AdifHeader>> {
        if self.position >= self.data.len() {
            return Ok(None);
        }

        // Check if file starts with '<' (no header)
        if self.data[self.position] == b'<' {
            return Ok(None);
        }

        let mut preamble = String::new();
        let mut fields = Vec::new();

        // Find the end of header marker
        while self.position < self.data.len() {
            if self.check_for_eoh() {
                // Parse any fields in the header
                let header_data = &self.data[..self.position];
                let mut field_parser = AdifParser::new(header_data);
                field_parser.position = preamble.len();

                while field_parser.position < header_data.len() {
                    if field_parser.check_for_eoh() {
                        break;
                    }
                    if let Some(field) = field_parser.parse_field()? {
                        fields.push(field);
                    } else {
                        field_parser.position += 1;
                    }
                }

                // Skip the <eoh> tag
                self.skip_eoh();
                let excess_data = self.consume_until_next_field();

                return Ok(Some(AdifHeader {
                    preamble,
                    fields,
                    excess_data,
                }));
            }

            preamble.push(self.data[self.position] as char);
            self.position += 1;
        }

        Ok(None)
    }

    fn parse_record(&mut self) -> Result<Option<AdifRecord>> {
        if self.position >= self.data.len() {
            return Ok(None);
        }

        let mut fields = Vec::new();

        while self.position < self.data.len() {
            if self.check_for_eor() {
                self.skip_eor();
                let excess_data = self.consume_until_next_field();
                return Ok(Some(AdifRecord {
                    fields,
                    excess_data,
                }));
            }

            if let Some(field) = self.parse_field()? {
                fields.push(field);
            } else {
                self.position += 1;
            }
        }

        if !fields.is_empty() {
            Ok(Some(AdifRecord {
                fields,
                excess_data: String::new(),
            }))
        } else {
            Ok(None)
        }
    }

    fn parse_field(&mut self) -> Result<Option<AdifField>> {
        if self.position >= self.data.len() || self.data[self.position] != b'<' {
            return Ok(None);
        }

        let start_pos = self.position;
        self.position += 1; // Skip '<'

        // Parse field name
        let mut field_name = String::new();
        while self.position < self.data.len() && self.data[self.position] != b':' {
            let ch = self.data[self.position] as char;
            if ch.is_ascii_alphanumeric() || ch == '_' {
                field_name.push(ch);
                self.position += 1;
            } else {
                // Invalid field name character, backtrack
                self.position = start_pos;
                return Ok(None);
            }
        }

        if self.position >= self.data.len() || field_name.is_empty() {
            self.position = start_pos;
            return Ok(None);
        }

        self.position += 1; // Skip ':'

        // Parse length
        let mut length_str = String::new();
        while self.position < self.data.len() && self.data[self.position].is_ascii_digit() {
            length_str.push(self.data[self.position] as char);
            self.position += 1;
        }

        if length_str.is_empty() {
            self.position = start_pos;
            return Ok(None);
        }

        let length: usize = length_str.parse().map_err(|_| {
            TransadifError::Parse(format!("Invalid length: {}", length_str))
        })?;

        // Check for optional type
        let mut field_type = None;
        if self.position < self.data.len() && self.data[self.position] == b':' {
            self.position += 1; // Skip ':'
            let mut type_str = String::new();
            while self.position < self.data.len() && self.data[self.position] != b'>' {
                let ch = self.data[self.position] as char;
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    type_str.push(ch);
                    self.position += 1;
                } else {
                    break;
                }
            }
            if !type_str.is_empty() {
                field_type = Some(type_str);
            }
        }

        // Expect '>'
        if self.position >= self.data.len() || self.data[self.position] != b'>' {
            self.position = start_pos;
            return Ok(None);
        }

        self.position += 1; // Skip '>'

        // Extract raw data
        let data_start = self.position;
        let data_end = std::cmp::min(self.position + length, self.data.len());
        let mut raw_data = self.data[data_start..data_end].to_vec();
        self.position = data_end;

        // Consume excess data until next field
        let mut excess_data = self.consume_until_next_field();

        // Apply field count heuristics if needed
        let mut actual_length = length;
        if self.should_reinterpret_field_count(&raw_data, &excess_data) {
            // Try reinterpreting as character count instead of byte count
            if let Some((new_raw_data, new_excess_data)) = self.try_reinterpret_field_count(data_start, length, &excess_data) {
                raw_data = new_raw_data;
                excess_data = new_excess_data;
                actual_length = raw_data.len();
            }
        }

        // Convert raw data to string (initially as ISO-8859-1 for safety)
        let data = String::from_utf8_lossy(&raw_data).to_string();

        Ok(Some(AdifField {
            name: field_name,
            length: actual_length,
            field_type,
            data,
            raw_data,
            excess_data,
        }))
    }

    fn check_for_eoh(&self) -> bool {
        self.check_for_tag(b"eoh")
    }

    fn check_for_eor(&self) -> bool {
        self.check_for_tag(b"eor")
    }

    fn check_for_tag(&self, tag: &[u8]) -> bool {
        if self.position + tag.len() + 2 > self.data.len() {
            return false;
        }

        if self.data[self.position] != b'<' {
            return false;
        }

        let tag_start = self.position + 1;
        let tag_end = tag_start + tag.len();

        if tag_end >= self.data.len() || self.data[tag_end] != b'>' {
            return false;
        }

        let found_tag = &self.data[tag_start..tag_end];
        found_tag.to_ascii_lowercase() == tag
    }

    fn skip_eoh(&mut self) {
        self.skip_tag(b"eoh");
    }

    fn skip_eor(&mut self) {
        self.skip_tag(b"eor");
    }

    fn skip_tag(&mut self, tag: &[u8]) {
        if self.check_for_tag(tag) {
            self.position += tag.len() + 2; // Skip '<tag>'
        }
    }

    fn consume_until_next_field(&mut self) -> String {
        let start = self.position;
        while self.position < self.data.len() {
            if self.data[self.position] == b'<' {
                // Check if this is a valid field start or EOR/EOH
                if self.check_for_eor() || self.check_for_eoh() {
                    break;
                }

                let saved_pos = self.position;
                if self.parse_field().unwrap_or(None).is_some() {
                    // This was a valid field, backtrack
                    self.position = saved_pos;
                    break;
                } else {
                    // Not a valid field, continue
                    self.position = saved_pos + 1;
                }
            } else {
                self.position += 1;
            }
        }

        String::from_utf8_lossy(&self.data[start..self.position]).to_string()
    }

    fn should_reinterpret_field_count(&self, raw_data: &[u8], excess_data: &str) -> bool {
        // Check if excess data contains non-whitespace characters
        let has_non_whitespace_excess = !excess_data.trim().is_empty();

        // Check if raw data OR excess data contains UTF-8 sequences (including corrupted ones)
        let has_utf8_in_raw = std::str::from_utf8(raw_data).is_ok() &&
                             raw_data.iter().any(|&b| b > 127);

        let has_utf8_in_excess = excess_data.chars().any(|c| c as u32 > 127);

        let has_utf8_sequences = has_utf8_in_raw || has_utf8_in_excess;


        has_non_whitespace_excess && has_utf8_sequences
    }

    fn try_reinterpret_field_count(&self, data_start: usize, original_length: usize, excess_data: &str) -> Option<(Vec<u8>, String)> {
        // Try to find the actual end of the field by looking for UTF-8 character boundaries
        let available_data = &self.data[data_start..];

        // Try to decode as UTF-8 and count characters
        if let Ok(utf8_str) = std::str::from_utf8(available_data) {
            let chars: Vec<char> = utf8_str.chars().collect();
            if chars.len() >= original_length {
                // Take the specified number of characters instead of bytes
                let char_data: String = chars.iter().take(original_length).collect();
                let char_bytes = char_data.as_bytes().to_vec();

                // Calculate new excess data
                let char_byte_len = char_bytes.len();
                let remaining_start = data_start + char_byte_len;
                let remaining_data = &self.data[remaining_start..];

                // Find where the next field starts
                let mut new_excess_data = String::new();
                for (i, &byte) in remaining_data.iter().enumerate() {
                    if byte == b'<' {
                        // Check if this might be a field or tag
                        let remaining = &remaining_data[i..];
                        if remaining.len() > 1 {
                            new_excess_data = String::from_utf8_lossy(&remaining_data[..i]).to_string();
                            break;
                        }
                    }
                }

                // Only reinterpret if the new excess data is mostly whitespace
                if new_excess_data.trim().is_empty() || new_excess_data.trim().len() < excess_data.trim().len() {
                    return Some((char_bytes, new_excess_data));
                }
            }
        }

        None
    }
}
