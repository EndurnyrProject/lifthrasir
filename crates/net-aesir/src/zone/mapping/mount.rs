use crate::proto::aesir::net;
use net_contract::events::{PecoMountRejection, PecoMountResult};

pub fn mount_result(r: net::MountResult) -> PecoMountResult {
    let outcome = match net::MountResultCode::try_from(r.result) {
        Ok(net::MountResultCode::MountSkillNotLearned) => Err(PecoMountRejection::SkillNotLearned),
        Ok(net::MountResultCode::MountAlreadyMounted) => Err(PecoMountRejection::AlreadyMounted),
        Ok(net::MountResultCode::MountNotMounted) => Err(PecoMountRejection::NotMounted),
        Ok(net::MountResultCode::MountDead) => Err(PecoMountRejection::Dead),
        _ => Ok(()),
    };
    PecoMountResult { outcome }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_every_rejection_code() {
        let cases = [
            (net::MountResultCode::MountOk, Ok(())),
            (
                net::MountResultCode::MountSkillNotLearned,
                Err(PecoMountRejection::SkillNotLearned),
            ),
            (
                net::MountResultCode::MountAlreadyMounted,
                Err(PecoMountRejection::AlreadyMounted),
            ),
            (
                net::MountResultCode::MountNotMounted,
                Err(PecoMountRejection::NotMounted),
            ),
            (
                net::MountResultCode::MountDead,
                Err(PecoMountRejection::Dead),
            ),
        ];
        for (code, expected) in cases {
            let mapped = mount_result(net::MountResult {
                result: code as i32,
            });
            assert_eq!(mapped.outcome, expected);
        }
    }

    #[test]
    fn unknown_code_maps_to_ok() {
        assert_eq!(
            mount_result(net::MountResult { result: 999 }).outcome,
            Ok(())
        );
    }
}
