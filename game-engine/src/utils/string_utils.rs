use encoding_rs::EUC_KR;
use nom::{bytes::complete::take, IResult};

pub fn parse_korean_string(input: &[u8], length: usize) -> IResult<&[u8], String> {
    let (input, bytes) = take(length)(input)?;
    let end_pos = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());

    let string_bytes = &bytes[..end_pos];
    let (decoded, _, _) = EUC_KR.decode(string_bytes);
    let filename = decoded.into_owned();

    Ok((input, filename))
}

/// Pack a string into a fixed-length null-terminated byte array
///
/// Used for encoding string fields in network packets.
/// Truncates if the string is too long, null-pads if too short.
///
/// # Arguments
///
/// * `s` - The string to pack
/// * `len` - The desired length of the output array
///
/// # Returns
///
/// A Vec<u8> of exactly `len` bytes, null-terminated
pub fn pack_string(s: &str, len: usize) -> Vec<u8> {
    let mut result = vec![0u8; len];
    let bytes = s.as_bytes();
    let copy_len = bytes.len().min(len - 1); // Reserve 1 byte for null terminator
    result[..copy_len].copy_from_slice(&bytes[..copy_len]);
    result
}
