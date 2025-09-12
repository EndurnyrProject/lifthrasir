use encoding_rs::EUC_KR;
use nom::{IResult, bytes::complete::take};

pub fn parse_korean_string(input: &[u8], length: usize) -> IResult<&[u8], String> {
    let (input, bytes) = take(length)(input)?;
    let end_pos = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());

    let string_bytes = &bytes[..end_pos];
    let (decoded, _, _) = EUC_KR.decode(string_bytes);
    let filename = decoded.into_owned();

    Ok((input, filename))
}
