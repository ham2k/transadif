use crate::encoding::{EncodingDetector, EncodingOptions};
use crate::error::{Result, TransAdifError};
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
    pub field_type: Option<String>,
    pub data: String,
    pub excess_data: String,
}

pub struct AdifParser {
    field_regex: Regex,
    eoh_regex: Regex,
    eor_regex: Regex,
    encoding_detector: EncodingDetector,
}

impl AdifParser {
    pub fn new() -> Self {
        Self {
            field_regex: Regex::new(r"(?i)<([A-Za-z0-9_]+):(\d+)(?::([A-Za-z0-9_]+))?>").unwrap(),
            eoh_regex: Regex::new(r"(?i)<eoh>").unwrap(),
            eor_regex: Regex::new(r"(?i)<eor>").unwrap(),
            encoding_detector: EncodingDetector::new(),
        }
    }

    pub fn parse(&self, data: &[u8], opts: &EncodingOptions) -> Result<AdifFile> {
        // Detect encoding
        let encoding = self.encoding_detector.detect_encoding(data, opts.input_encoding.as_deref())?;

        // Decode to Unicode
        let text = self.encoding_detector.decode_to_unicode(data, encoding, opts.strict_mode)?;

        // Parse structure
        let (header, remaining) = self.parse_header(&text)?;
        let records = self.parse_records(&remaining, opts)?;

        Ok(AdifFile { header, records })
    }

    fn parse_header<'a>(&self, text: &'a str) -> Result<(AdifHeader, &'a str)> {
        // Check if file starts with '<' (no header)
        if text.trim_start().starts_with('<') && !text.trim_start().to_lowercase().starts_with("<eoh>") {
            return Ok((
                AdifHeader {
                    preamble: String::new(),
                    fields: Vec::new(),
                    excess_data: String::new(),
                },
                text,
            ));
        }

        // Find end of header
        if let Some(eoh_match) = self.eoh_regex.find(text) {
            let header_text = &text[..eoh_match.start()];
            let remaining = &text[eoh_match.end()..];

            // Parse header content
            let (preamble, header_fields, excess_data) = self.parse_header_content(header_text)?;

            // Extract excess data after <eoh>
            let (header_excess, record_start) = self.extract_excess_data(remaining);
            let final_excess = if excess_data.is_empty() {
                header_excess
            } else if header_excess.is_empty() {
                excess_data
            } else {
                format!("{}{}", excess_data, header_excess)
            };

            Ok((
                AdifHeader {
                    preamble,
                    fields: header_fields,
                    excess_data: final_excess,
                },
                record_start,
            ))
        } else {
            // No <eoh> found, treat entire content as preamble
            Ok((
                AdifHeader {
                    preamble: text.to_string(),
                    fields: Vec::new(),
                    excess_data: String::new(),
                },
                "",
            ))
        }
    }

    fn parse_header_content(&self, header_text: &str) -> Result<(String, Vec<AdifField>, String)> {
        let mut preamble = String::new();
        let mut fields: Vec<AdifField> = Vec::new();
        let mut excess_data = String::new();
        let mut current_pos = 0;
        let mut found_first_field = false;

        for field_match in self.field_regex.find_iter(header_text) {
            if !found_first_field {
                preamble = header_text[..field_match.start()].to_string();
                found_first_field = true;
            } else {
                // Excess data between fields
                if current_pos < field_match.start() {
                    if let Some(last_field) = fields.last_mut() {
                        last_field.excess_data = header_text[current_pos..field_match.start()].to_string();
                    }
                }
            }

            let field = self.parse_field_at_match(header_text, &field_match)?;
            current_pos = field_match.end() + field.data.len();
            fields.push(field);
        }

        // Handle remaining text after last field
        if current_pos < header_text.len() {
            excess_data = header_text[current_pos..].to_string();
        }

        // If no fields found, everything is preamble
        if !found_first_field {
            preamble = header_text.to_string();
        }

        Ok((preamble, fields, excess_data))
    }

    fn parse_records(&self, text: &str, opts: &EncodingOptions) -> Result<Vec<AdifRecord>> {
        let mut records = Vec::new();
        let mut remaining = text;

        while !remaining.trim().is_empty() {
            if let Some(eor_match) = self.eor_regex.find(remaining) {
                let record_text = &remaining[..eor_match.start()];
                let record = self.parse_single_record(record_text, opts)?;
                records.push(record);

                // Extract excess data after <eor>
                remaining = &remaining[eor_match.end()..];
                let (excess, next_record) = self.extract_excess_data(remaining);
                if !excess.is_empty() {
                    if let Some(last_record) = records.last_mut() {
                        last_record.excess_data = excess;
                    }
                }
                remaining = next_record;
            } else {
                // No more <eor> tags, parse remaining as incomplete record
                if !remaining.trim().is_empty() {
                    let record = self.parse_single_record(remaining, opts)?;
                    records.push(record);
                }
                break;
            }
        }

        Ok(records)
    }

    fn parse_single_record(&self, record_text: &str, _opts: &EncodingOptions) -> Result<AdifRecord> {
        let mut fields: Vec<AdifField> = Vec::new();
        let mut current_pos = 0;

        for field_match in self.field_regex.find_iter(record_text) {
            // Handle excess data before this field
            if current_pos < field_match.start() {
                if let Some(last_field) = fields.last_mut() {
                    last_field.excess_data = record_text[current_pos..field_match.start()].to_string();
                }
            }

            let field = self.parse_field_at_match(record_text, &field_match)?;
            current_pos = field_match.end() + field.data.len();
            fields.push(field);
        }

        // Handle remaining text after last field
        let excess_data = if current_pos < record_text.len() {
            record_text[current_pos..].to_string()
        } else {
            String::new()
        };

        Ok(AdifRecord { fields, excess_data })
    }

    fn parse_field_at_match(&self, text: &str, field_match: &regex::Match) -> Result<AdifField> {
        let captures = self.field_regex.captures(&text[field_match.range()]).unwrap();

        let name = captures.get(1).unwrap().as_str().to_string();
        let length_str = captures.get(2).unwrap().as_str();
        let length = length_str.parse::<usize>()
            .map_err(|_| TransAdifError::InvalidField(format!("Invalid length: {}", length_str)))?;
        let field_type = captures.get(3).map(|m| m.as_str().to_string());

        // Extract field data - handle both byte and character counting
        let data_start_byte = field_match.end();

        // First try byte-based extraction (traditional ADIF)
        let data_end_byte = data_start_byte + length;
        let data = if data_end_byte <= text.len() {
            let candidate = text[data_start_byte..data_end_byte].to_string();

            // Check if this extraction resulted in valid UTF-8 and reasonable length
            // If the candidate is shorter in characters than expected,
            // it might be that the length was specified in characters, not bytes
            if candidate.chars().count() < length {
                // Try character-based extraction instead
                let char_start = text[..data_start_byte].chars().count();
                let char_extracted: String = text.chars().skip(char_start).take(length).collect();
                if !char_extracted.is_empty() {
                    char_extracted
                } else {
                    candidate
                }
            } else {
                candidate
            }
        } else {
            // Byte-based extraction would go beyond string, try character-based
            let char_start = text[..data_start_byte].chars().count();
            let char_extracted: String = text.chars().skip(char_start).take(length).collect();
            if char_extracted.is_empty() {
                return Err(TransAdifError::Parse {
                    pos: data_start_byte,
                    msg: format!("Field {} claims length {} but insufficient data", name, length),
                });
            }
            char_extracted
        };

        Ok(AdifField {
            name,
            length,
            field_type,
            data,
            excess_data: String::new(),
        })
    }

    fn extract_excess_data<'a>(&self, text: &'a str) -> (String, &'a str) {
        // Find the next field start
        if let Some(field_match) = self.field_regex.find(text) {
            (text[..field_match.start()].to_string(), &text[field_match.start()..])
        } else {
            (text.to_string(), "")
        }
    }
}

impl AdifFile {
    pub fn parse(data: &[u8], opts: &EncodingOptions) -> Result<Self> {
        let parser = AdifParser::new();
        parser.parse(data, opts)
    }

    /// Get encoding from header fields
    pub fn get_encoding(&self) -> Option<String> {
        self.header.fields.iter()
            .find(|f| f.name.to_uppercase() == "ENCODING")
            .map(|f| f.data.clone())
    }

    /// Set or update encoding in header
    pub fn set_encoding(&mut self, encoding: &str) {
        // Remove existing encoding field
        self.header.fields.retain(|f| f.name.to_uppercase() != "ENCODING");

        // Add new encoding field after PROGRAMID if it exists, otherwise at the beginning
        let insert_pos = if let Some(pos) = self.header.fields.iter().position(|f| f.name.to_uppercase() == "PROGRAMID") {
            pos + 1
        } else {
            0
        };

        self.header.fields.insert(insert_pos, AdifField {
            name: "ENCODING".to_string(),
            length: encoding.len(),
            field_type: None,
            data: encoding.to_string(),
            excess_data: String::new(),
        });
    }

    /// Ensure PROGRAMID is set to TransADIF
    pub fn set_program_id(&mut self) {
        // Remove existing PROGRAMID field
        self.header.fields.retain(|f| f.name.to_uppercase() != "PROGRAMID");

        // Add TransADIF PROGRAMID at the beginning
        let program_id = "TransADIF";
        self.header.fields.insert(0, AdifField {
            name: "PROGRAMID".to_string(),
            length: program_id.len(),
            field_type: None,
            data: program_id.to_string(),
            excess_data: String::new(),
        });
    }
}
