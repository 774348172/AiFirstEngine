//! Spatial evaluation belongs to World's existing hierarchy and dirty lifecycle.
use super::*;
#[path = "spatial_3d/bulk.rs"]
mod bulk;
#[path = "spatial_3d/structure.rs"]
mod structure;
#[path = "spatial_3d/timeline.rs"]
mod timeline;
use crate::render_contract::{multiply_matrix, CameraPose, IDENTITY_MATRIX};
use crate::spatial_contract::{
    Camera3D, CanonicalTransform3D, Light3D, MeshRenderer3D, Visibility3D,
};
pub(crate) use structure::{PreparedSpatialStructure, SpatialStructureRequest};
pub use timeline::*;

impl World {
    pub(crate) fn decompose_spatial_matrix(
        id: &EntityId,
        matrix: [f32; 16],
    ) -> Result<CanonicalTransform3D, WorldMutationError> {
        structure::decompose(id, matrix)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpatialWorldState {
    pub anchor: [f64; 3],
    pub world_matrix: [f32; 16],
    pub previous_world_matrix: [f32; 16],
    pub pose: CameraPose,
    pub enabled: bool,
    pub authored_visible: bool,
    pub inherited_visible: bool,
    pub rigid: bool,
    pub version: u64,
}

struct SpatialPublishInput<'a> {
    entity_id: &'a EntityId,
    runtime_id: RuntimeEntityId,
    local: CanonicalTransform3D,
    parent: Option<EntityId>,
    state: SpatialWorldState,
    mask: SpatialFieldMask,
    explicit: bool,
}

#[derive(Clone)]
struct Node {
    anchor: [f64; 3],
    parent: Option<EntityId>,
    transform: Option<CanonicalTransform3D>,
    enabled: bool,
    visible: bool,
    camera: bool,
}

pub(super) fn is_spatial_type(t: &ComponentTypeId) -> bool {
    t.is_spatial3d()
}

fn failure(id: &EntityId, code: &'static str, message: impl Into<String>) -> WorldMutationError {
    WorldMutationError::new(
        code,
        "spatial_hierarchy",
        message,
        "Repair the typed spatial value or parent reference.",
    )
    .with_source_entity_id(id.clone())
}

fn matrix(t: &CanonicalTransform3D) -> [f32; 16] {
    let [x, y, z, w] = t.rotation_xyzw;
    let [sx, sy, sz] = t.scale;
    let [tx, ty, tz] = t.translation;
    [
        (1. - 2. * (y * y + z * z)) * sx,
        2. * (x * y + w * z) * sx,
        2. * (x * z - w * y) * sx,
        0.,
        2. * (x * y - w * z) * sy,
        (1. - 2. * (x * x + z * z)) * sy,
        2. * (y * z + w * x) * sy,
        0.,
        2. * (x * z + w * y) * sz,
        2. * (y * z - w * x) * sz,
        (1. - 2. * (x * x + y * y)) * sz,
        0.,
        tx,
        ty,
        tz,
        1.,
    ]
}

#[cfg(test)]
std::thread_local! { static SPATIAL_PARENT_PRODUCTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
std::thread_local! { static SPATIAL_DIRTY_RECORD_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

fn compose_spatial_matrix(parent: Option<&SpatialWorldState>, local: [f32; 16]) -> [f32; 16] {
    let Some(parent) = parent else {
        return local;
    };
    #[cfg(test)]
    SPATIAL_PARENT_PRODUCTS.with(|n| n.set(n.get() + 1));
    multiply_matrix(parent.world_matrix, local)
}

fn rotation(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [x, y, z, w] = a;
    let [i, j, k, r] = b;
    let q = [
        w * i + x * r + y * k - z * j,
        w * j - x * k + y * r + z * i,
        w * k + x * j - y * i + z * r,
        w * r - x * i - y * j - z * k,
    ];
    let n = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    q.map(|v| v / n)
}

fn valid_affine(m: [f32; 16]) -> bool {
    valid_affine_with_linear_check(m, &mut None)
}

#[cfg(test)]
mod affine_reuse_tests {
    use super::*;

    // Frozen scalar predicate, independent of the reuse implementation.
    fn scalar(m: [f32; 16]) -> bool {
        if m.iter().any(|v| !v.is_finite()) {
            return false;
        }
        let a = m.map(f64::from);
        let cof = [
            a[5] * a[10] - a[9] * a[6],
            a[9] * a[2] - a[1] * a[10],
            a[1] * a[6] - a[5] * a[2],
            a[8] * a[6] - a[4] * a[10],
            a[0] * a[10] - a[8] * a[2],
            a[4] * a[2] - a[0] * a[6],
            a[4] * a[9] - a[8] * a[5],
            a[8] * a[1] - a[0] * a[9],
            a[0] * a[5] - a[4] * a[1],
        ];
        let det = a[0] * cof[0] + a[4] * cof[1] + a[8] * cof[2];
        det != 0.
            && (det as f32).is_finite()
            && (det as f32) != 0.
            && cof
                .iter()
                .all(|v| (*v / det) as f32 != f32::INFINITY && ((*v / det) as f32).is_finite())
            && (0..3).all(|i| {
                let v = ((cof[i] * a[12] + cof[3 + i] * a[13] + cof[6 + i] * a[14]) / det) as f32;
                v != f32::INFINITY && v.is_finite()
            })
    }

    #[test]
    fn root_linear_work_is_shared_but_each_translation_is_checked() {
        let id = EntityId::from("root-linear");
        for count in [1usize, 16, 128, 1024] {
            let mut cache = None;
            let mut node = Node {
                anchor: [0.; 3],
                parent: None,
                transform: None,
                enabled: true,
                visible: true,
                camera: false,
            };
            ROOT_LINEAR_BUILDS.with(|n| n.set(0));
            for i in 0..count {
                let t = CanonicalTransform3D {
                    translation: [i as f32, -0., 2.],
                    rotation_xyzw: [0., 0., 0., 1.],
                    scale: [1.; 3],
                };
                let expected = evaluate_validated_node(&id, &node, &t, None).unwrap();
                // The scalar reference above has an independent one-call cache.
                let before = ROOT_LINEAR_BUILDS.with(|n| n.get());
                let got =
                    evaluate_validated_node_with_linear_check(&id, &node, &t, None, &mut cache)
                        .unwrap();
                let added = ROOT_LINEAR_BUILDS.with(|n| n.get()) - before;
                assert_eq!(
                    added,
                    usize::from(i == 0),
                    "actual root matrix/rotation work at N={count}, row={i}"
                );
                assert_eq!(got, expected);
            }
            let mut t = CanonicalTransform3D {
                translation: [f32::MAX, 0., 0.],
                rotation_xyzw: [0., 0., 0., 1.],
                scale: [0.25, 1., 1.],
            };
            assert!(
                evaluate_validated_node_with_linear_check(&id, &node, &t, None, &mut cache)
                    .is_err()
            );
            t.translation = [1., 0., 0.];
            let got = evaluate_validated_node_with_linear_check(&id, &node, &t, None, &mut cache)
                .unwrap();
            assert_eq!(got, evaluate_validated_node(&id, &node, &t, None).unwrap());
            node.camera = true;
            assert_eq!(
                evaluate_validated_node_with_linear_check(&id, &node, &t, None, &mut cache)
                    .unwrap_err()
                    .code,
                "three_d.camera_scale"
            );
        }
    }

    #[test]
    fn root_linear_reuse_matches_fresh_evaluation_for_dynamic_linear_and_metadata() {
        let id = EntityId::from("dynamic-root");
        let mut cache = None;
        for i in 0..256 {
            let q = [0.1, (i as f32 * 0.02).sin(), -0.2, 1.];
            let norm = q.iter().map(|v| v * v).sum::<f32>().sqrt();
            let mut node = Node {
                anchor: [i as f64, 0., 0.],
                parent: None,
                transform: None,
                enabled: i % 2 == 0,
                visible: i % 3 == 0,
                camera: false,
            };
            for scale in [[1.; 3], [-2., 0.5, 3.], [f32::MIN_POSITIVE; 3], [0.; 3]] {
                for x in [0., -0., 32_000., f32::MAX, f32::INFINITY] {
                    let t = CanonicalTransform3D {
                        translation: [x, -0., 2.],
                        rotation_xyzw: q.map(|v| v / norm),
                        scale,
                    };
                    for camera in [false, true] {
                        node.camera = camera;
                        let old = evaluate_validated_node(&id, &node, &t, None);
                        let new = evaluate_validated_node_with_linear_check(
                            &id, &node, &t, None, &mut cache,
                        );
                        match (old, new) {
                            (Ok(a), Ok(b)) => {
                                assert_eq!(a, b);
                                assert_eq!(
                                    a.world_matrix.map(f32::to_bits),
                                    b.world_matrix.map(f32::to_bits)
                                );
                                assert_eq!(
                                    a.pose.rotation.map(f32::to_bits),
                                    b.pose.rotation.map(f32::to_bits)
                                );
                            }
                            (Err(a), Err(b)) => assert_eq!(a, b),
                            (a, b) => panic!("fresh/reused mismatch: {a:?} / {b:?}"),
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn spatial_affine_reuse_matches_scalar_extremes_and_checks_every_translation() {
        let mut cache = None;
        for scale in [
            0.,
            1.,
            -2.,
            0.00001,
            1e20,
            1e-30,
            f32::MAX,
            f32::MIN_POSITIVE,
        ] {
            for angle in [0., 0.2, 1.5, -2.4] {
                let q: [f32; 4] = [angle, 0.2, -0.1, 1.];
                let length = q.iter().map(|v| v * v).sum::<f32>().sqrt();
                let q =
                    crate::spatial_contract::canonical_quaternion(q.map(|v| v / length)).unwrap();
                for x in [0., 3., 32000., 1e30, f32::MAX, f32::INFINITY, f32::NAN] {
                    let m = matrix(&CanonicalTransform3D {
                        translation: [x, -x, 1.],
                        rotation_xyzw: q,
                        scale: [scale, 1., 1.],
                    });
                    assert_eq!(
                        valid_affine_with_linear_check(m, &mut cache),
                        scalar(m),
                        "scale={scale} angle={angle} x={x}"
                    );
                }
            }
        }
        let mut m = IDENTITY_MATRIX;
        m[0] = 0.25;
        assert!(valid_affine_with_linear_check(m, &mut cache));
        AFFINE_LINEAR_BUILDS.with(|n| n.set(0));
        m[12] = f32::MAX;
        assert!(
            !valid_affine_with_linear_check(m, &mut cache),
            "same valid linear matrix must not admit overflowing inverse translation"
        );
        assert_eq!(AFFINE_LINEAR_BUILDS.with(|n| n.get()), 0);
        m[12] = 1.;
        assert!(valid_affine_with_linear_check(m, &mut cache));
        m[0] = 0.;
        assert!(!valid_affine_with_linear_check(m, &mut cache));
        m[0] = 1.;
        assert!(valid_affine_with_linear_check(m, &mut cache));
    }
}

// Private to a single exclusive evaluation. Only cofactor/determinant work is
// shared; current translation and all finite/range checks remain per matrix.
// Pure root-local arithmetic, retained only by this exclusive evaluation.
// Bit keys preserve signed zero; translation and camera checks remain per row.
#[derive(Clone, Copy)]
struct RootLinearPreparation {
    key: ([u32; 4], [u32; 3]),
    matrix: [f32; 16],
    rotation: [f32; 4],
}
struct AffineLinearCheck {
    linear: [f32; 12],
    cof: [f64; 9],
    det: f64,
    root: Option<RootLinearPreparation>,
}

fn valid_affine_with_linear_check(m: [f32; 16], previous: &mut Option<AffineLinearCheck>) -> bool {
    if m.iter().any(|v| !v.is_finite()) {
        return false;
    }
    let a = m.map(f64::from);
    let linear: [f32; 12] = m[..12].try_into().expect("fixed matrix prefix");
    if previous.as_ref().is_none_or(|p| p.linear != linear) {
        #[cfg(test)]
        AFFINE_LINEAR_BUILDS.with(|n| n.set(n.get() + 1));
        let cof = [
            a[5] * a[10] - a[9] * a[6],
            a[9] * a[2] - a[1] * a[10],
            a[1] * a[6] - a[5] * a[2],
            a[8] * a[6] - a[4] * a[10],
            a[0] * a[10] - a[8] * a[2],
            a[4] * a[2] - a[0] * a[6],
            a[4] * a[9] - a[8] * a[5],
            a[8] * a[1] - a[0] * a[9],
            a[0] * a[5] - a[4] * a[1],
        ];
        let det = a[0] * cof[0] + a[4] * cof[1] + a[8] * cof[2];
        if !(det != 0.
            && (det as f32).is_finite()
            && (det as f32) != 0.
            && cof
                .iter()
                .all(|v| (*v / det) as f32 != f32::INFINITY && ((*v / det) as f32).is_finite()))
        {
            return false;
        }
        *previous = Some(AffineLinearCheck {
            linear,
            cof,
            det,
            root: None,
        });
    }
    let AffineLinearCheck { cof, det, .. } = previous.as_ref().expect("validated linear matrix");
    (0..3).all(|i| {
        ((cof[i] * a[12] + cof[3 + i] * a[13] + cof[6 + i] * a[14]) / det) as f32 != f32::INFINITY
            && (((cof[i] * a[12] + cof[3 + i] * a[13] + cof[6 + i] * a[14]) / det) as f32)
                .is_finite()
    })
}

#[cfg(test)]
std::thread_local! {static SPATIAL_EVALUATIONS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
std::thread_local! {static ROOT_LINEAR_BUILDS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
std::thread_local! {static AFFINE_LINEAR_BUILDS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
std::thread_local! {static SPATIAL_PUBLICATION_LOOKUPS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
std::thread_local! {static SPATIAL_SORT_COMPARISONS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}

fn evaluate(
    id: &EntityId,
    node: &impl Fn(&EntityId) -> Option<Node>,
    cache: &mut BTreeMap<EntityId, SpatialWorldState>,
    visiting: &mut BTreeSet<EntityId>,
) -> Result<SpatialWorldState, WorldMutationError> {
    if let Some(state) = cache.get(id) {
        return Ok(state.clone());
    }
    if !visiting.insert(id.clone()) {
        return Err(failure(id, "three_d.parent_cycle", "Hierarchy cycle."));
    }
    let n = node(id)
        .ok_or_else(|| failure(id, "three_d.parent_missing", "Missing hierarchy entity."))?;
    let parent = n
        .parent
        .as_ref()
        .map(|p| evaluate(p, node, cache, visiting))
        .transpose()?;
    let state = evaluate_node(id, n, parent.as_ref())?;
    visiting.remove(id);
    cache.insert(id.clone(), state.clone());
    Ok(state)
}

fn evaluate_node(
    id: &EntityId,
    n: Node,
    parent: Option<&SpatialWorldState>,
) -> Result<SpatialWorldState, WorldMutationError> {
    let t = n.transform.as_ref().ok_or_else(|| {
        failure(
            id,
            "three_d.domain_conflict",
            "Parent and child must share spatial domain.",
        )
    })?;
    t.validate()
        .map_err(|e| failure(id, "three_d.transform_invalid", e.to_string()))?;
    evaluate_validated_node(id, &n, t, parent)
}

// Only the immediate exclusive bulk commit calls this after validating every
// local value. World-matrix/inverse and camera constraints still execute here.
fn evaluate_validated_node(
    id: &EntityId,
    n: &Node,
    t: &CanonicalTransform3D,
    parent: Option<&SpatialWorldState>,
) -> Result<SpatialWorldState, WorldMutationError> {
    evaluate_validated_node_with_linear_check(id, n, t, parent, &mut None)
}

fn evaluate_validated_node_with_linear_check(
    id: &EntityId,
    n: &Node,
    t: &CanonicalTransform3D,
    parent: Option<&SpatialWorldState>,
    linear_check: &mut Option<AffineLinearCheck>,
) -> Result<SpatialWorldState, WorldMutationError> {
    #[cfg(test)]
    SPATIAL_EVALUATIONS.with(|count| count.set(count.get() + 1));
    let root_key = parent
        .is_none()
        .then(|| (t.rotation_xyzw.map(f32::to_bits), t.scale.map(f32::to_bits)));
    let prepared = root_key.and_then(|key| {
        linear_check
            .as_ref()
            .and_then(|check| check.root)
            .filter(|root| root.key == key)
    });
    let (world, pose_rotation) = if let Some(root) = prepared {
        let mut world = root.matrix;
        world[12..15].copy_from_slice(&t.translation);
        (world, root.rotation)
    } else {
        #[cfg(test)]
        if parent.is_none() {
            ROOT_LINEAR_BUILDS.with(|n| n.set(n.get() + 1));
        }
        (
            compose_spatial_matrix(parent, matrix(t)),
            rotation(
                parent
                    .as_ref()
                    .map_or([0., 0., 0., 1.], |p| p.pose.rotation),
                t.rotation_xyzw,
            ),
        )
    };
    if !valid_affine_with_linear_check(world, linear_check) {
        return Err(failure(
            id,
            "three_d.world_singular",
            "World matrix or its inverse is not representable.",
        ));
    }
    if prepared.is_none() {
        if let Some(key) = root_key {
            linear_check.as_mut().expect("validated affine").root = Some(RootLinearPreparation {
                key,
                matrix: world,
                rotation: pose_rotation,
            });
        }
    }
    let rigid = t.scale == [1.; 3] && parent.as_ref().is_none_or(|p| p.rigid);
    if n.camera && !rigid {
        return Err(failure(
            id,
            "three_d.camera_scale",
            "Camera and ancestors require unit scale.",
        ));
    }
    let state = SpatialWorldState {
        anchor: parent.as_ref().map_or(n.anchor, |p| p.anchor),
        world_matrix: world,
        previous_world_matrix: world,
        pose: CameraPose {
            position: [world[12], world[13], world[14]],
            rotation: pose_rotation,
        },
        enabled: n.enabled && parent.as_ref().is_none_or(|p| p.enabled),
        authored_visible: n.visible,
        inherited_visible: parent
            .as_ref()
            .is_none_or(|p| p.inherited_visible && p.authored_visible),
        rigid,
        version: 0,
    };
    Ok(state)
}

#[cfg(test)]
std::thread_local! { static SPATIAL_OVERLAY_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

impl World {
    #[cfg(test)]
    pub(crate) fn spatial_affine_linear_work(reset: bool) -> usize {
        AFFINE_LINEAR_BUILDS.with(|n| if reset { n.replace(0) } else { n.get() })
    }
    #[cfg(test)]
    pub(crate) fn spatial_parent_product_work(reset: bool) -> usize {
        SPATIAL_PARENT_PRODUCTS.with(|n| if reset { n.replace(0) } else { n.get() })
    }

    #[cfg(test)]
    pub(crate) fn spatial_overlay_work(reset: bool) -> usize {
        SPATIAL_OVERLAY_ENTRIES.with(|n| if reset { n.replace(0) } else { n.get() })
    }

    #[cfg(test)]
    pub(crate) fn spatial_publication_lookup_work(reset: bool) -> usize {
        SPATIAL_PUBLICATION_LOOKUPS.with(|n| if reset { n.replace(0) } else { n.get() })
    }
    #[cfg(test)]
    pub(crate) fn spatial_sort_work(reset: bool) -> usize {
        SPATIAL_SORT_COMPARISONS.with(|n| if reset { n.replace(0) } else { n.get() })
    }
    #[cfg(test)]
    pub(crate) fn spatial_tick_cow_work(reset: bool) -> usize {
        timeline::TICK_COW_BODIES.with(|n| if reset { n.replace(0) } else { n.get() })
    }

    #[cfg(test)]
    pub(crate) fn spatial_pose_allocation_work(reset: bool) -> usize {
        timeline::POSE_BODY_ALLOCATIONS.with(|n| if reset { n.replace(0) } else { n.get() })
    }

    #[cfg(test)]
    pub(crate) fn spatial_evaluation_work(reset: bool) -> usize {
        SPATIAL_EVALUATIONS.with(|count| if reset { count.replace(0) } else { count.get() })
    }

    fn validate_spatial_overlay(
        &self,
        writes: &[(EntityId, ComponentValue)],
        camera: Option<&EntityId>,
        parents: &BTreeMap<EntityId, Option<EntityId>>,
        anchors: &BTreeMap<EntityId, [f64; 3]>,
        allow_new_component: bool,
    ) -> Result<(), WorldMutationError> {
        self.evaluate_spatial_overlay(writes, camera, parents, anchors, allow_new_component)
            .map(|_| ())
    }

    fn evaluate_spatial_overlay(
        &self,
        writes: &[(EntityId, ComponentValue)],
        camera: Option<&EntityId>,
        parents: &BTreeMap<EntityId, Option<EntityId>>,
        anchors: &BTreeMap<EntityId, [f64; 3]>,
        allow_new_component: bool,
    ) -> Result<BTreeMap<EntityId, SpatialWorldState>, WorldMutationError> {
        let mut values = BTreeMap::new();
        let mut affected = parents
            .keys()
            .chain(anchors.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        for (id, value) in writes {
            #[cfg(test)]
            mutation_work::VALIDATED_WRITES.with(|count| count.set(count.get() + 1));
            self.ensure_entity_exists(id, "deferred_components")?;
            if !allow_new_component
                && !crate::character_animation::permits_control_bootstrap(self, id, value)
                && !crate::physics_3d::permits_control_bootstrap(self, id, value)
                && !crate::navigation_3d::permits_control_bootstrap(self, id, value)
                && !self.table_row_for_source(id).is_some_and(|(table, _)| {
                    table.has_component_named(value.component_type_name())
                })
            {
                return Err(WorldMutationError::missing_component(
                    "deferred_components",
                    id.clone(),
                    value.component_type(),
                ));
            }
            self.validate_spatial_value(id, value)?;
            values.insert((id, value.component_type_name()), value);
            if is_spatial_type(&value.component_type())
                || matches!(value, ComponentValue::EntityMeta(_))
            {
                affected.insert(id.clone());
            }
        }
        let mut roots = BTreeSet::new();
        for id in &affected {
            let mut parent = self.hierarchy(id).and_then(|h| h.parent_id);
            let mut covered = false;
            while let Some(p) = parent {
                if affected.contains(&p) {
                    covered = true;
                    break;
                }
                parent = self.hierarchy(&p).and_then(|h| h.parent_id);
            }
            if !covered {
                roots.insert(id.clone());
            }
        }
        affected = roots.iter().flat_map(|id| self.descendants(id)).collect();
        let nodes = |id: &EntityId| {
            let mut node = self.spatial_node(id)?;
            if let Some(parent) = parents.get(id) {
                node.parent = parent.clone();
            }
            if let Some(anchor) = anchors.get(id) {
                node.anchor = *anchor;
            }
            if let Some(ComponentValue::Transform3D(v)) = values.get(&(id, "engine.transform3d")) {
                node.transform = Some(v.clone());
            }
            if let Some(ComponentValue::EntityMeta(v)) = values.get(&(id, "engine.entity_meta")) {
                node.enabled = v.enabled;
            }
            let visibility = values.get(&(id, "engine.visibility3d"));
            let mesh = values.get(&(id, "engine.mesh_renderer3d"));
            if visibility.is_some() || mesh.is_some() {
                let visible = match visibility {
                    Some(ComponentValue::Visibility3D(v)) => v.visible,
                    _ => self.visibility3d(id).is_none_or(|v| v.visible),
                };
                let mesh_visible = match mesh {
                    Some(ComponentValue::MeshRenderer3D(v)) => v.visible,
                    _ => self.mesh_renderer3d(id).is_none_or(|v| v.visible),
                };
                node.visible = visible && mesh_visible;
            }
            Some(node)
        };
        let mut cache = BTreeMap::new();
        for id in affected {
            evaluate(&id, &nodes, &mut cache, &mut BTreeSet::new())?;
        }
        if let Some(id) = camera {
            let camera_enabled = match values.get(&(id, "engine.camera3d")) {
                Some(ComponentValue::Camera3D(v)) => v.enabled,
                _ => self.camera3d(id).is_some_and(|v| v.enabled),
            };
            if !camera_enabled || !evaluate(id, &nodes, &mut cache, &mut BTreeSet::new())?.enabled {
                return Err(failure(
                    id,
                    "three_d.main_camera_invalid",
                    "Select a live enabled Camera3D with enabled ancestors.",
                ));
            }
        }
        Ok(cache)
    }

    /// Called only after the deferred owner revalidates all values and live identities.
    pub(crate) fn commit_deferred_components(
        &mut self,
        writes: Vec<(EntityId, ComponentValue)>,
        camera: Option<EntityId>,
    ) {
        let final_values: BTreeMap<_, _> = writes
            .into_iter()
            .map(|(id, value)| ((id, value.component_type()), value))
            .collect();
        for ((id, _), value) in final_values {
            self.insert_component_value(id, value);
        }
        if let Some(camera) = camera {
            self.main_camera = Some(camera);
        }
    }

    pub fn transform3d(&self, id: &EntityId) -> Option<&CanonicalTransform3D> {
        let (t, r) = self.table_row_for_source(id)?;
        t.transform3d(r)
    }
    pub fn visibility3d(&self, id: &EntityId) -> Option<&Visibility3D> {
        let (t, r) = self.table_row_for_source(id)?;
        t.visibility3d(r)
    }
    pub fn mesh_renderer3d(&self, id: &EntityId) -> Option<&MeshRenderer3D> {
        let (t, r) = self.table_row_for_source(id)?;
        t.mesh_renderer3d(r)
    }
    pub fn light3d(&self, id: &EntityId) -> Option<&Light3D> {
        let (t, r) = self.table_row_for_source(id)?;
        t.light3d(r)
    }
    pub fn camera3d(&self, id: &EntityId) -> Option<&Camera3D> {
        let (t, r) = self.table_row_for_source(id)?;
        t.camera3d(r)
    }
    pub fn spatial_world(&self, id: &EntityId) -> Option<&SpatialWorldState> {
        self.spatial_timeline.world_state(id)
    }
    pub fn main_camera(&self) -> Option<&EntityId> {
        self.main_camera.as_ref()
    }

    pub fn select_main_camera(&mut self, id: Option<EntityId>) -> Result<(), WorldMutationError> {
        if let Some(id) = &id {
            if self.camera3d(id).is_none_or(|c| !c.enabled) {
                return Err(failure(
                    id,
                    "three_d.main_camera_invalid",
                    "Select an enabled Camera3D.",
                ));
            }
            let state = evaluate(
                id,
                &|id| self.spatial_node(id),
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?;
            if !state.enabled {
                return Err(failure(
                    id,
                    "three_d.main_camera_disabled",
                    "Selected camera or ancestor is disabled.",
                ));
            }
        }
        self.main_camera = id;
        Ok(())
    }

    fn spatial_node(&self, id: &EntityId) -> Option<Node> {
        let (table, row) = self.table_row_for_source(id)?;
        self.spatial_node_at(id, table, row)
    }
    fn spatial_node_at(&self, id: &EntityId, table: &ArchetypeTable, row: usize) -> Option<Node> {
        let meta = table.entity_meta(row)?;
        Some(Node {
            anchor: self.spatial_anchors.get(id).copied().unwrap_or([0.; 3]),
            parent: meta.hierarchy.parent_id.clone(),
            transform: table.transform3d(row).cloned(),
            enabled: meta.enabled,
            visible: table.visibility3d(row).is_none_or(|v| v.visible)
                && table.mesh_renderer3d(row).is_none_or(|v| v.visible),
            camera: table.camera3d(row).is_some(),
        })
    }

    pub(super) fn track_parent(&mut self, id: &EntityId, parent: Option<&EntityId>) {
        if let Some(p) = parent {
            self.hierarchy_children
                .entry(p.clone())
                .or_default()
                .insert(id.clone());
        }
    }
    pub(super) fn untrack_parent(&mut self, id: &EntityId) {
        if let Some(p) = self.hierarchy(id).and_then(|h| h.parent_id) {
            if let Some(children) = self.hierarchy_children.get_mut(&p) {
                children.remove(id);
                if children.is_empty() {
                    self.hierarchy_children.remove(&p);
                }
            }
        }
    }
    pub(crate) fn descendants(&self, id: &EntityId) -> BTreeSet<EntityId> {
        let mut result = BTreeSet::new();
        let mut pending = vec![id.clone()];
        while let Some(id) = pending.pop() {
            if result.insert(id.clone()) {
                if let Some(children) = self.hierarchy_children.get(&id) {
                    pending.extend(children.iter().cloned());
                }
            }
        }
        result
    }

    pub(super) fn reject_legacy_on_spatial(
        &self,
        id: &EntityId,
        t: &ComponentTypeId,
    ) -> Result<(), WorldMutationError> {
        if matches!(
            t.as_str(),
            "engine.transform"
                | "engine.renderable"
                | "engine.sprite_renderer2d"
                | "engine.collider2d"
                | "engine.animator2d"
        ) && self.transform3d(id).is_some()
        {
            return Err(failure(
                id,
                "three_d.domain_conflict",
                "Legacy spatial component on World3D entity.",
            ));
        }
        Ok(())
    }

    fn validate_spatial_value(
        &self,
        id: &EntityId,
        value: &ComponentValue,
    ) -> Result<(), WorldMutationError> {
        let t = value.component_type();
        if let ComponentValue::Dynamic { value, .. } = value {
            let json = || crate::character_animation::dynamic_json(value);
            if let Some(result) = crate::model_lod3d::validate_component(t.as_str(), json()) {
                result.map_err(|e| failure(id, "presentation.invalid", e.to_string()))?;
            }
            if t.as_str() == "engine.streaming_guard3d" {
                let b: crate::world_streaming::StreamingGuard3D = serde_json::from_value(json())
                    .map_err(|e| failure(id, "streaming.invalid", e.to_string()))?;
                b.validate()
                    .map_err(|e| failure(id, "streaming.invalid", e.to_string()))?;
            }
            if t.as_str() == "engine.world_streaming3d" {
                let b: crate::world_streaming::WorldStreaming3D = serde_json::from_value(json())
                    .map_err(|e| failure(id, "streaming.invalid", e.to_string()))?;
                b.validate()
                    .map_err(|e| failure(id, "streaming.invalid", e.to_string()))?;
            } else if t.as_str() == "engine.world_persistence3d" {
                let b: crate::world_persistence::WorldPersistence3D =
                    serde_json::from_value(json())
                        .map_err(|e| failure(id, "world_save.invalid", e.to_string()))?;
                crate::world_persistence::validate_binding(&b)
                    .map_err(|e| failure(id, "world_save.invalid", e.to_string()))?;
            }
        }
        self.reject_legacy_on_spatial(id, &t)?;
        if let ComponentValue::Dynamic { .. } = value {
            if ComponentRegistry::new().contains(&t) {
                return Err(failure(
                    id,
                    "three_d.component_reserved",
                    "Spatial components require typed values.",
                ));
            }
        }
        if is_spatial_type(&t) && self.transform3d(id).is_none() {
            return Err(failure(
                id,
                "three_d.domain_conflict",
                "Create World3D entities through typed hydration.",
            ));
        }
        // Metadata/parent writes must never desynchronize the hierarchy index,
        // including a legacy child trying to enter the 3D domain indirectly.
        match value {
            ComponentValue::Hierarchy(_) => {
                return Err(failure(
                    id,
                    "three_d.component_reserved",
                    "Use the structural hierarchy API.",
                ));
            }
            ComponentValue::EntityMeta(v) => {
                let old = self.entity(id).unwrap();
                if v.id != *id || v.hierarchy != old.hierarchy || v.alive != old.alive {
                    return Err(failure(
                        id,
                        "three_d.component_reserved",
                        "Identity and hierarchy require structural World APIs.",
                    ));
                }
            }
            _ => {}
        }
        let Some(mut n) = self.spatial_node(id) else {
            return Ok(());
        };
        if n.transform.is_none() {
            return Ok(());
        }
        match value {
            ComponentValue::Transform3D(v) => {
                v.validate()
                    .map_err(|e| failure(id, "three_d.transform_invalid", e.to_string()))?;
                n.transform = Some(v.clone());
            }
            ComponentValue::Visibility3D(v) => {
                if v.schema_version != "visibility3d.v1" {
                    return Err(failure(
                        id,
                        "three_d.schema_unsupported",
                        "Invalid Visibility3D version.",
                    ));
                }
                n.visible = v.visible && self.mesh_renderer3d(id).is_none_or(|m| m.visible);
            }
            ComponentValue::MeshRenderer3D(v) => {
                v.validate()
                    .map_err(|e| failure(id, "three_d.mesh_invalid", e.to_string()))?;
                n.visible = v.visible && self.visibility3d(id).is_none_or(|m| m.visible);
            }
            ComponentValue::Light3D(v) => {
                v.validate()
                    .map_err(|e| failure(id, "three_d.light_invalid", e.to_string()))?;
            }
            ComponentValue::Camera3D(v) => {
                v.validate()
                    .map_err(|e| failure(id, "three_d.camera_invalid", e.to_string()))?;
                n.camera = true;
            }
            ComponentValue::EntityMeta(v) => {
                let old = self.entity(id).unwrap();
                if v.id != *id || v.hierarchy != old.hierarchy || v.alive != old.alive {
                    return Err(failure(
                        id,
                        "three_d.component_reserved",
                        "Identity and hierarchy require structural World APIs.",
                    ));
                }
                n.enabled = v.enabled;
            }
            ComponentValue::Hierarchy(_) => {
                return Err(failure(
                    id,
                    "three_d.component_reserved",
                    "Use try_set_parent to update hierarchy.",
                ));
            }
            _ => return Ok(()),
        }
        Ok(())
    }
    pub(super) fn validate_spatial_component(
        &self,
        id: &EntityId,
        value: &ComponentValue,
    ) -> Result<(), WorldMutationError> {
        self.validate_spatial_overlay(
            &[(id.clone(), value.clone())],
            None,
            &BTreeMap::new(),
            &BTreeMap::new(),
            true,
        )
    }
    fn validate_changed_node(&self, id: &EntityId, n: Node) -> Result<(), WorldMutationError> {
        let nodes = |key: &EntityId| {
            if key == id {
                Some(n.clone())
            } else {
                self.spatial_node(key)
            }
        };
        let mut cache = BTreeMap::new();
        for child in self.descendants(id) {
            evaluate(&child, &nodes, &mut cache, &mut BTreeSet::new())?;
        }
        Ok(())
    }
    pub(super) fn validate_spatial_reparent(
        &self,
        id: &EntityId,
        parent: Option<&EntityId>,
    ) -> Result<(), WorldMutationError> {
        let spatial = self.transform3d(id).is_some();
        if parent.is_some_and(|p| self.transform3d(p).is_some() != spatial) {
            return Err(failure(
                id,
                "three_d.domain_conflict",
                "Parent and child must share spatial domain.",
            ));
        }
        if spatial {
            let mut n = self.spatial_node(id).unwrap();
            n.parent = parent.cloned();
            self.validate_changed_node(id, n)?;
        }
        Ok(())
    }

    pub(crate) fn validate_prepared_spatial(
        &self,
        entities: &[crate::runtime_entity_hydration::PreparedRuntimeEntity],
    ) -> Result<(), WorldMutationError> {
        self.evaluate_prepared_spatial(entities).map(|_| ())
    }
    pub(crate) fn evaluate_prepared_spatial(
        &self,
        entities: &[crate::runtime_entity_hydration::PreparedRuntimeEntity],
    ) -> Result<BTreeMap<EntityId, SpatialWorldState>, WorldMutationError> {
        let mut staged = BTreeMap::new();
        for e in entities {
            let mut n = Node {
                anchor: [0.; 3],
                parent: e.hierarchy.parent_id.clone(),
                transform: None,
                enabled: e.enabled,
                visible: true,
                camera: false,
            };
            for c in &e.components {
                match c {
                    ComponentValue::Transform3D(v) => n.transform = Some(v.clone()),
                    ComponentValue::Visibility3D(v) => n.visible &= v.visible,
                    ComponentValue::MeshRenderer3D(v) => n.visible &= v.visible,
                    ComponentValue::Camera3D(_) => n.camera = true,
                    _ => {}
                }
            }
            staged.insert(e.world_id.clone(), n);
        }
        let nodes = |key: &EntityId| staged.get(key).cloned().or_else(|| self.spatial_node(key));
        let mut cache = BTreeMap::new();
        for (id, n) in &staged {
            if n.transform.is_some() {
                evaluate(id, &nodes, &mut cache, &mut BTreeSet::new())?;
            } else if n
                .parent
                .as_ref()
                .and_then(&nodes)
                .is_some_and(|p| p.transform.is_some())
            {
                return Err(failure(
                    id,
                    "three_d.domain_conflict",
                    "Legacy child under World3D parent.",
                ));
            }
        }
        cache.retain(|id, _| staged.contains_key(id));
        Ok(cache)
    }

    /// Exclusive immediate commit. The final evaluation never crosses a callback or yield.
    pub(crate) fn commit_spatial_local_batch(
        &mut self,
        updates: Vec<(
            project_runtime_sdk::spatial_bulk::SpatialEntityKey,
            project_runtime_sdk::ProjectRuntimeTransform3D,
        )>,
    ) -> Result<(), WorldMutationError> {
        self.check_spatial_budget(updates.len(), 0)?;
        if self.try_commit_simple_spatial(&updates)? {
            return Ok(());
        }
        let count = updates.len();
        let mut writes = Vec::with_capacity(count);
        for (key, t) in updates {
            let id = self.resolve_spatial_key(key)?;
            let transform = crate::spatial_contract::Transform3D {
                schema_version: "transform3d.v1".into(),
                translation: t.translation,
                rotation: crate::spatial_contract::Rotation3D::Quaternion {
                    xyzw: t.rotation_xyzw,
                },
                scale: t.scale,
            }
            .canonicalize()
            .map_err(|e| failure(&id, "three_d.transform_invalid", e.to_string()))?;
            writes.push((id, ComponentValue::Transform3D(transform)));
        }
        let evaluated = self.evaluate_spatial_overlay(
            &writes,
            None,
            &BTreeMap::new(),
            &BTreeMap::new(),
            false,
        )?;
        self.charge_spatial_budget(count, 0);
        self.commit_deferred_components(writes, None);
        self.propagate_spatial_evaluated(evaluated);
        Ok(())
    }

    /// Called at the existing dirty-drain boundary. Clean frames do no World scan.
    pub(crate) fn propagate_spatial(&mut self) {
        self.propagate_spatial_evaluated(BTreeMap::new());
    }

    fn take_spatial_render_dirty(&mut self) -> BTreeSet<EntityId> {
        let mut seen = std::mem::take(&mut self.spatial_render_dirty);
        // World owns this append-only queue until take_dirty_records drains it.
        // Repeated commits need only consume the newly appended suffix.
        let suffix = &self.dirty_records[self.spatial_render_dirty_scanned..];
        #[cfg(test)]
        SPATIAL_DIRTY_RECORD_SCANS.with(|n| n.set(n.get() + suffix.len()));
        seen.extend(
            suffix
                .iter()
                .filter(|r| r.dirty_type == DirtyType::RenderState)
                .map(|r| r.entity_id.clone()),
        );
        self.spatial_render_dirty_scanned = self.dirty_records.len();
        seen
    }

    fn propagate_spatial_evaluated(
        &mut self,
        mut evaluated: BTreeMap<EntityId, SpatialWorldState>,
    ) {
        let masks = std::mem::take(&mut self.spatial_dirty);
        let roots = masks
            .keys()
            .filter(|id| {
                let mut parent = self.hierarchy(id).and_then(|h| h.parent_id);
                while let Some(p) = parent {
                    if masks.get(&p).is_some_and(|mask| {
                        mask.0
                            & (SpatialFieldMask::TRANSFORM.0
                                | SpatialFieldMask::VISIBILITY.0
                                | SpatialFieldMask::HIERARCHY.0)
                            != 0
                    }) {
                        return false;
                    }
                    parent = self.hierarchy(&p).and_then(|h| h.parent_id);
                }
                true
            })
            .cloned()
            .collect::<Vec<_>>();
        if roots.is_empty() {
            // Legacy standalone dirty-drain callers have no tick owner. Preserve their
            // one-drain settling contract; the formal frame loop uses explicit ticks.
            if self.spatial_timeline.tick == 0 {
                for id in std::mem::take(&mut self.spatial_previous_pending) {
                    if let Some(state) = self.spatial_timeline.world_state_mut(&id) {
                        if state.previous_world_matrix != state.world_matrix {
                            state.previous_world_matrix = state.world_matrix;
                            self.dirty_records.push(DirtyRecord {
                                entity_id: id.clone(),
                                dirty_type: DirtyType::RenderState,
                            });
                        }
                    }
                    self.reset_spatial_history(&id, None);
                }
            }
            return;
        }
        let root_count = roots.len();
        let mut affected = BTreeSet::new();
        for root in roots {
            if masks[&root].0
                & (SpatialFieldMask::TRANSFORM.0
                    | SpatialFieldMask::VISIBILITY.0
                    | SpatialFieldMask::HIERARCHY.0)
                != 0
            {
                affected.extend(self.descendants(&root));
            } else {
                affected.insert(root);
            }
        }
        for id in &affected {
            evaluate(
                id,
                &|id| self.spatial_node(id),
                &mut evaluated,
                &mut BTreeSet::new(),
            )
            .expect("spatial mutations are validated before commit");
        }
        let already_dirty = self.take_spatial_render_dirty();
        let mut changes = Vec::new();
        for id in affected {
            let state = evaluated.remove(&id).expect("evaluated descendant");
            let node = self.spatial_node(&id).expect("evaluated live node");
            let input = SpatialPublishInput {
                runtime_id: self
                    .runtime_id_for_source(&id)
                    .expect("live spatial entity"),
                local: node.transform.expect("spatial transform"),
                parent: node.parent,
                mask: masks.get(&id).copied().unwrap_or_default(),
                explicit: masks.contains_key(&id),
                entity_id: &id,
                state,
            };
            self.publish_spatial_state(input, &already_dirty, &mut changes);
        }
        self.spatial_timeline.merge_delta(root_count, changes);
        self.spatial_render_dirty = already_dirty;
    }
}

#[cfg(test)]
mod root_matrix_reference_tests {
    use super::*;
    #[test]
    fn render_dirty_membership_consumes_only_appends_and_drain_releases_old_ids() {
        let mut world = World::new();
        for i in 0..1000 {
            world.mark_render_payload_dirty(format!("old-{i}").into());
        }
        SPATIAL_DIRTY_RECORD_SCANS.with(|n| n.set(0));
        for _ in 0..100 {
            let seen = world.take_spatial_render_dirty();
            assert_eq!(seen.len(), 1000);
            world.spatial_render_dirty = seen;
        }
        assert_eq!(SPATIAL_DIRTY_RECORD_SCANS.with(|n| n.get()), 1000);
        world.mark_render_payload_dirty("new".into());
        let seen = world.take_spatial_render_dirty();
        assert_eq!(seen.len(), 1001);
        assert!(seen.contains(&EntityId::from("new")));
        world.spatial_render_dirty = seen;
        assert_eq!(SPATIAL_DIRTY_RECORD_SCANS.with(|n| n.get()), 1001);
        let retained_records = world.take_dirty_records();
        assert_eq!(retained_records.len(), 1001);
        assert!(world.spatial_render_dirty.is_empty());
        assert_eq!(world.spatial_render_dirty_scanned, 0);
        world.mark_render_payload_dirty("after-drain".into());
        let seen = world.take_spatial_render_dirty();
        assert_eq!(seen, [EntityId::from("after-drain")].into());
        assert_eq!(SPATIAL_DIRTY_RECORD_SCANS.with(|n| n.get()), 1002);
    }

    #[test]
    fn root_matrix_matches_identity_reference_and_parent_still_composes() {
        for i in 0..1000 {
            let q = [(i as f32 * 0.01).sin(), (i as f32 * 0.02).cos(), 0.1, 0.7];
            let norm = q.iter().map(|v| v * v).sum::<f32>().sqrt();
            let local = CanonicalTransform3D {
                translation: [i as f32 - 500., i as f32 * 0.03125, -0.],
                rotation_xyzw: crate::spatial_contract::canonical_quaternion(q.map(|v| v / norm))
                    .unwrap(),
                scale: [if i % 2 == 0 { -2. } else { 3. }, 0.001, 1e3],
            };
            let m = matrix(&local);
            SPATIAL_PARENT_PRODUCTS.with(|n| n.set(0));
            assert_eq!(
                compose_spatial_matrix(None, m),
                multiply_matrix(IDENTITY_MATRIX, m)
            );
            assert_eq!(SPATIAL_PARENT_PRODUCTS.with(|n| n.get()), 0);
            let parent = SpatialWorldState {
                anchor: [0.; 3],
                world_matrix: m,
                previous_world_matrix: m,
                pose: CameraPose::default(),
                enabled: true,
                authored_visible: true,
                inherited_visible: true,
                rigid: false,
                version: 1,
            };
            assert_eq!(
                compose_spatial_matrix(Some(&parent), m),
                multiply_matrix(m, m)
            );
            assert_eq!(SPATIAL_PARENT_PRODUCTS.with(|n| n.get()), 1);
        }
        for scale in [f32::MAX, f32::MIN_POSITIVE, 1e-30, -1e30] {
            let local = CanonicalTransform3D {
                translation: [1e30, -1e30, 0.],
                rotation_xyzw: [0., 0., 0., 1.],
                scale: [scale; 3],
            };
            let m = matrix(&local);
            assert_eq!(
                valid_affine(compose_spatial_matrix(None, m)),
                valid_affine(multiply_matrix(IDENTITY_MATRIX, m))
            );
        }
    }
}
