// Copyright 2026 Nicolas Meylan. Apache-2.0; see LICENSE.
// Adapted for Lifthrasir: module paths and validation. See README.md.
//! Reader for the Granny (GR2) 3D asset format used by RO for WOE guardians,
//! the Emperium, guild flags, and treasure boxes. A file is a set of
//! Oodle-compressed sections (`oodle` module) that decompress into one buffer,
//! with pointer fix-ups rewritten to absolute offsets; [`model`] then walks the
//! type tree to extract skeletons, meshes, textures, and animations.
//!
//! Format references:
//! - <https://github.com/rdw-archive/RagnarokFileFormats/blob/master/GR2.MD>
//! - <https://github.com/arves100/Granny2-research/wiki/File-Format-Documentation>

mod bink;
pub mod model;
mod oodle;
mod range_coder;

pub use model::Gr2File;

#[derive(Debug)]
pub enum Gr2Error {
    InvalidMagic,
    UnsupportedVersion(u8, u8),
    UnexpectedEof,
    DecompressionFailed(String),
}

impl std::fmt::Display for Gr2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Gr2Error {}

const HEADER_SIZE: usize = 0x20;
const SECTOR_SIZE: usize = 44;
const FIXUP_SIZE: usize = 12;
const OODLE_TAIL_PAD: usize = 4;

// The 16-byte file signature (read as four little-endian words) also encodes
// the byte order and pointer size of the file; these two are the little-endian,
// 32-bit variants for format versions 6 and 7 respectively.
const MAGIC_FF6_LE: [u32; 4] = [0xCAB0_67B8, 0x0FB1_6DF8, 0x7E8C_7284, 0x1E00_195E];
const MAGIC_FF7_LE: [u32; 4] = [0xC06C_DE29, 0x2B53_A4BA, 0xA5B7_F525, 0xEEE2_66F6];

/// A `(sector, position)` reference into the decompressed data buffer. The GR2
/// header stores its root object and type as these pairs, and pointer fix-ups
/// resolve to them.
#[derive(Clone, Copy)]
pub struct SectorRef {
    pub sector: u32,
    pub position: u32,
}

#[derive(Clone, Copy)]
pub struct SectorInfo {
    pub compress_type: u32,
    pub data_offset: u32,
    pub compressed_len: u32,
    pub decompress_len: u32,
    pub oodle_stop0: u32,
    pub oodle_stop1: u32,
    pub fixup_offset: u32,
    pub fixup_count: u32,
}

/// A parsed GR2 file: all sectors decompressed into one contiguous buffer with
/// pointer fix-ups rewritten to absolute offsets within it.
pub struct Gr2Container {
    pub version: u32,
    pub data: Vec<u8>,
    pub sector_offsets: Vec<usize>,
    pub sectors: Vec<SectorInfo>,
    pub type_ref: SectorRef,
    pub root_ref: SectorRef,
}

/// Standard reflected CRC-32 (polynomial `0xEDB88320`), used to verify the file
/// body against the checksum stored in the header.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Read a little-endian `u32` at a byte offset. Shared by the container,
/// object-graph, and texture-header parsers, which all address the buffer by
/// absolute offset rather than reading sequentially.
pub(super) fn read_u32(data: &[u8], off: usize) -> Result<u32, Gr2Error> {
    let end = off.checked_add(4).ok_or(Gr2Error::UnexpectedEof)?;
    let bytes = data.get(off..end).ok_or(Gr2Error::UnexpectedEof)?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

impl Gr2Container {
    pub fn parse(bytes: &[u8]) -> Result<Self, Gr2Error> {
        if bytes.len() < HEADER_SIZE {
            return Err(Gr2Error::UnexpectedEof);
        }
        let magic = [
            read_u32(bytes, 0)?,
            read_u32(bytes, 4)?,
            read_u32(bytes, 8)?,
            read_u32(bytes, 12)?,
        ];
        if magic != MAGIC_FF6_LE && magic != MAGIC_FF7_LE {
            return Err(Gr2Error::InvalidMagic);
        }
        if read_u32(bytes, 0x18)? != 0 {
            return Err(Gr2Error::InvalidMagic);
        }

        let version = read_u32(bytes, 0x20)?;
        if version != 6 && version != 7 {
            return Err(Gr2Error::UnsupportedVersion(version as u8, 0));
        }
        let total_size = read_u32(bytes, 0x24)? as usize;
        let file_crc = read_u32(bytes, 0x28)?;
        let file_info_size = read_u32(bytes, 0x2c)? as usize;
        let sector_count = read_u32(bytes, 0x30)? as usize;
        let type_ref = SectorRef {
            sector: read_u32(bytes, 0x34)?,
            position: read_u32(bytes, 0x38)?,
        };
        let root_ref = SectorRef {
            sector: read_u32(bytes, 0x3c)?,
            position: read_u32(bytes, 0x40)?,
        };

        if total_size != bytes.len() {
            return Err(Gr2Error::DecompressionFailed("gr2: size mismatch".into()));
        }

        let crc_start = HEADER_SIZE + file_info_size;
        let crc = crc32(bytes.get(crc_start..).ok_or(Gr2Error::UnexpectedEof)?);
        if crc != file_crc {
            return Err(Gr2Error::DecompressionFailed("gr2: crc mismatch".into()));
        }

        let sector_table = HEADER_SIZE + file_info_size;
        if sector_count == 0
            || sector_count > bytes.len().saturating_sub(sector_table) / SECTOR_SIZE
        {
            return Err(Gr2Error::UnexpectedEof);
        }
        let mut sectors = Vec::with_capacity(sector_count);
        for i in 0..sector_count {
            let base = sector_table + i * SECTOR_SIZE;
            sectors.push(SectorInfo {
                compress_type: read_u32(bytes, base)?,
                data_offset: read_u32(bytes, base + 4)?,
                compressed_len: read_u32(bytes, base + 8)?,
                decompress_len: read_u32(bytes, base + 12)?,
                oodle_stop0: read_u32(bytes, base + 20)?,
                oodle_stop1: read_u32(bytes, base + 24)?,
                fixup_offset: read_u32(bytes, base + 28)?,
                fixup_count: read_u32(bytes, base + 32)?,
            });
        }

        for reference in [type_ref, root_ref] {
            if sectors
                .get(reference.sector as usize)
                .is_none_or(|s| reference.position >= s.decompress_len)
            {
                return Err(Gr2Error::DecompressionFailed(
                    "gr2: invalid root reference".into(),
                ));
            }
        }
        let total: u64 = sectors.iter().map(|s| u64::from(s.decompress_len)).sum();
        // Bound allocations for RO actors rather than accepting arbitrary Granny packages.
        if total > 64 * 1024 * 1024 {
            return Err(Gr2Error::DecompressionFailed(
                "gr2: decoded data exceeds 64 MiB".into(),
            ));
        }
        let mut data = vec![0u8; total as usize];
        let mut sector_offsets = Vec::with_capacity(sector_count);
        let mut ofs = 0usize;

        // Decompress each sector into its slice of the contiguous output buffer.
        for s in &sectors {
            sector_offsets.push(ofs);
            let dst = &mut data[ofs..ofs + s.decompress_len as usize];
            let src_start = s.data_offset as usize;
            if s.compress_type > 1 {
                return Err(Gr2Error::DecompressionFailed(format!(
                    "gr2: unsupported compression {}",
                    s.compress_type
                )));
            }
            if s.compress_type == 0 {
                if s.compressed_len != s.decompress_len {
                    return Err(Gr2Error::DecompressionFailed(
                        "gr2: invalid uncompressed sector length".into(),
                    ));
                }
                let src = bytes
                    .get(src_start..src_start + s.decompress_len as usize)
                    .ok_or(Gr2Error::UnexpectedEof)?;
                dst.copy_from_slice(src);
            } else {
                let src = bytes
                    .get(src_start..src_start + s.compressed_len as usize)
                    .ok_or(Gr2Error::UnexpectedEof)?;
                // The decoder may read a few bytes past the compressed input, so
                // pad the tail with zeros (which decode as a graceful stop).
                let mut compressed = src.to_vec();
                compressed.resize(src.len() + OODLE_TAIL_PAD, 0);
                oodle::decompress(&compressed, dst, s.oodle_stop0, s.oodle_stop1)?;
            }
            ofs += s.decompress_len as usize;
        }

        apply_fixups(bytes, &mut data, &sector_offsets, &sectors)?;

        Ok(Gr2Container {
            version,
            data,
            sector_offsets,
            sectors,
            type_ref,
            root_ref,
        })
    }

    /// Absolute offset of a sector reference within `data`.
    pub fn ref_offset(&self, r: SectorRef) -> Result<usize, Gr2Error> {
        let index = r.sector as usize;
        let sector = self.sectors.get(index).ok_or(Gr2Error::UnexpectedEof)?;
        if r.position >= sector.decompress_len {
            return Err(Gr2Error::UnexpectedEof);
        }
        let start = self
            .sector_offsets
            .get(index)
            .ok_or(Gr2Error::UnexpectedEof)?;
        Ok(start + r.position as usize)
    }
}

fn apply_fixups(
    bytes: &[u8],
    data: &mut [u8],
    sector_offsets: &[usize],
    sectors: &[SectorInfo],
) -> Result<(), Gr2Error> {
    for (i, s) in sectors.iter().enumerate() {
        for k in 0..s.fixup_count as usize {
            let base = s.fixup_offset as usize + k * FIXUP_SIZE;
            let src_offset = read_u32(bytes, base)? as usize;
            let dst_sector = read_u32(bytes, base + 4)? as usize;
            let dst_offset = read_u32(bytes, base + 8)? as usize;
            if dst_sector >= sector_offsets.len()
                || dst_offset >= sectors[dst_sector].decompress_len as usize
                || src_offset
                    .checked_add(4)
                    .is_none_or(|end| end > s.decompress_len as usize)
            {
                return Err(Gr2Error::DecompressionFailed(
                    "gr2: bad fixup sector".into(),
                ));
            }
            let target = (sector_offsets[dst_sector] + dst_offset) as u32;
            let at = sector_offsets[i] + src_offset;
            data.get_mut(at..at + 4)
                .ok_or(Gr2Error::UnexpectedEof)?
                .copy_from_slice(&target.to_le_bytes());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn container() -> Vec<u8> {
        let mut bytes = vec![0_u8; 0x58 + SECTOR_SIZE + 4];
        for (i, word) in MAGIC_FF6_LE.iter().enumerate() {
            put(&mut bytes, i * 4, *word);
        }
        put(&mut bytes, 0x20, 6);
        put(&mut bytes, 0x2c, 0x38);
        put(&mut bytes, 0x30, 1);
        put(&mut bytes, 0x58 + 4, (0x58 + SECTOR_SIZE) as u32);
        put(&mut bytes, 0x58 + 8, 4);
        put(&mut bytes, 0x58 + 12, 4);
        seal(&mut bytes);
        bytes
    }

    fn seal(bytes: &mut [u8]) {
        put(bytes, 0x24, bytes.len() as u32);
        put(bytes, 0x28, crc32(&bytes[0x58..]));
    }

    #[test]
    fn rejects_out_of_sector_root_references() {
        let mut bytes = container();
        put(&mut bytes, 0x3c, 1);
        assert!(Gr2Container::parse(&bytes).is_err());
        put(&mut bytes, 0x3c, 0);
        put(&mut bytes, 0x40, 4);
        assert!(Gr2Container::parse(&bytes).is_err());
    }

    #[test]
    fn rejects_truncated_header_and_invalid_checksum() {
        for bytes in [vec![], vec![0_u8; 31], container()[..40].to_vec()] {
            assert!(Gr2Container::parse(&bytes).is_err());
        }
        let mut bytes = container();
        *bytes.last_mut().unwrap() = 1;
        assert!(Gr2Container::parse(&bytes).is_err());
    }

    #[test]
    fn rejects_out_of_sector_fixup_destination() {
        let mut bytes = container();
        let fixup = bytes.len();
        bytes.extend_from_slice(&[0; FIXUP_SIZE]);
        put(&mut bytes, 0x58 + 28, fixup as u32);
        put(&mut bytes, 0x58 + 32, 1);
        put(&mut bytes, fixup + 8, 4);
        seal(&mut bytes);
        assert!(Gr2Container::parse(&bytes).is_err());
    }
}
