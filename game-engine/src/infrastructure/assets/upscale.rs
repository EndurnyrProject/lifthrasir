use crate::domain::settings::resources::Upscaling;

/// Sources already this large per side are left untouched.
const SKIP_THRESHOLD: u32 = 1024;
/// The largest output side the memory guard allows.
const MAX_OUTPUT_SIDE: u32 = 2048;

/// Upscale an RGBA buffer by the `Upscaling` factor via xBRZ.
///
/// `Off`, oversized sources, and guard-clamped factors of `1` return the input
/// unchanged. Otherwise the effective factor is reduced so no output side
/// exceeds [`MAX_OUTPUT_SIDE`].
pub fn scale(rgba: &[u8], width: u32, height: u32, upscaling: Upscaling) -> (Vec<u8>, u32, u32) {
    let factor = match upscaling.factor() {
        None => return (rgba.to_vec(), width, height),
        Some(f) => f as u32,
    };

    let max_side = width.max(height);
    if max_side >= SKIP_THRESHOLD {
        return (rgba.to_vec(), width, height);
    }

    let eff = factor.min(MAX_OUTPUT_SIDE / max_side.max(1));
    if eff <= 1 {
        return (rgba.to_vec(), width, height);
    }

    let scaled = xbrz::scale_rgba(rgba, width as usize, height as usize, eff as usize);
    (scaled, width * eff, height * eff)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(width: u32, height: u32) -> Vec<u8> {
        vec![0u8; (width * height * 4) as usize]
    }

    #[test]
    fn off_is_identity() {
        let src = buffer(4, 4);
        let (out, w, h) = scale(&src, 4, 4, Upscaling::Off);
        assert_eq!(out, src);
        assert_eq!((w, h), (4, 4));
    }

    #[test]
    fn x2_doubles_dimensions() {
        let src = buffer(4, 4);
        let (out, w, h) = scale(&src, 4, 4, Upscaling::X2);
        assert_eq!((w, h), (8, 8));
        assert_eq!(out.len(), (8 * 8 * 4) as usize);
    }

    #[test]
    fn one_by_one_does_not_panic() {
        let src = buffer(1, 1);
        for upscaling in Upscaling::ALL {
            let _ = scale(&src, 1, 1, upscaling);
        }
    }

    #[test]
    fn large_source_is_skipped() {
        let src = buffer(1024, 1);
        let (out, w, h) = scale(&src, 1024, 1, Upscaling::X4);
        assert_eq!(out, src);
        assert_eq!((w, h), (1024, 1));
    }

    #[test]
    fn factor_is_clamped_to_output_budget() {
        let src = buffer(600, 1);
        let (out, w, h) = scale(&src, 600, 1, Upscaling::X4);
        assert_eq!((w, h), (1800, 3));
        assert_eq!(out.len(), (1800 * 3 * 4) as usize);
    }
}
