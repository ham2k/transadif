use crate::adif::{AdifFile, AdifField};
use crate::encoding::{EncodingDetector, EncodingOptions};
use crate::error::Result;

pub fn generate_adif(adif_file: &mut AdifFile, opts: &EncodingOptions) -> Result<Vec<u8>> {
    let encoding_detector = EncodingDetector::new();

    // Only set encoding in output if the original had header fields or if encoding changed
    let output_encoding = if opts.output_encoding.to_lowercase() == "utf-8" {
        "UTF-8".to_string()
    } else {
        opts.output_encoding.clone()
    };

    // Check if the original file had any header fields and specifically an encoding field
    let has_existing_header_fields = !adif_file.header.fields.is_empty();
    let has_existing_encoding_field = adif_file.header.fields.iter()
        .any(|field| field.name.to_lowercase() == "encoding");

    // Add encoding field if there are header fields
    if has_existing_encoding_field || has_existing_header_fields {
        adif_file.set_encoding(&output_encoding);
    }

    // Add/update program ID if there were already header fields
    if has_existing_header_fields {
        adif_file.set_program_id();
    }

    // Generate the output string
    let mut output = String::new();

    // Write header
    write_header(&mut output, &adif_file.header, opts)?;

    // Write records
    for record in &adif_file.records {
        write_record(&mut output, record, opts)?;
    }

    // Encode to target encoding
    encoding_detector.encode_from_unicode(&output, &opts.output_encoding, opts)
}

fn write_header(output: &mut String, header: &crate::adif::AdifHeader, opts: &EncodingOptions) -> Result<()> {
    // Write preamble if present
    if !header.preamble.trim().is_empty() {
        output.push_str(&header.preamble);
        if !header.preamble.ends_with('\n') {
            output.push('\n');
        }
    }

    // Write header fields on separate lines
    for field in &header.fields {
        write_field(output, field, opts)?;
        output.push('\n');
    }

    // Write header excess data
    if !header.excess_data.trim().is_empty() {
        output.push_str(&header.excess_data);
    }

    // Write end of header
    output.push_str("<eoh>");

    Ok(())
}

fn write_record(output: &mut String, record: &crate::adif::AdifRecord, opts: &EncodingOptions) -> Result<()> {
    // Write record fields on separate lines
    for field in &record.fields {
        output.push('\n');
        write_field(output, field, opts)?;
    }

    // Write record excess data
    if !record.excess_data.trim().is_empty() {
        output.push_str(&record.excess_data);
    }

    // Write end of record
    output.push('\n');
    output.push_str("<eor>");
    output.push('\n');

    Ok(())
}

fn write_field(output: &mut String, field: &AdifField, opts: &EncodingOptions) -> Result<()> {
    let encoding_detector = EncodingDetector::new();

    // For UTF-8 output, use the field data and corrected length
    let (final_string, length) = if opts.output_encoding.to_lowercase() == "utf-8" {
        let string = field.data.clone();
        // Use the corrected length from the field (after mojibake correction)
        let char_count = field.length;
        (string, char_count)
    } else {
        // For non-UTF-8, process through encoding pipeline
        let processed_data = encoding_detector.encode_from_unicode(&field.data, &opts.output_encoding, opts)?;
        let processed_string = String::from_utf8_lossy(&processed_data).into_owned();
        let byte_count = processed_data.len();
        (processed_string, byte_count)
    };

    // Write field using original case
    let field_name = field.name.clone();

    if let Some(ref field_type) = field.field_type {
        output.push_str(&format!("<{}:{}:{}>{}",
            field_name, length, field_type, final_string));
    } else {
        output.push_str(&format!("<{}:{}>{}",
            field_name, length, final_string));
    }

    // Write field excess data
    if !field.excess_data.trim().is_empty() {
        output.push_str(&field.excess_data);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adif::{AdifHeader, AdifRecord};

    #[test]
    fn test_generate_simple_adif() {
        let mut adif_file = AdifFile {
            header: AdifHeader {
                preamble: "Test file\n".to_string(),
                fields: vec![],
                excess_data: String::new(),
            },
            records: vec![
                AdifRecord {
                    fields: vec![
                        AdifField {
                            name: "CALL".to_string(),
                            length: 5,
                            field_type: None,
                            data: "K1ABC".to_string(),
                            excess_data: String::new(),
                        },
                    ],
                    excess_data: String::new(),
                }
            ],
        };

        let opts = EncodingOptions {
            input_encoding: None,
            output_encoding: "utf-8".to_string(),
            transcode: false,
            replace_char: "?".to_string(),
            delete_incompatible: false,
            ascii_transliterate: false,
            strict_mode: false,
        };

        let result = generate_adif(&mut adif_file, &opts);
        assert!(result.is_ok());
    }
}

