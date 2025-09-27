use crate::adif::{AdifFile, Field, Record};
use crate::encoding::{AdifEncoding, EncodingProcessor};
use std::io::Write;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OutputError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Encoding error: {0}")]
    Encoding(#[from] crate::encoding::EncodingError),
}

pub struct OutputFormatter {
    processor: EncodingProcessor,
    output_encoding: AdifEncoding,
    replacement_char: Option<char>,
    delete_incompatible: bool,
    transliterate_ascii: bool,
}

impl OutputFormatter {
    pub fn new(
        input_encoding: Option<AdifEncoding>,
        output_encoding: AdifEncoding,
        strict_mode: bool,
        replacement_char: Option<char>,
        delete_incompatible: bool,
        transliterate_ascii: bool,
    ) -> Self {
        let processor = EncodingProcessor::new(input_encoding, output_encoding.clone(), strict_mode);

        Self {
            processor,
            output_encoding,
            replacement_char,
            delete_incompatible,
            transliterate_ascii,
        }
    }

    pub fn format_adif<W: Write>(&self, adif: &AdifFile, writer: &mut W) -> Result<(), OutputError> {
        // Write preamble
        if !adif.preamble.is_empty() {
            writer.write_all(adif.preamble.as_bytes())?;
        }

        // Write header fields first, then add encoding
        for field in &adif.header_fields {
            if field.name.to_lowercase() != "encoding" {
                self.write_field(writer, field)?;
            }
        }

        // Write encoding field after other header fields
        self.write_encoding_field(writer)?;

        // Write <eoh>
        writer.write_all(b"<eoh>")?;

        // Write header excess data
        if !adif.header_excess_data.is_empty() {
            writer.write_all(adif.header_excess_data.as_bytes())?;
        }

        // Write records
        for record in &adif.records {
            self.write_record(writer, record)?;
        }

        Ok(())
    }

    fn write_encoding_field<W: Write>(&self, writer: &mut W) -> Result<(), OutputError> {
        let encoding_name = self.output_encoding.to_string();
        let length = self.processor.count_length(&encoding_name, &self.output_encoding);

        write!(writer, "<encoding:{}>{}\r\n", length, encoding_name)?;
        Ok(())
    }

    fn write_field<W: Write>(&self, writer: &mut W, field: &Field) -> Result<(), OutputError> {
        // Process the field data
        let processed_data = self.processor.process_field_data(&field.original_bytes)?;
        let final_data = self.apply_output_transformations(&processed_data);

        // Calculate new length based on output encoding
        let length = self.processor.count_length(&final_data, &self.output_encoding);

        // Write field
        if let Some(ref field_type) = field.field_type {
            write!(writer, "<{}:{}:{}>{}", field.name, length, field_type, final_data)?;
        } else {
            write!(writer, "<{}:{}>{}", field.name, length, final_data)?;
        }

        // Write excess data (preserve as-is)
        if !field.excess_data.is_empty() {
            writer.write_all(field.excess_data.as_bytes())?;
        }

        Ok(())
    }

    fn write_record<W: Write>(&self, writer: &mut W, record: &Record) -> Result<(), OutputError> {
        for field in &record.fields {
            self.write_field(writer, field)?;
        }

        writer.write_all(b"<eor>")?;

        if !record.excess_data.is_empty() {
            writer.write_all(record.excess_data.as_bytes())?;
        }

        Ok(())
    }

    fn apply_output_transformations(&self, text: &str) -> String {
        let mut result = text.to_string();

        // Apply ASCII transliteration if requested
        if self.transliterate_ascii {
            result = self.transliterate_to_ascii(&result);
        }

        // Handle incompatible characters based on output encoding
        if self.output_encoding != AdifEncoding::Utf8 {
            result = self.handle_incompatible_characters(&result);
        }

        result
    }

    fn transliterate_to_ascii(&self, text: &str) -> String {
        use unicode_normalization::UnicodeNormalization;

        // Normalize to NFD (decomposed form) and remove combining characters
        text.nfd()
            .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
            .collect::<String>()
            .chars()
            .map(|c| {
                if c.is_ascii() {
                    c
                } else {
                    // Simple transliterations for common cases
                    match c {
                        'æ' | 'ǽ' => 'a',
                        'ð' => 'd',
                        'ø' => 'o',
                        'þ' => 'p',
                        'ß' => 's',
                        _ => self.replacement_char.unwrap_or('?'),
                    }
                }
            })
            .collect()
    }

    fn handle_incompatible_characters(&self, text: &str) -> String {
        let encoding = self.output_encoding.to_encoding_rs();

        text.chars()
            .filter_map(|c| {
                let char_str = c.to_string();
                let (_, _, had_errors) = encoding.encode(&char_str);

                if had_errors {
                    if self.delete_incompatible {
                        None // Remove the character
                    } else if let Some(replacement) = self.replacement_char {
                        Some(replacement)
                    } else {
                        // For now, just use '?' - entity references need special handling
                        Some('?')
                    }
                } else {
                    Some(c)
                }
            })
            .collect()
    }

    pub fn format_as_entity_reference(c: char) -> String {
        format!("&0x{:X};", c as u32)
    }
}

pub struct DebugFormatter;

impl DebugFormatter {
    pub fn print_qso_debug(adif: &AdifFile, qso_indices: &[usize]) {
        use crate::encoding::EncodingProcessor;
        for &index in qso_indices {
            if let Some(record) = adif.records.get(index) {
                println!("=== QSO {} ===", index + 1);

                for field in &record.fields {
                    println!("Field: {}", field.name);
                    println!("  Length: {} (original)", field.length);
                    println!("  Type: {:?}", field.field_type);
                    println!("  Data (original): {:?}", field.data);
                    println!("  Data (bytes): {:?}", field.original_bytes);
                    println!("  Excess: {:?}", field.excess_data);

                    // Try to show what the corrected data would be
                    let processor = EncodingProcessor::new(None, AdifEncoding::Utf8, false);
                    if let Ok(processed) = processor.process_field_data(&field.original_bytes) {
                        println!("  Processed: {:?}", processed);
                        if processed != field.data {
                            println!("  ** Data was corrected **");
                        }
                    }
                    println!();
                }

                if !record.excess_data.is_empty() {
                    println!("Record excess data: {:?}", record.excess_data);
                }
                println!();
            } else {
                println!("QSO {} not found (file has {} QSOs)", index + 1, adif.records.len());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adif::Field;

    #[test]
    fn test_ascii_transliteration() {
        let formatter = OutputFormatter::new(
            None,
            AdifEncoding::Ascii,
            false,
            Some('?'),
            false,
            true,
        );

        let text = "José Müller";
        let result = formatter.transliterate_to_ascii(text);
        // Should convert accented characters to base forms
        assert!(result.chars().all(|c| c.is_ascii()));
    }

    #[test]
    fn test_entity_reference_formatting() {
        let entity = OutputFormatter::format_as_entity_reference('€');
        assert_eq!(entity, "&0x20AC;");
    }
}