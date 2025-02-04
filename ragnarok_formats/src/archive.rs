use ragnarok_bytes::{ByteConvertable, FixedByteSize};

use crate::signature::Signature;

/// Represents the Header of the GRF file.
#[derive(Clone, ByteConvertable, FixedByteSize)]
pub struct Header {
    pub signature: Signature<b"Master of Magic\0">,
    pub encryption: [u8; 14],
    pub file_table_offset: u32,
    pub reserved_files: u32,
    pub file_count: u32,
    pub version: u32,
}

impl Header {
    // TODO: This is temporary and can be removed after improving the
    // ByteConvertable trait.
    pub fn new(file_table_offset: u32, reserved_files: u32, file_count: u32, version: u32) -> Self {
        Self {
            signature: Signature,
            encryption: Default::default(),
            file_table_offset,
            reserved_files,
            file_count,
            version,
        }
    }
}

impl Header {
    pub const FILE_OFFSET: usize = 7;

    pub fn get_file_count(&self) -> usize {
        (self.file_count - self.reserved_files) as usize - Self::FILE_OFFSET
    }
}

/// Represents file information about each of the files stored in the GRF.
#[derive(Clone, Debug, ByteConvertable)]
pub struct FileTableRow {
    pub file_name: String,
    pub compressed_size: u32,
    pub compressed_size_aligned: u32,
    pub uncompressed_size: u32,
    pub flags: u8,
    pub offset: u32,
}

/// Stores the table of files the parent GRF is holding.
#[derive(Clone, ByteConvertable, FixedByteSize)]
pub struct AssetTable {
    pub compressed_size: u32,
    pub uncompressed_size: u32,
}
