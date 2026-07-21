use crate::des;
use crate::string_utils::parse_korean_string;
use flate2::read::ZlibDecoder;
use nom::{IResult, Parser, number::complete::le_u32};
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
    InvalidTableOffset { offset: u64 },
    #[error("Decompression failed: {0}")]
    DecompressionError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("IO error")]
    IoErrorStd(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GrfVersion {
    V200,
    V300,
}

impl GrfVersion {
    fn from_raw(version: u32) -> Result<Self, GrfError> {
        match version {
            0x200 => Ok(GrfVersion::V200),
            0x300 => Ok(GrfVersion::V300),
            _ => Err(GrfError::UnsupportedVersion { version }),
        }
    }
}

#[derive(Debug)]
pub struct GrfHeader {
    pub signature: [u8; 15],
    pub file_table_offset: u64,
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
    pub offset: u64,
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
const GRF_SIGNATURES: [&str; 2] = ["Master of Magic", "Event Horizon"];
const HEADER_SIZE: u64 = 46;

impl GrfFile {
    fn validate_header(header: &GrfHeader, data_len: usize) -> Result<(), GrfError> {
        // The version is validated while parsing the header. Branded clients ship
        // GRFs with a custom magic (e.g. "Event Horizon"), so the signature is
        // matched against a known allowlist by prefix.
        let signature_str = String::from_utf8_lossy(&header.signature);
        let signature = signature_str.trim_end_matches('\0');
        if !GRF_SIGNATURES
            .iter()
            .any(|known| signature.starts_with(known))
        {
            return Err(GrfError::InvalidSignature(signature_str.to_string()));
        }

        if header.file_table_offset + HEADER_SIZE > data_len as u64 {
            return Err(GrfError::InvalidTableOffset {
                offset: header.file_table_offset,
            });
        }

        Ok(())
    }

    fn real_file_count(header: &GrfHeader, version: GrfVersion) -> u32 {
        match version {
            GrfVersion::V300 => header.file_count,
            GrfVersion::V200 => header.file_count.saturating_sub(header.skip + 7),
        }
    }

    pub fn from_path(path: PathBuf) -> Result<Self, GrfError> {
        let mut file = File::open(&path).map_err(|e| GrfError::IoError(e.to_string()))?;
        let mut header_bytes = vec![0u8; 46];

        file.read_exact(&mut header_bytes)
            .map_err(|e| GrfError::IoError(e.to_string()))?;

        let (header, version) = Self::parse_header(&header_bytes)?;

        let metadata = std::fs::metadata(&path).map_err(|e| GrfError::IoError(e.to_string()))?;
        Self::validate_header(&header, metadata.len() as usize)?;

        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(
            header.file_table_offset + HEADER_SIZE,
        ))
        .map_err(|e| GrfError::IoError(e.to_string()))?;

        // Calculate file table size (rest of the file after header)
        let file_table_size = metadata.len() - (header.file_table_offset + HEADER_SIZE);

        let mut file_table_data = vec![0u8; file_table_size as usize];
        file.read_exact(&mut file_table_data)
            .map_err(|e| GrfError::IoError(e.to_string()))?;

        // Decompress the file table first
        let decompressed_table = Self::decompress_file_table(&file_table_data, version)?;

        // Parse file table and entries
        let entries = Self::parse_entries(
            &decompressed_table,
            Self::real_file_count(&header, version),
            version,
        )?;

        // Create filename -> index mapping for fast lookups. Keys are
        // ASCII-lowercased so lookups are case-insensitive: GND/RSW/RSM assets
        // often declare paths in a different case than the GRF stores them.
        let mut entry_map = HashMap::new();
        for (index, entry) in entries.iter().enumerate() {
            entry_map.insert(entry.filename.to_ascii_lowercase(), index);
        }

        Ok(GrfFile {
            entries,
            entry_map,
            file_path: path,
        })
    }

    fn parse_header(data: &[u8]) -> Result<(GrfHeader, GrfVersion), GrfError> {
        if data.len() < 46 {
            return Err(GrfError::ParseError(
                "File too small to contain valid GRF header".to_string(),
            ));
        }

        let raw_version = u32::from_le_bytes([data[42], data[43], data[44], data[45]]);
        let version = GrfVersion::from_raw(raw_version)?;

        let mut signature = [0u8; 15];
        signature.copy_from_slice(&data[0..15]);

        // v0x300 widens the file table offset to a little-endian i64 spanning the
        // old offset and seed fields (bytes 30..38); the seed concept is dropped.
        let (file_table_offset, skip) = match version {
            GrfVersion::V300 => (u64::from_le_bytes(data[30..38].try_into().unwrap()), 0),
            GrfVersion::V200 => (
                u32::from_le_bytes(data[30..34].try_into().unwrap()) as u64,
                u32::from_le_bytes(data[34..38].try_into().unwrap()),
            ),
        };

        let file_count = u32::from_le_bytes(data[38..42].try_into().unwrap());

        let header = GrfHeader {
            signature,
            file_table_offset,
            skip,
            file_count,
            version: raw_version,
        };

        Ok((header, version))
    }

    fn decompress_file_table(
        file_table_data: &[u8],
        version: GrfVersion,
    ) -> Result<Vec<u8>, GrfError> {
        // v0x300 prefixes the size pair with a 4-byte field (always 0).
        let preamble = if version == GrfVersion::V300 { 4 } else { 0 };
        if file_table_data.len() < preamble {
            return Err(GrfError::ParseError(
                "File table truncated before size header".to_string(),
            ));
        }
        let file_table_data = &file_table_data[preamble..];

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

    fn parse_entries(
        data: &[u8],
        count: u32,
        version: GrfVersion,
    ) -> Result<Vec<GrfEntry>, GrfError> {
        // v0x300 widens the per-entry offset to an i64, growing the fixed tail
        // from 17 to 21 bytes.
        let entry_tail = match version {
            GrfVersion::V300 => 21,
            GrfVersion::V200 => 17,
        };

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

            if pos + entry_tail > data.len() {
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
            let offset = match version {
                GrfVersion::V300 => {
                    u64::from_le_bytes(data[pos + 13..pos + 21].try_into().unwrap())
                }
                GrfVersion::V200 => u32::from_le_bytes([
                    data[pos + 13],
                    data[pos + 14],
                    data[pos + 15],
                    data[pos + 16],
                ]) as u64,
            };
            pos += entry_tail;

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
        let entry_index = *self.entry_map.get(&filename.to_ascii_lowercase())?;
        let entry = &self.entries[entry_index];

        // Check if it's actually a file (not a directory)
        if (entry.file_type & FILELIST_TYPE_FILE) == 0 {
            return None;
        }

        // Open the GRF file and seek to the file's location
        let mut file = File::open(&self.file_path).ok()?;

        // Calculate absolute offset in the GRF file
        let absolute_offset = entry.offset + HEADER_SIZE;

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

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    /// Builds a minimal, single-entry v0x300 GRF in memory: 46-byte header,
    /// the zlib payload right after the header, then the table section
    /// (4-byte skip + compressed/real sizes + zlib'd 21-byte entry).
    fn build_v300_grf(magic: &str, filename: &str, content: &[u8]) -> Vec<u8> {
        let payload = zlib(content);

        let mut entry = Vec::new();
        entry.extend_from_slice(filename.as_bytes());
        entry.push(0);
        entry.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        entry.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        entry.extend_from_slice(&(content.len() as u32).to_le_bytes());
        entry.push(FILELIST_TYPE_FILE);
        // payload sits at byte 46, so the stored offset (relative to the header) is 0.
        entry.extend_from_slice(&0i64.to_le_bytes());
        let table_comp = zlib(&entry);

        let mut buf = vec![0u8; HEADER_SIZE as usize];
        buf[0..magic.len()].copy_from_slice(magic.as_bytes());
        for i in 0..14 {
            buf[15 + i] = (i + 1) as u8;
        }
        buf.extend_from_slice(&payload);

        let file_table_offset = buf.len() as u64 - HEADER_SIZE;
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&(table_comp.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(entry.len() as u32).to_le_bytes());
        buf.extend_from_slice(&table_comp);

        buf[30..38].copy_from_slice(&file_table_offset.to_le_bytes());
        buf[38..42].copy_from_slice(&1u32.to_le_bytes());
        buf[42..46].copy_from_slice(&0x300u32.to_le_bytes());
        buf
    }

    fn roundtrip(magic: &str, name: &str, filename: &str, content: &[u8]) {
        let bytes = build_v300_grf(magic, filename, content);
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, &bytes).unwrap();

        let grf = GrfFile::from_path(path.clone()).unwrap();
        assert_eq!(grf.entries.len(), 1);
        assert_eq!(grf.entries[0].filename, filename);
        assert_eq!(grf.get_file(filename).as_deref(), Some(content));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn v300_roundtrip() {
        let content = b"hello v0x300 world, repeated content compresses well. ".repeat(16);
        roundtrip(
            "Master of Magic",
            "lifthrasir_grf_v300_roundtrip.grf",
            "data\\test.txt",
            &content,
        );
    }

    #[test]
    fn v300_accepts_branded_magic() {
        let content = b"branded private-server payload ".repeat(8);
        roundtrip(
            "Event Horizon",
            "lifthrasir_grf_v300_branded.grf",
            "data\\brand.txt",
            &content,
        );
    }

    #[test]
    fn rejects_unknown_version() {
        let mut bytes = build_v300_grf("Master of Magic", "x.txt", b"data");
        bytes[42..46].copy_from_slice(&0x999u32.to_le_bytes());
        let path = std::env::temp_dir().join("lifthrasir_grf_bad_version.grf");
        std::fs::write(&path, &bytes).unwrap();

        assert!(matches!(
            GrfFile::from_path(path.clone()),
            Err(GrfError::UnsupportedVersion { version: 0x999 })
        ));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_unknown_magic() {
        let mut bytes = build_v300_grf("Master of Magic", "x.txt", b"data");
        bytes[0..15].copy_from_slice(b"Not A Real GRF!");
        let path = std::env::temp_dir().join("lifthrasir_grf_bad_magic.grf");
        std::fs::write(&path, &bytes).unwrap();

        assert!(matches!(
            GrfFile::from_path(path.clone()),
            Err(GrfError::InvalidSignature(_))
        ));

        std::fs::remove_file(&path).ok();
    }
}
