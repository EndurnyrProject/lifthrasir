// TODO: Wire GameMode, card compounding, vending SP errors, guild alliances, and
// guild storage through the net-contract adapter boundary.
pub mod aesir {
    pub mod net {
        #![allow(clippy::all)]
        #![allow(missing_docs)]
        include!("aesir.net.rs");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn envelope_default_path() {
        let e = super::aesir::net::Envelope::default();
        assert_eq!(e.seq, 0);
        assert!(e.body.is_none());
    }
}
