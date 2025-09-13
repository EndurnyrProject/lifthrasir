use crate::ro_formats::des;
use crate::utils::string_utils::parse_korean_string;
use bevy::prelude::info;
use flate2::read::ZlibDecoder;
use nom::{IResult, Parser, bytes::complete::take, number::complete::le_u32};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GrfError {
    #[error("Invalid GRF signature: {0}")]
    InvalidSignature(String),
    #[error("Unsupported GRF version: 0x{version:x}")]
    UnsupportedVersion { version: u32 },
    #[error("File table offset out of bounds: {offset}")]
    InvalidTableOffset { offset: u32 },
    #[error("Decompression failed: {0}")]
    DecompressionError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("IO error")]
    IoErrorStd(#[from] std::io::Error),
}

#[derive(Debug)]
pub struct GrfHeader {
    pub signature: [u8; 15],
    // pub key: [u8; 15],
    pub file_table_offset: u32,
    pub skip: u32,
    pub file_count: u32,
    pub version: u32,
}

#[derive(Debug, Clone)]
pub struct GrfTable {
    pub pack_size: u32,
    pub real_size: u32,
}

#[derive(Debug, Clone)]
pub struct GrfEntry {
    pub filename: String,
    pub pack_size: u32,
    pub length_aligned: u32,
    pub real_size: u32,
    pub file_type: u8,
    pub offset: u32,
}

#[derive(Debug)]
pub struct GrfFile {
    pub entries: Vec<GrfEntry>,
    pub entry_map: HashMap<String, usize>,
    file_path: PathBuf,
}

// File type constants from roBrowser
const FILELIST_TYPE_FILE: u8 = 0x01;
const FILELIST_TYPE_ENCRYPT_MIXED: u8 = 0x02;
const FILELIST_TYPE_ENCRYPT_HEADER: u8 = 0x04;

// GRF constants
const GRF_SIGNATURE: &str = "Master of Magic";
const SUPPORTED_VERSION: u32 = 0x200;
const HEADER_SIZE: u64 = 46;

impl GrfFile {
    fn validate_header(header: &GrfHeader, data_len: usize) -> Result<(), GrfError> {
        // Validate signature
        let signature_str = String::from_utf8_lossy(&header.signature);
        if signature_str.trim_end_matches('\0') != GRF_SIGNATURE {
            return Err(GrfError::InvalidSignature(signature_str.to_string()));
        }

        // Validate version
        if header.version != SUPPORTED_VERSION {
            return Err(GrfError::UnsupportedVersion {
                version: header.version,
            });
        }

        // Validate file table offset
        if header.file_table_offset as usize + HEADER_SIZE as usize > data_len {
            return Err(GrfError::InvalidTableOffset {
                offset: header.file_table_offset,
            });
        }

        Ok(())
    }

    pub fn from_path(path: PathBuf) -> Result<Self, GrfError> {
        let mut file = File::open(&path).map_err(|e| GrfError::IoError(e.to_string()))?;
        let mut header_bytes = vec![0u8; 46];

        file.read_exact(&mut header_bytes)
            .map_err(|e| GrfError::IoError(e.to_string()))?;

        let header = Self::parse_header(&header_bytes)?;

        let metadata = std::fs::metadata(&path).map_err(|e| GrfError::IoError(e.to_string()))?;
        Self::validate_header(&header, metadata.len() as usize)?;

        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(
            header.file_table_offset as u64 + HEADER_SIZE,
        ))
        .map_err(|e| GrfError::IoError(e.to_string()))?;

        // Calculate file table size (rest of the file after header)
        let file_table_size = metadata.len() - (header.file_table_offset as u64 + HEADER_SIZE);

        let mut file_table_data = vec![0u8; file_table_size as usize];
        file.read_exact(&mut file_table_data)
            .map_err(|e| GrfError::IoError(e.to_string()))?;

        // Decompress the file table first
        let decompressed_table = Self::decompress_file_table(&file_table_data)?;

        // Parse file table and entries
        let entries = Self::parse_entries(
            &decompressed_table,
            header.file_count.saturating_sub(header.skip + 7),
        )?;

        // Create filename -> index mapping for fast lookups
        let mut entry_map = HashMap::new();
        for (index, entry) in entries.iter().enumerate() {
            entry_map.insert(entry.filename.clone(), index);
        }

        Ok(GrfFile {
            entries,
            entry_map,
            file_path: path,
        })
    }

    fn parse_header(data: &[u8]) -> Result<GrfHeader, GrfError> {
        if data.len() < 46 {
            return Err(GrfError::ParseError(
                "File too small to contain valid GRF header".to_string(),
            ));
        }

        let (_, header) = parse_grf_header(data)
            .map_err(|e| GrfError::ParseError(format!("Header parsing failed: {e:?}")))?;

        Ok(header)
    }

    fn decompress_file_table(file_table_data: &[u8]) -> Result<Vec<u8>, GrfError> {
        // Parse table info (8 bytes: pack_size + real_size)
        let (remaining, table) = parse_grf_table(file_table_data)
            .map_err(|e| GrfError::ParseError(format!("Table parsing failed: {e:?}")))?;

        // Read compressed table data
        if remaining.len() < table.pack_size as usize {
            return Err(GrfError::ParseError(
                "Compressed table data incomplete".to_string(),
            ));
        }

        let compressed_data = &remaining[..table.pack_size as usize];

        // Decompress table data using zlib
        let mut decoder = ZlibDecoder::new(compressed_data);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| GrfError::DecompressionError(e.to_string()))?;

        // Verify decompressed size
        if decompressed.len() != table.real_size as usize {
            return Err(GrfError::DecompressionError(format!(
                "Decompressed size mismatch: expected {}, got {}",
                table.real_size,
                decompressed.len()
            )));
        }

        Ok(decompressed)
    }

    fn parse_entries(data: &[u8], count: u32) -> Result<Vec<GrfEntry>, GrfError> {
        let mut entries = Vec::with_capacity(count as usize);
        let mut pos = 0;

        for _ in 0..count {
            if pos >= data.len() {
                break;
            }

            let mut filename_bytes = Vec::new();
            while pos < data.len() && data[pos] != 0 {
                filename_bytes.push(data[pos]);
                pos += 1;
            }

            let filename = parse_korean_string(&filename_bytes, filename_bytes.len())
                .map_err(|e| GrfError::ParseError(format!("Filename parse error: {e:?}")))?
                .1;

            if pos >= data.len() {
                break;
            }
            pos += 1;

            // Ensure we have enough bytes for the entry data (17 bytes)
            if pos + 17 > data.len() {
                break;
            }

            // Read entry data (little-endian format)
            let pack_size =
                u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let length_aligned =
                u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);
            let real_size =
                u32::from_le_bytes([data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11]]);
            let file_type = data[pos + 12];
            let offset = u32::from_le_bytes([
                data[pos + 13],
                data[pos + 14],
                data[pos + 15],
                data[pos + 16],
            ]);
            pos += 17;

            // Log water-related files during GRF parsing
            if filename.to_lowercase().contains("water")
                || filename.contains("물")
                || filename.contains("워터")
            {
                info!("Found water-related file in GRF: {}", filename);
            }

            entries.push(GrfEntry {
                filename,
                pack_size,
                length_aligned,
                real_size,
                file_type,
                offset,
            });
        }

        Ok(entries)
    }

    pub fn get_file(&self, filename: &str) -> Option<Vec<u8>> {
        let entry_index = if let Some(&index) = self.entry_map.get(filename) {
            index
        } else {
            self.entries
                .iter()
                .position(|entry| entry.filename.eq_ignore_ascii_case(filename))?
        };
        let entry = &self.entries[entry_index];

        // Check if it's actually a file (not a directory)
        if (entry.file_type & FILELIST_TYPE_FILE) == 0 {
            return None;
        }

        // Open the GRF file and seek to the file's location
        let mut file = File::open(&self.file_path).ok()?;

        // Calculate absolute offset in the GRF file
        let absolute_offset = entry.offset as u64 + HEADER_SIZE;

        // Seek to the file location
        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(absolute_offset)).ok()?;

        // Read the compressed data
        let mut file_data = vec![0u8; entry.length_aligned as usize];
        file.read_exact(&mut file_data).ok()?;

        // Handle decryption if needed
        let was_encrypted = if entry.file_type & FILELIST_TYPE_ENCRYPT_MIXED != 0 {
            des::decode_full(&mut file_data, entry.length_aligned, entry.pack_size);
            true
        } else if entry.file_type & FILELIST_TYPE_ENCRYPT_HEADER != 0 {
            des::decode_header(&mut file_data, entry.length_aligned);
            true
        } else {
            false
        };

        // If file was encrypted OR is compressed, decompress it
        // Encrypted files are always compressed before encryption
        if was_encrypted || entry.real_size != entry.pack_size {
            let mut decoder = ZlibDecoder::new(&file_data[..]);
            let mut decompressed = Vec::new();
            match decoder.read_to_end(&mut decompressed) {
                Ok(_) => Some(decompressed),
                Err(_) => None,
            }
        } else {
            Some(file_data)
        }
    }
}

fn parse_grf_header(input: &[u8]) -> IResult<&[u8], GrfHeader> {
    let (input, (signature, key, file_table_offset, skip, file_count, version)) = (
        take(15usize), // signature
        take(15usize), // key
        le_u32,        // file_table_offset
        le_u32,        // skip
        le_u32,        // file_count
        le_u32,        // version
    )
        .parse(input)?;

    let mut sig_array = [0u8; 15];
    let mut key_array = [0u8; 15];
    sig_array.copy_from_slice(signature);
    key_array.copy_from_slice(key);

    Ok((
        input,
        GrfHeader {
            signature: sig_array,
            // key: key_array,
            file_table_offset,
            skip,
            file_count,
            version,
        },
    ))
}

fn parse_grf_table(input: &[u8]) -> IResult<&[u8], GrfTable> {
    let (input, (pack_size, real_size)) = (le_u32, le_u32).parse(input)?;

    Ok((
        input,
        GrfTable {
            pack_size,
            real_size,
        },
    ))
}
