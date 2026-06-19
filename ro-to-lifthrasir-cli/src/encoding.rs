pub fn decode_euckr(bytes: &[u8]) -> String {
    encoding_rs::EUC_KR.decode(bytes).0.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_korean() {
        let (enc, _, _) = encoding_rs::EUC_KR.encode("초보자");
        assert_eq!(decode_euckr(&enc), "초보자");
    }

    #[test]
    fn passes_through_ascii() {
        assert_eq!(decode_euckr(b"Novice"), "Novice");
    }
}
