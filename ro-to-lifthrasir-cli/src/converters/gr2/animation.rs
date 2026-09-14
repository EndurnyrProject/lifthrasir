//! Offline sampling of embedded and external RO GR2 animation clips.
//!
//! Curve evaluation adapted from Nicolas Meylan's nostalro-client (Apache-2.0).
//! See ro-formats/src/gr2/LICENSE and README.md. Added validation and local-pose output;
//! removed runtime/world dependencies and implicit unsupported-curve fallbacks.

use anyhow::{Context, ensure};
use glam::{Mat3, Mat4, Quat, Vec3};
use ro_formats::gr2::{
    Gr2File,
    model::{Gr2Curve, Gr2Transform},
};

/// Local transforms in model-0 skeleton order, without model placement or axis correction.
pub(crate) fn sample_local_pose(
    file: &Gr2File,
    animation_file: &Gr2File,
    animation_index: usize,
    time_seconds: f32,
) -> anyhow::Result<Vec<Mat4>> {
    let model = file.models.first().context("GR2 file has no model")?;
    let skeleton = file
        .skeletons
        .get(model.skeleton_index.context("model has no skeleton")?)
        .context("invalid model skeleton reference")?;
    let clip = animation_file
        .animations
        .get(animation_index)
        .context("missing GR2 animation")?;
    ensure!(
        clip.duration.is_finite() && clip.duration > 0.0,
        "invalid animation duration"
    );
    ensure!(
        time_seconds.is_finite() && (0.0..=clip.duration).contains(&time_seconds),
        "sample outside animation duration"
    );
    let groups = clip
        .track_group_indices
        .iter()
        .map(|&index| {
            animation_file
                .track_groups
                .get(index)
                .context("invalid animation track group")
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    ensure!(
        groups
            .iter()
            .flat_map(|g| &g.transform_tracks)
            .any(|track| skeleton.bones.iter().any(|bone| bone.name == track.name)),
        "animation has no tracks matching the model skeleton"
    );
    skeleton
        .bones
        .iter()
        .enumerate()
        .map(|(index, bone)| {
            ensure!(
                bone.parent_index == -1 || (0..index as i32).contains(&bone.parent_index),
                "invalid parent for joint {}",
                bone.name
            );
            let mut tracks = groups
                .iter()
                .flat_map(|g| &g.transform_tracks)
                .filter(|t| t.name == bone.name);
            let track = tracks.next();
            ensure!(
                tracks.next().is_none(),
                "duplicate track for joint {}",
                bone.name
            );
            let mut transform = bone.transform;
            if let Some(track) = track {
                transform.position =
                    sample_curve(&track.position, time_seconds, transform.position)?;
                transform.rotation =
                    sample_curve(&track.orientation, time_seconds, transform.rotation)?;
                transform.scale_shear =
                    sample_curve(&track.scale_shear, time_seconds, transform.scale_shear)?;
            }
            transform_matrix(&transform)
                .with_context(|| format!("joint {} at {time_seconds}s", bone.name))
        })
        .collect()
}

pub(super) fn transform_matrix(transform: &Gr2Transform) -> anyhow::Result<Mat4> {
    let rotation = Quat::from_array(transform.rotation);
    ensure!(
        rotation.is_finite() && rotation.length_squared() > 1e-12,
        "invalid joint quaternion"
    );
    let matrix = Mat4::from_translation(Vec3::from_array(transform.position))
        * Mat4::from_mat3(
            Mat3::from_quat(rotation.normalize())
                * Mat3::from_cols_array(&transform.scale_shear).transpose(),
        );
    decompose(matrix)?;
    Ok(matrix)
}

/// Reject matrices that would change when written as glTF animation TRS values.
pub(super) fn decompose(matrix: Mat4) -> anyhow::Result<(Vec3, Quat, Vec3)> {
    ensure!(
        matrix.is_finite() && matrix.determinant().abs() > 1e-12,
        "nonfinite or singular joint transform"
    );
    let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
    let rotation = rotation.normalize();
    let restored = Mat4::from_scale_rotation_translation(scale, rotation, translation);
    ensure!(
        restored.is_finite() && matrix.abs_diff_eq(restored, 1e-4),
        "joint shear cannot be represented by glTF TRS"
    );
    Ok((scale, rotation, translation))
}

fn sample_curve<const N: usize>(
    curve: &Gr2Curve,
    time: f32,
    default: [f32; N],
) -> anyhow::Result<[f32; N]> {
    ensure!(
        curve
            .controls
            .iter()
            .chain(&curve.knots)
            .all(|v| v.is_finite()),
        "nonfinite curve"
    );
    if curve.controls.is_empty() {
        ensure!(
            curve.knots.is_empty() && curve.degree == 0,
            "malformed empty curve"
        );
        return Ok(default);
    }
    if curve.degree == 0 {
        ensure!(
            curve.controls.len() == N && curve.knots.len() <= 1,
            "malformed constant curve"
        );
        return Ok(std::array::from_fn(|i| curve.controls[i]));
    }
    ensure!(
        curve.degree == 2,
        "unsupported curve degree {}",
        curve.degree
    );
    ensure!(
        curve.knots.len() >= 3 && curve.controls.len() == curve.knots.len() * N,
        "invalid quadratic curve dimensions"
    );
    ensure!(
        curve.knots.windows(2).all(|pair| pair[0] <= pair[1]),
        "unordered curve knots"
    );
    let knots = &curve.knots;
    let n = knots.len() as isize;
    let span = (knots.partition_point(|&k| k <= time) as isize).clamp(2, n - 1);
    let idx = |i: isize| i.clamp(0, n - 1) as usize;
    let [ka, kb, kc, kd] = [span - 2, span - 1, span, span + 1].map(|i| knots[idx(i)]);
    let divide = |num: f32, den: f32| if den.abs() < 1e-12 { 0.0 } else { num / den };
    let a = divide(time - kb, kc - kb);
    let b = divide(time - ka, kc - ka);
    let c = divide(time - kb, kd - kb);
    Ok(std::array::from_fn(|d| {
        let p = |i| curve.controls[idx(i) * N + d];
        let e1 = (1.0 - b) * p(span - 2) + b * p(span - 1);
        let e2 = (1.0 - c) * p(span - 1) + c * p(span);
        (1.0 - a) * e1 + a * e2
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadratic_curve_interpolates_known_control_points_and_endpoint() {
        let curve = Gr2Curve {
            degree: 2,
            knots: vec![0.0, 0.0, 1.0, 2.0],
            controls: vec![0.0, 2.0, 4.0, 6.0],
        };
        assert_eq!(sample_curve(&curve, 0.5, [9.0]).unwrap(), [1.75]);
        assert_eq!(sample_curve(&curve, 2.0, [9.0]).unwrap(), [6.0]);
    }

    #[test]
    fn absent_and_constant_channels_preserve_their_values() {
        assert_eq!(
            sample_curve(&Gr2Curve::default(), 0.5, [3.0, 4.0]).unwrap(),
            [3.0, 4.0]
        );
        let curve = Gr2Curve {
            degree: 0,
            knots: vec![0.0],
            controls: vec![5.0, 6.0],
        };
        assert_eq!(sample_curve(&curve, 0.5, [3.0, 4.0]).unwrap(), [5.0, 6.0]);
    }

    #[test]
    fn rejects_unsupported_or_malformed_curves() {
        for curve in [
            Gr2Curve {
                degree: 1,
                knots: vec![0.0, 1.0],
                controls: vec![0.0, 1.0],
            },
            Gr2Curve {
                degree: 2,
                knots: vec![0.0, 1.0, 2.0],
                controls: vec![0.0],
            },
            Gr2Curve {
                controls: vec![f32::NAN],
                ..Default::default()
            },
        ] {
            assert!(sample_curve(&curve, 0.5, [0.0]).is_err());
        }
    }

    #[test]
    fn normalizes_quaternions_and_rejects_shear() {
        let mut transform = Gr2Transform::IDENTITY;
        transform.rotation[3] = 2.0;
        assert_eq!(transform_matrix(&transform).unwrap(), Mat4::IDENTITY);
        transform.scale_shear[1] = 0.5;
        assert!(transform_matrix(&transform).is_err());
    }
}
