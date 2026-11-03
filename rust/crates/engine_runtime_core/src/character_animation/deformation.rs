//! Immutable renderer input. Morph deltas are applied in mesh space before skinning.
use super::*;
use crate::render_contract::GeometryBounds;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    sync::{Arc, Weak},
};

#[cfg(test)]
thread_local! { static SOURCE_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

// A weak alias reserves this immutable allocation's identity without retaining
// its streams. Arc::make_mut dissociates weak aliases, including with one strong
// owner, so edits and deserialized values must pass the original full check.
// This is an optional bounded memo: overflow and other threads validate normally.
const MAX_VALIDATED_SOURCES: usize = 256;
thread_local! {
    static VALIDATED_SOURCES: RefCell<Vec<Weak<SkinVertexData3D>>> = const { RefCell::new(Vec::new()) };
}
fn validate_shared_source(source: &Arc<SkinVertexData3D>) -> ContractResult<()> {
    let hit = VALIDATED_SOURCES.with(|cache| {
        cache
            .borrow()
            .iter()
            .any(|old| std::ptr::eq(old.as_ptr(), Arc::as_ptr(source)))
    });
    if hit {
        return Ok(());
    }
    source.validate()?;
    VALIDATED_SOURCES.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.retain(|old| old.strong_count() != 0);
        if cache.len() < MAX_VALIDATED_SOURCES {
            cache.push(Arc::downgrade(source));
        }
    });
    Ok(())
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::render_contract::IDENTITY_MATRIX;

    fn source() -> Arc<SkinVertexData3D> {
        Arc::new(SkinVertexData3D {
            vertex_count: 1,
            joint_count: 1,
            morph_count: 0,
            influences: Arc::new(
                [0_u32; 4]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .chain([1_f32, 0., 0., 0.].into_iter().flat_map(f32::to_le_bytes))
                    .collect(),
            ),
            morphs: Arc::new(Vec::new()),
        })
    }

    #[test]
    fn shared_immutable_skin_source_is_scanned_once_while_pose_is_always_checked() {
        let source = source();
        SOURCE_VALIDATIONS.with(|n| n.set(0));
        let deformation = SkinDeformation3D::new(
            source.clone(),
            &[IDENTITY_MATRIX],
            &[IDENTITY_MATRIX],
            &[],
            &[],
            false,
        )
        .unwrap();
        assert_eq!(SOURCE_VALIDATIONS.with(|n| n.get()), 1);
        for _ in 0..8 {
            deformation.validate().unwrap();
        }
        assert_eq!(
            SOURCE_VALIDATIONS.with(|n| n.get()),
            1,
            "unchanged immutable vertices must not be rescanned per pose/consumer"
        );
        let mut broken = deformation.clone();
        Arc::make_mut(&mut broken.current_matrices)[0..4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(
            broken.validate().is_err(),
            "source proof never validates a new matrix"
        );
        broken = deformation;
        broken.current_weights = Arc::new(vec![0; 4]);
        assert!(
            broken.validate().is_err(),
            "source proof never validates morph weights"
        );
    }

    #[test]
    fn source_mutation_and_deserialization_cannot_inherit_successful_validation() {
        let mut original = source();
        validate_shared_source(&original).unwrap();
        let old = Arc::downgrade(&original);
        Arc::make_mut(&mut Arc::make_mut(&mut original).influences)[0] = 1;
        assert!(
            old.upgrade().is_none(),
            "the successful proof retains no source payload"
        );
        assert!(validate_shared_source(&original).is_err());
        let good = source();
        validate_shared_source(&good).unwrap();
        let mut detached: SkinVertexData3D =
            serde_json::from_value(serde_json::to_value(&*good).unwrap()).unwrap();
        Arc::make_mut(&mut detached.influences)[16..20].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(validate_shared_source(&Arc::new(detached)).is_err());
        let mut shared = good.clone();
        Arc::make_mut(&mut shared).joint_count = 0;
        assert!(validate_shared_source(&shared).is_err());
        validate_shared_source(&good).unwrap();
    }

    #[test]
    fn source_proof_is_bounded_reclaims_expired_entries_and_never_caches_failure() {
        VALIDATED_SOURCES.with(|cache| cache.borrow_mut().clear());
        let mut held = Vec::new();
        for _ in 0..MAX_VALIDATED_SOURCES + 1 {
            let source = source();
            validate_shared_source(&source).unwrap();
            held.push(source);
        }
        assert_eq!(
            VALIDATED_SOURCES.with(|cache| cache.borrow().len()),
            MAX_VALIDATED_SOURCES
        );
        SOURCE_VALIDATIONS.with(|n| n.set(0));
        validate_shared_source(held.last().unwrap()).unwrap();
        assert_eq!(
            SOURCE_VALIDATIONS.with(|n| n.get()),
            1,
            "overflow remains a full-validation fallback"
        );
        held.clear();
        let good = source();
        validate_shared_source(&good).unwrap();
        assert_eq!(VALIDATED_SOURCES.with(|cache| cache.borrow().len()), 1);
        let mut bad = source();
        Arc::make_mut(&mut bad).vertex_count = 2;
        SOURCE_VALIDATIONS.with(|n| n.set(0));
        assert!(validate_shared_source(&bad).is_err());
        assert!(validate_shared_source(&bad).is_err());
        assert_eq!(SOURCE_VALIDATIONS.with(|n| n.get()), 2);
    }
}

/// Packed little endian GPU streams, shared by all instances of one cooked skin.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkinVertexData3D {
    pub vertex_count: u32,
    pub joint_count: u32,
    pub morph_count: u32,
    /// Eight u32 words per vertex: four joint indices, four f32 weights.
    pub influences: Arc<Vec<u8>>,
    /// Target-major six f32 words per vertex: position delta then normal delta.
    pub morphs: Arc<Vec<u8>>,
}
impl SkinVertexData3D {
    pub fn from_skin(skin: &CharacterSkin) -> Self {
        let influences = skin
            .influences
            .iter()
            .flat_map(|v| {
                v.joints
                    .iter()
                    .flat_map(|j| u32::from(*j).to_le_bytes())
                    .chain(v.weights.iter().flat_map(|v| v.to_le_bytes()))
                    .collect::<Vec<_>>()
            })
            .collect();
        let morphs = skin
            .morph_targets
            .iter()
            .flat_map(|m| {
                m.position_deltas
                    .iter()
                    .zip(&m.normal_deltas)
                    .flat_map(|(p, n)| p.iter().chain(n).flat_map(|v| v.to_le_bytes()))
            })
            .collect();
        Self {
            vertex_count: skin.influences.len() as u32,
            joint_count: skin.joints.len() as u32,
            morph_count: skin.morph_targets.len() as u32,
            influences: Arc::new(influences),
            morphs: Arc::new(morphs),
        }
    }
    pub fn validate(&self) -> ContractResult<()> {
        #[cfg(test)]
        SOURCE_VALIDATIONS.with(|count| count.set(count.get() + 1));
        let n = self.vertex_count as usize;
        if n == 0
            || n > 10_000_000
            || self.joint_count == 0
            || self.joint_count > MAX_JOINTS as u32
            || self.morph_count > 32
            || self.influences.len() != n * 32
            || self.morphs.len() != n * self.morph_count as usize * 24
        {
            return Err(invalid(
                "/skin/streams",
                "Invalid packed skin counts or lengths.",
            ));
        }
        for vertex in self.influences.chunks_exact(32) {
            let joints = vertex[..16]
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
            let weights: [f32; 4] = std::array::from_fn(|i| f32_at(&vertex[16 + i * 4..]));
            if joints.into_iter().any(|j| j >= self.joint_count)
                || weights
                    .iter()
                    .any(|w| !w.is_finite() || !(0.0..=1.0).contains(w))
                || (weights.iter().sum::<f32>() - 1.).abs() > 1e-4
            {
                return Err(invalid("/skin/weights", "Invalid packed joint weights."));
            }
        }
        if self.morphs.chunks_exact(4).any(|b| !f32_at(b).is_finite()) {
            return Err(invalid("/skin/morphs", "Non-finite morph delta."));
        }
        Ok(())
    }
}
fn f32_at(b: &[u8]) -> f32 {
    f32::from_le_bytes(b[..4].try_into().unwrap())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkinDeformation3D {
    pub source: Arc<SkinVertexData3D>,
    pub current_matrices: Arc<Vec<u8>>,
    pub previous_matrices: Arc<Vec<u8>>,
    pub current_weights: Arc<Vec<u8>>,
    pub previous_weights: Arc<Vec<u8>>,
    pub history_reset: bool,
}
impl SkinDeformation3D {
    pub fn new(
        source: Arc<SkinVertexData3D>,
        current: &[[f32; 16]],
        previous: &[[f32; 16]],
        weights: &[f32],
        previous_weights: &[f32],
        history_reset: bool,
    ) -> ContractResult<Self> {
        let pack = |v: &[f32]| Arc::new(v.iter().flat_map(|v| v.to_le_bytes()).collect());
        let result = Self {
            source,
            current_matrices: pack(current.as_flattened()),
            previous_matrices: pack(previous.as_flattened()),
            current_weights: pack(weights),
            previous_weights: pack(previous_weights),
            history_reset,
        };
        result.validate()?;
        Ok(result)
    }
    pub fn validate(&self) -> ContractResult<()> {
        validate_shared_source(&self.source)?;
        for bytes in [&self.current_matrices, &self.previous_matrices] {
            if bytes.len() != self.source.joint_count as usize * 64 {
                return Err(invalid("/skin/matrices", "Matrix count mismatch."));
            }
            for bytes in bytes.chunks_exact(64) {
                let m = Mat4::from_cols_array(&std::array::from_fn(|i| f32_at(&bytes[i * 4..])));
                if !m.is_finite()
                    || m.determinant().abs() < 1e-10
                    || m.row(3).to_array() != [0., 0., 0., 1.]
                {
                    return Err(invalid(
                        "/skin/matrices",
                        "Skin matrix must be finite, affine and invertible.",
                    ));
                }
            }
        }
        for bytes in [&self.current_weights, &self.previous_weights] {
            if bytes.len() != self.source.morph_count as usize * 4
                || bytes
                    .chunks_exact(4)
                    .any(|b| !(0.0..=1.0).contains(&f32_at(b)))
            {
                return Err(invalid("/skin/morphWeights", "Morph weight outside [0,1]."));
            }
        }
        Ok(())
    }
    /// Scalar reference / bounds inspection only; native rendering skins on the GPU.
    pub fn reference_vertex(
        &self,
        index: u32,
        position: [f32; 3],
        normal: [f32; 3],
        previous: bool,
    ) -> ContractResult<([f32; 3], [f32; 3])> {
        if index >= self.source.vertex_count {
            return Err(invalid("/skin/vertex", "Vertex out of range."));
        }
        let matrices = if previous {
            &self.previous_matrices
        } else {
            &self.current_matrices
        };
        let weights = if previous {
            &self.previous_weights
        } else {
            &self.current_weights
        };
        let mut p = Vec3::from_array(position);
        let mut n = Vec3::from_array(normal);
        for target in 0..self.source.morph_count as usize {
            let offset = (target * self.source.vertex_count as usize + index as usize) * 24;
            let delta = &self.source.morphs[offset..offset + 24];
            let w = f32_at(&weights[target * 4..]);
            p += Vec3::new(f32_at(delta), f32_at(&delta[4..]), f32_at(&delta[8..])) * w;
            n += Vec3::new(
                f32_at(&delta[12..]),
                f32_at(&delta[16..]),
                f32_at(&delta[20..]),
            ) * w;
        }
        let influence = &self.source.influences[index as usize * 32..];
        let mut out = Vec3::ZERO;
        let mut normal = Vec3::ZERO;
        for i in 0..4 {
            let j = u32::from_le_bytes(influence[i * 4..i * 4 + 4].try_into().unwrap()) as usize;
            let w = f32_at(&influence[16 + i * 4..]);
            if w == 0. {
                continue;
            }
            let bytes = &matrices[j * 64..];
            let m = Mat4::from_cols_array(&std::array::from_fn(|i| f32_at(&bytes[i * 4..])));
            out += m.transform_point3(p) * w;
            normal += m.inverse().transpose().transform_vector3(n) * w;
        }
        if !out.is_finite() || normal.length_squared() < 1e-16 {
            return Err(invalid("/skin/result", "Degenerate deformed vertex."));
        }
        Ok((out.to_array(), normal.normalize().to_array()))
    }
    /// Positive normalized weights make this union conservative for every skinned vertex.
    pub fn conservative_bounds(&self, base: &GeometryBounds) -> GeometryBounds {
        let mut extent = Vec3::ZERO;
        for target in 0..self.source.morph_count as usize {
            let mut delta = Vec3::ZERO;
            let start = target * self.source.vertex_count as usize * 24;
            for vertex in self.source.morphs[start..start + self.source.vertex_count as usize * 24]
                .chunks_exact(24)
            {
                delta = delta.max(
                    Vec3::new(f32_at(vertex), f32_at(&vertex[4..]), f32_at(&vertex[8..])).abs(),
                );
            }
            let w = f32_at(&self.current_weights[target * 4..])
                .max(f32_at(&self.previous_weights[target * 4..]));
            extent += delta * w;
        }
        let lo = Vec3::from_array(base.min) - extent;
        let hi = Vec3::from_array(base.max) + extent;
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for bytes in self
            .current_matrices
            .chunks_exact(64)
            .chain(self.previous_matrices.chunks_exact(64))
        {
            let m = Mat4::from_cols_array(&std::array::from_fn(|i| f32_at(&bytes[i * 4..])));
            for bits in 0..8 {
                let p = m.transform_point3(Vec3::new(
                    if bits & 1 == 0 { lo.x } else { hi.x },
                    if bits & 2 == 0 { lo.y } else { hi.y },
                    if bits & 4 == 0 { lo.z } else { hi.z },
                ));
                min = min.min(p);
                max = max.max(p);
            }
        }
        let center = (min + max) * 0.5;
        GeometryBounds {
            min: min.to_array(),
            max: max.to_array(),
            sphere_center: center.to_array(),
            sphere_radius: (max - center).length(),
        }
    }
}
