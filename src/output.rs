use crate::adif::{AdifFile, AdifHeader, AdifRecord, AdifField};
use crate::encoding::{EncodingProcessor, OutputEncoding};
use crate::error::Result;
use std::io::Write;

pub struct OutputGenerator<'a> {
    processor: &'a EncodingProcessor,
}

impl<'a> OutputGenerator<'a> {
    pub fn new(processor: &'a EncodingProcessor) -> Self {
        Self { processor }
    }

    pub fn generate<W: Write>(&self, file: &AdifFile, writer: &mut W) -> Result<()> {
        // Write header if present
        if let Some(header) = &file.header {
            self.write_header(header, writer)?;
        }

        // Write records
        for record in &file.records {
            self.write_record(record, writer)?;
        }

        Ok(())
    }

    fn write_header<W: Write>(&self, header: &AdifHeader, writer: &mut W) -> Result<()> {
        // Write preamble
        writer.write_all(header.preamble.as_bytes())?;

        // Write encoding field first if not present
        let has_encoding_field = header.fields.iter()
            .any(|f| f.name.to_lowercase() == "encoding");

        if !has_encoding_field {
            self.write_encoding_field(writer)?;
            writer.write_all(b"\n")?; // Add newline after encoding field
        }

        // Write header fields
        for field in &header.fields {
            self.write_field(field, writer)?;
            writer.write_all(field.excess_data.as_bytes())?;
        }

        // Write end of header
        writer.write_all(b"<eoh>")?;
        writer.write_all(header.excess_data.as_bytes())?;

        Ok(())
    }

    fn write_record<W: Write>(&self, record: &AdifRecord, writer: &mut W) -> Result<()> {
        // Write record fields
        for field in &record.fields {
            self.write_field(field, writer)?;
            writer.write_all(field.excess_data.as_bytes())?;
        }

        // Write end of record
        writer.write_all(b"<eor>")?;
        writer.write_all(record.excess_data.as_bytes())?;

        Ok(())
    }

    fn write_field<W: Write>(&self, field: &AdifField, writer: &mut W) -> Result<()> {
        // Encode the field data for output
        let (encoded_data, char_count) = self.processor.encode_for_output(&field.data)?;

        // Write field header
        writer.write_all(b"<")?;
        writer.write_all(field.name.as_bytes())?;
        writer.write_all(b":")?;

        // Use the calculated length based on output encoding
        let length = match self.processor.options.output_encoding {
            OutputEncoding::Utf8 => char_count, // UTF-8 uses character count
            _ => encoded_data.len(), // Other encodings use byte count
        };

        writer.write_all(length.to_string().as_bytes())?;

        // Write field type if present
        if let Some(ref field_type) = field.field_type {
            writer.write_all(b":")?;
            writer.write_all(field_type.as_bytes())?;
        }

        writer.write_all(b">")?;

        // Write field data
        writer.write_all(&encoded_data)?;

        Ok(())
    }

    fn write_encoding_field<W: Write>(&self, writer: &mut W) -> Result<()> {
        let encoding_name = match &self.processor.options.output_encoding {
            OutputEncoding::Utf8 => "UTF-8",
            OutputEncoding::Ascii => "ASCII",
            OutputEncoding::CodePage(name) => name.as_str(),
        };

        let encoding_bytes = encoding_name.as_bytes();
        writer.write_all(b"<encoding:")?;
        writer.write_all(encoding_bytes.len().to_string().as_bytes())?;
        writer.write_all(b">")?;
        writer.write_all(encoding_bytes)?;

        Ok(())
    }
}

impl AdifFile {
    pub fn write_to<W: Write>(
        &self,
        writer: &mut W,
        processor: &EncodingProcessor,
    ) -> Result<()> {
        let generator = OutputGenerator::new(processor);
        generator.generate(self, writer)
    }
}
