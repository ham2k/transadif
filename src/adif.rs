use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdifError {
    #[error("Invalid field format: {0}")]
    InvalidField(String),
    #[error("Encoding error: {0}")]
    EncodingError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone)]
pub enum FieldCountMode {
    Bytes,
    Characters,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub length: usize,
    pub field_type: Option<String>,
    pub data: String,
    pub excess_data: String,
    pub original_bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Record {
    pub fields: Vec<Field>,
    pub excess_data: String,
}

#[derive(Debug, Clone)]
pub struct AdifFile {
    pub preamble: String,
    pub header_fields: Vec<Field>,
    pub header_excess_data: String,
    pub records: Vec<Record>,
    pub encoding: Option<String>,
}

impl AdifFile {
    pub fn new() -> Self {
        Self {
            preamble: String::new(),
            header_fields: Vec::new(),
            header_excess_data: String::new(),
            records: Vec::new(),
            encoding: None,
        }
    }

    pub fn parse(data: &[u8]) -> Result<Self, AdifError> {
        let mut parser = AdifParser::new(data);
        parser.parse()
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

    fn parse(&mut self) -> Result<AdifFile, AdifError> {
        let mut adif = AdifFile::new();

        // Check if file starts with '<' (no header)
        if self.peek_byte() == Some(b'<') {
            // No header, start parsing records
            adif.records = self.parse_records()?;
        } else {
            // Parse header
            adif.preamble = self.parse_preamble()?;
            adif.header_fields = self.parse_header_fields()?;
            adif.header_excess_data = self.parse_excess_until_record()?;
            adif.records = self.parse_records()?;
        }

        // Extract encoding from header fields
        for field in &adif.header_fields {
            if field.name.to_lowercase() == "encoding" {
                adif.encoding = Some(field.data.clone());
                break;
            }
        }

        Ok(adif)
    }

    fn parse_preamble(&mut self) -> Result<String, AdifError> {
        let start = self.position;

        // Find the start of the first field or <eoh>
        while self.position < self.data.len() {
            if self.peek_byte() == Some(b'<') {
                // Check if this is <eoh>
                if self.is_at_eoh() {
                    break;
                }
                // Check if this looks like a field
                if self.is_at_field() {
                    break;
                }
            }
            self.position += 1;
        }

        let preamble_bytes = &self.data[start..self.position];
        Ok(String::from_utf8_lossy(preamble_bytes).to_string())
    }

    fn parse_header_fields(&mut self) -> Result<Vec<Field>, AdifError> {
        let mut fields = Vec::new();

        while self.position < self.data.len() {
            if self.is_at_eoh() {
                // Skip <eoh>
                self.skip_eoh();
                break;
            }

            if self.is_at_field() {
                fields.push(self.parse_field()?);
            } else {
                self.position += 1;
            }
        }

        Ok(fields)
    }

    fn parse_records(&mut self) -> Result<Vec<Record>, AdifError> {
        let mut records = Vec::new();

        while self.position < self.data.len() {
            if self.is_at_field() {
                let record = self.parse_record()?;
                records.push(record);
            } else {
                self.position += 1;
            }
        }

        Ok(records)
    }

    fn parse_record(&mut self) -> Result<Record, AdifError> {
        let mut fields = Vec::new();

        while self.position < self.data.len() {
            if self.is_at_eor() {
                // Skip <eor>
                self.skip_eor();
                break;
            }

            if self.is_at_field() {
                fields.push(self.parse_field()?);
            } else {
                self.position += 1;
            }
        }

        let excess_data = self.parse_excess_until_record()?;

        Ok(Record {
            fields,
            excess_data,
        })
    }

    fn parse_field(&mut self) -> Result<Field, AdifError> {
        self.parse_field_with_count_mode(None)
    }

    fn parse_field_with_count_mode(&mut self, count_mode: Option<FieldCountMode>) -> Result<Field, AdifError> {
        if self.peek_byte() != Some(b'<') {
            return Err(AdifError::InvalidField("Field must start with '<'".to_string()));
        }

        self.position += 1; // Skip '<'

        // Parse field name
        let name_start = self.position;
        while self.position < self.data.len() && self.peek_byte() != Some(b':') {
            self.position += 1;
        }

        if self.position >= self.data.len() {
            return Err(AdifError::InvalidField("Unexpected end of field".to_string()));
        }

        let name = String::from_utf8_lossy(&self.data[name_start..self.position]).to_string();
        self.position += 1; // Skip ':'

        // Parse length
        let length_start = self.position;
        while self.position < self.data.len() && self.peek_byte().unwrap().is_ascii_digit() {
            self.position += 1;
        }

        if self.position == length_start {
            return Err(AdifError::InvalidField("Missing field length".to_string()));
        }

        let length_str = String::from_utf8_lossy(&self.data[length_start..self.position]);
        let declared_length: usize = length_str.parse()
            .map_err(|_| AdifError::InvalidField("Invalid field length".to_string()))?;

        // Check for optional type
        let mut field_type = None;
        if self.peek_byte() == Some(b':') {
            self.position += 1; // Skip ':'
            let type_start = self.position;
            while self.position < self.data.len() && self.peek_byte() != Some(b'>') {
                self.position += 1;
            }
            field_type = Some(String::from_utf8_lossy(&self.data[type_start..self.position]).to_string());
        }

        if self.peek_byte() != Some(b'>') {
            return Err(AdifError::InvalidField("Field must end with '>'".to_string()));
        }

        self.position += 1; // Skip '>'

        // Try to parse data with the declared length first
        let (final_length, data_bytes, excess_data) =
            self.parse_field_data_with_count_handling(declared_length, count_mode)?;

        let data = String::from_utf8_lossy(data_bytes).to_string();

        Ok(Field {
            name,
            length: final_length,
            field_type,
            data,
            excess_data,
            original_bytes: data_bytes.to_vec(),
        })
    }

    fn parse_field_data_with_count_handling(
        &mut self,
        declared_length: usize,
        count_mode: Option<FieldCountMode>
    ) -> Result<(usize, &[u8], String), AdifError> {
        let data_start = self.position;

        // First attempt with declared length as bytes
        let data_end = std::cmp::min(self.position + declared_length, self.data.len());
        let data_bytes = &self.data[data_start..data_end];
        self.position = data_end;

        // Parse excess data to check if reinterpretation is needed
        let excess_start = self.position;
        while self.position < self.data.len() {
            if self.is_at_field() || self.is_at_eor() || self.is_at_eoh() {
                break;
            }
            self.position += 1;
        }

        let excess_data = String::from_utf8_lossy(&self.data[excess_start..self.position]).to_string();

        // Check if we need to reinterpret the field count
        if self.should_reinterpret_field_count(data_bytes, &excess_data, count_mode) {
            // Try character-based counting
            if let Some((char_end, char_byte_count)) = self.calculate_character_based_field(data_start, declared_length) {
                // Reset position for character-based parsing
                self.position = char_end;

                // Parse new excess data
                let new_excess_start = self.position;
                while self.position < self.data.len() {
                    if self.is_at_field() || self.is_at_eor() || self.is_at_eoh() {
                        break;
                    }
                    self.position += 1;
                }

                let new_excess_data = String::from_utf8_lossy(&self.data[new_excess_start..self.position]).to_string();

                // If the new interpretation produces cleaner excess data, use it
                if self.is_excess_data_cleaner(&new_excess_data, &excess_data) {
                    let char_data_bytes = &self.data[data_start..char_end];
                    return Ok((declared_length, char_data_bytes, new_excess_data));
                }
            }

            // Revert to original interpretation
            self.position = excess_start + excess_data.as_bytes().len();
        }

        Ok((declared_length, data_bytes, excess_data))
    }

    fn calculate_character_based_field(&self, start_pos: usize, n: usize) -> Option<(usize, usize)> {
        let mut pos = start_pos;
        let mut char_count = 0;

        while pos < self.data.len() && char_count < n {
            // Try to decode the next character
            let remaining = &self.data[pos..];
            if let Some(ch) = std::str::from_utf8(remaining).ok()?.chars().next() {
                pos += ch.len_utf8();
                char_count += 1;
            } else {
                // Not valid UTF-8, treat as single byte
                pos += 1;
                char_count += 1;
            }
        }

        if char_count == n {
            Some((pos, pos - start_pos))
        } else {
            None
        }
    }

    fn should_reinterpret_field_count(
        &self,
        data_bytes: &[u8],
        excess_data: &str,
        _count_mode: Option<FieldCountMode>
    ) -> bool {
        // Only reinterpret if excess data contains non-whitespace
        if excess_data.trim().is_empty() {
            return false;
        }

        // Check if data contains UTF-8 sequences
        self.has_utf8_sequences_in_bytes(data_bytes)
    }

    fn has_utf8_sequences_in_bytes(&self, data: &[u8]) -> bool {
        let mut i = 0;
        while i < data.len() {
            if data[i] > 127 {
                // Check for valid UTF-8 sequence
                let mut count = 0;
                if data[i] & 0b11100000 == 0b11000000 {
                    count = 1;
                } else if data[i] & 0b11110000 == 0b11100000 {
                    count = 2;
                } else if data[i] & 0b11111000 == 0b11110000 {
                    count = 3;
                }

                if count > 0 && i + count < data.len() {
                    let mut valid = true;
                    for j in 1..=count {
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

    fn try_reinterpret_field_count(&self, declared_length: usize, data_bytes: &[u8]) -> Option<usize> {
        // If we have UTF-8 sequences and non-whitespace excess data,
        // the declared length is likely in bytes but should be in characters
        if self.has_utf8_sequences_in_bytes(data_bytes) {
            if let Ok(utf8_str) = std::str::from_utf8(data_bytes) {
                let char_count = utf8_str.chars().count();
                // If the character count is different from declared length,
                // we might need to read more data to get the full character count
                if char_count < declared_length {
                    // We need more bytes to reach the character count
                    return Some(declared_length); // Keep trying with character-based counting
                }
            }
        }

        // Try interpreting as bytes instead of characters
        if data_bytes.len() != declared_length {
            return Some(data_bytes.len());
        }

        None
    }

    fn is_excess_data_cleaner(&self, new_excess: &str, old_excess: &str) -> bool {
        let new_non_whitespace = new_excess.chars().filter(|c| !c.is_whitespace()).count();
        let old_non_whitespace = old_excess.chars().filter(|c| !c.is_whitespace()).count();

        new_non_whitespace < old_non_whitespace
    }

    fn parse_excess_until_record(&mut self) -> Result<String, AdifError> {
        let start = self.position;

        while self.position < self.data.len() {
            if self.is_at_field() {
                break;
            }
            self.position += 1;
        }

        Ok(String::from_utf8_lossy(&self.data[start..self.position]).to_string())
    }

    fn peek_byte(&self) -> Option<u8> {
        if self.position < self.data.len() {
            Some(self.data[self.position])
        } else {
            None
        }
    }

    fn is_at_eoh(&self) -> bool {
        self.check_tag(b"eoh")
    }

    fn is_at_eor(&self) -> bool {
        self.check_tag(b"eor")
    }

    fn is_at_field(&self) -> bool {
        if self.peek_byte() != Some(b'<') {
            return false;
        }

        // Look ahead to see if this looks like a field
        let mut pos = self.position + 1;

        // Skip field name (alphanumeric + underscore)
        while pos < self.data.len() {
            let byte = self.data[pos];
            if byte == b':' {
                break;
            }
            if !byte.is_ascii_alphanumeric() && byte != b'_' {
                return false;
            }
            pos += 1;
        }

        if pos >= self.data.len() || self.data[pos] != b':' {
            return false;
        }

        pos += 1;

        // Check for length (digits)
        let length_start = pos;
        while pos < self.data.len() && self.data[pos].is_ascii_digit() {
            pos += 1;
        }

        if pos == length_start {
            return false;
        }

        // Optional type
        if pos < self.data.len() && self.data[pos] == b':' {
            pos += 1;
            while pos < self.data.len() && self.data[pos] != b'>' {
                let byte = self.data[pos];
                if !byte.is_ascii_alphanumeric() && byte != b'_' {
                    return false;
                }
                pos += 1;
            }
        }

        pos < self.data.len() && self.data[pos] == b'>'
    }

    fn check_tag(&self, tag: &[u8]) -> bool {
        if self.position + tag.len() + 2 > self.data.len() {
            return false;
        }

        if self.data[self.position] != b'<' {
            return false;
        }

        let tag_slice = &self.data[self.position + 1..self.position + 1 + tag.len()];
        let tag_match = tag_slice.eq_ignore_ascii_case(tag);

        if !tag_match {
            return false;
        }

        self.data[self.position + 1 + tag.len()] == b'>'
    }

    fn skip_eoh(&mut self) {
        self.skip_tag(b"eoh");
    }

    fn skip_eor(&mut self) {
        self.skip_tag(b"eor");
    }

    fn skip_tag(&mut self, tag: &[u8]) {
        if self.check_tag(tag) {
            self.position += tag.len() + 2; // '<' + tag + '>'
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_field() {
        let data = b"<call:5>K1MIX";
        let mut parser = AdifParser::new(data);
        let field = parser.parse_field().unwrap();

        assert_eq!(field.name, "call");
        assert_eq!(field.length, 5);
        assert_eq!(field.data, "K1MIX");
        assert!(field.field_type.is_none());
    }

    #[test]
    fn test_parse_field_with_type() {
        let data = b"<freq:5:N>7.200";
        let mut parser = AdifParser::new(data);
        let field = parser.parse_field().unwrap();

        assert_eq!(field.name, "freq");
        assert_eq!(field.length, 5);
        assert_eq!(field.data, "7.200");
        assert_eq!(field.field_type, Some("N".to_string()));
    }

    #[test]
    fn test_parse_simple_record() {
        let data = b"<call:5>K1MIX<band:3>40m<eor>";
        let mut parser = AdifParser::new(data);
        let record = parser.parse_record().unwrap();

        assert_eq!(record.fields.len(), 2);
        assert_eq!(record.fields[0].name, "call");
        assert_eq!(record.fields[0].data, "K1MIX");
        assert_eq!(record.fields[1].name, "band");
        assert_eq!(record.fields[1].data, "40m");
    }
}