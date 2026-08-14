use crate::components::{ComponentTypeId, Transform};
use crate::ids::EntityId;
use crate::math::Vec2;
use crate::projection::{ProjectionDiagnostic, ProjectionDomain, ProjectionKind, ProjectionReport};
use crate::world::World;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicsLayer(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicsMask(pub u32);

impl PhysicsLayer {
    pub const DEFAULT: Self = Self(1);

    pub fn matches(self, mask: PhysicsMask) -> bool {
        self.0 & mask.0 != 0
    }
}

impl PhysicsMask {
    pub const ALL: Self = Self(u32::MAX);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape2D {
    Aabb { half_extents: Vec2 },
    Circle { radius: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Collider2D {
    pub shape: Shape2D,
    pub offset: Vec2,
    pub layer: PhysicsLayer,
    pub mask: PhysicsMask,
    pub enabled: bool,
    pub is_sensor: bool,
}

impl Collider2D {
    pub fn aabb(half_extents: Vec2) -> Self {
        Self {
            shape: Shape2D::Aabb { half_extents },
            offset: Vec2::ZERO,
            layer: PhysicsLayer::DEFAULT,
            mask: PhysicsMask::ALL,
            enabled: true,
            is_sensor: false,
        }
    }

    pub fn circle(radius: f32) -> Self {
        Self {
            shape: Shape2D::Circle { radius },
            offset: Vec2::ZERO,
            layer: PhysicsLayer::DEFAULT,
            mask: PhysicsMask::ALL,
            enabled: true,
            is_sensor: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Physics2DColliderProxy {
    pub entity_id: EntityId,
    pub world_position: Vec2,
    pub shape: Shape2D,
    pub layer: PhysicsLayer,
    pub mask: PhysicsMask,
    pub enabled: bool,
    pub is_sensor: bool,
}

impl Physics2DColliderProxy {
    pub fn from_transform_and_collider(
        entity_id: EntityId,
        transform: &Transform,
        collider: &Collider2D,
    ) -> Self {
        Self {
            entity_id,
            world_position: Vec2 {
                x: transform.local_position.x + collider.offset.x,
                y: transform.local_position.y + collider.offset.y,
            },
            shape: collider.shape,
            layer: collider.layer,
            mask: collider.mask,
            enabled: collider.enabled,
            is_sensor: collider.is_sensor,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Physics2DWorld {
    colliders: BTreeMap<EntityId, Physics2DColliderProxy>,
}

impl Physics2DWorld {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_or_update_collider(&mut self, proxy: Physics2DColliderProxy) {
        self.colliders.insert(proxy.entity_id.clone(), proxy);
    }

    pub fn remove_collider(&mut self, entity_id: &EntityId) -> Option<Physics2DColliderProxy> {
        self.colliders.remove(entity_id)
    }

    pub fn clear(&mut self) {
        self.colliders.clear();
    }

    pub fn collider_count(&self) -> usize {
        self.colliders.len()
    }

    pub fn collider(&self, entity_id: &EntityId) -> Option<&Physics2DColliderProxy> {
        self.colliders.get(entity_id)
    }

    pub fn collider_ids(&self) -> Vec<EntityId> {
        self.colliders.keys().cloned().collect()
    }

    pub fn overlap_aabb(&self, query: &OverlapAabb2D) -> Vec<Physics2DHit> {
        let query_shape = Shape2D::Aabb {
            half_extents: query.half_extents,
        };
        let mut hits = self
            .colliders
            .values()
            .filter(|proxy| {
                should_test_proxy(proxy, query.layer, query.mask, query.include_sensors)
            })
            .filter(|proxy| {
                shapes_overlap(query.center, query_shape, proxy.world_position, proxy.shape)
            })
            .map(Physics2DHit::from_proxy)
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
        if let Some(limit) = query.limit {
            hits.truncate(limit);
        }
        hits
    }

    pub fn overlap_circle(&self, query: &OverlapCircle2D) -> Vec<Physics2DHit> {
        let query_shape = Shape2D::Circle {
            radius: query.radius,
        };
        let mut hits = self
            .colliders
            .values()
            .filter(|proxy| {
                should_test_proxy(proxy, query.layer, query.mask, query.include_sensors)
            })
            .filter(|proxy| {
                shapes_overlap(query.center, query_shape, proxy.world_position, proxy.shape)
            })
            .map(Physics2DHit::from_proxy)
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
        if let Some(limit) = query.limit {
            hits.truncate(limit);
        }
        hits
    }

    pub fn build_collision_pairs(&self) -> CollisionPairReport {
        let proxies = self
            .colliders
            .values()
            .filter(|proxy| proxy.enabled)
            .collect::<Vec<_>>();
        let mut pairs = Vec::new();
        let mut tested_pair_count = 0;
        let mut filtered_pair_count = 0;
        for left_index in 0..proxies.len() {
            for right_index in (left_index + 1)..proxies.len() {
                tested_pair_count += 1;
                let left = proxies[left_index];
                let right = proxies[right_index];
                if !layers_match_pair(left, right) {
                    filtered_pair_count += 1;
                    continue;
                }
                if !shapes_overlap(
                    left.world_position,
                    left.shape,
                    right.world_position,
                    right.shape,
                ) {
                    continue;
                }
                pairs.push(CollisionPair::new(left, right));
            }
        }
        pairs.sort_by(|left, right| {
            left.entity_a
                .cmp(&right.entity_a)
                .then_with(|| left.entity_b.cmp(&right.entity_b))
        });
        CollisionPairReport {
            pairs,
            collider_count: self.collider_count(),
            tested_pair_count,
            filtered_pair_count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlapAabb2D {
    pub center: Vec2,
    pub half_extents: Vec2,
    pub layer: PhysicsLayer,
    pub mask: PhysicsMask,
    pub include_sensors: bool,
    pub limit: Option<usize>,
}

impl OverlapAabb2D {
    pub fn new(center: Vec2, half_extents: Vec2) -> Self {
        Self {
            center,
            half_extents,
            layer: PhysicsLayer::DEFAULT,
            mask: PhysicsMask::ALL,
            include_sensors: true,
            limit: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlapCircle2D {
    pub center: Vec2,
    pub radius: f32,
    pub layer: PhysicsLayer,
    pub mask: PhysicsMask,
    pub include_sensors: bool,
    pub limit: Option<usize>,
}

impl OverlapCircle2D {
    pub fn new(center: Vec2, radius: f32) -> Self {
        Self {
            center,
            radius,
            layer: PhysicsLayer::DEFAULT,
            mask: PhysicsMask::ALL,
            include_sensors: true,
            limit: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Physics2DHit {
    pub entity_id: EntityId,
    pub shape: Shape2D,
    pub distance_hint: f32,
}

impl Physics2DHit {
    fn from_proxy(proxy: &Physics2DColliderProxy) -> Self {
        Self {
            entity_id: proxy.entity_id.clone(),
            shape: proxy.shape,
            distance_hint: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CollisionPair {
    pub entity_a: EntityId,
    pub entity_b: EntityId,
    pub shape_a: Shape2D,
    pub shape_b: Shape2D,
    pub is_sensor_pair: bool,
}

impl CollisionPair {
    fn new(left: &Physics2DColliderProxy, right: &Physics2DColliderProxy) -> Self {
        let (entity_a, shape_a, entity_b, shape_b) = if left.entity_id <= right.entity_id {
            (
                left.entity_id.clone(),
                left.shape,
                right.entity_id.clone(),
                right.shape,
            )
        } else {
            (
                right.entity_id.clone(),
                right.shape,
                left.entity_id.clone(),
                left.shape,
            )
        };
        Self {
            entity_a,
            entity_b,
            shape_a,
            shape_b,
            is_sensor_pair: left.is_sensor || right.is_sensor,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CollisionPairReport {
    pub pairs: Vec<CollisionPair>,
    pub collider_count: usize,
    pub tested_pair_count: usize,
    pub filtered_pair_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Physics2DSyncDiagnostic {
    pub entity_id: EntityId,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Physics2DSyncReport {
    pub scanned_entities: usize,
    pub synced_colliders: usize,
    pub removed_colliders: usize,
    pub disabled_colliders: usize,
    pub diagnostics: Vec<Physics2DSyncDiagnostic>,
}

impl Physics2DSyncReport {
    pub fn projection_summary(&self) -> ProjectionReport {
        let diagnostics = self
            .diagnostics
            .iter()
            .map(|diagnostic| {
                ProjectionDiagnostic::new(
                    "error",
                    diagnostic.code,
                    diagnostic.message.clone(),
                    Some("Physics2DProjectionAdapter<Collider2D>".to_string()),
                )
            })
            .collect::<Vec<_>>();
        ProjectionReport::new(
            ProjectionKind::Physics2D,
            ProjectionDomain::World,
            ProjectionDomain::Physics2D,
            "Physics2DProjectionAdapter<Collider2D>",
        )
        .with_counts(
            self.synced_colliders,
            self.disabled_colliders,
            self.diagnostics.len(),
        )
        .with_diagnostics(diagnostics)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Physics2DBridge;

impl Physics2DBridge {
    pub fn sync_from_world(
        world: &World,
        physics_world: &mut Physics2DWorld,
    ) -> Physics2DSyncReport {
        let previous_ids = physics_world
            .collider_ids()
            .into_iter()
            .collect::<BTreeSet<_>>();
        physics_world.clear();
        let mut report = Physics2DSyncReport::default();
        let collider_type = ComponentTypeId::collider2d();
        for entity_id in world.query_entities(
            &crate::query::QuerySpec::all([collider_type.clone()]).include_disabled(),
        ) {
            report.scanned_entities += 1;
            let Some(crate::archetype::ComponentValue::Collider2D(collider)) =
                world.component_value(&entity_id, &collider_type)
            else {
                continue;
            };
            if !collider.enabled {
                report.disabled_colliders += 1;
                continue;
            }
            let Some(transform) = world.transform(&entity_id) else {
                report.diagnostics.push(Physics2DSyncDiagnostic {
                    entity_id,
                    code: "missing_transform",
                    message: "Collider2D entity is missing Transform".to_string(),
                });
                continue;
            };
            physics_world.insert_or_update_collider(
                Physics2DColliderProxy::from_transform_and_collider(
                    entity_id, transform, &collider,
                ),
            );
            report.synced_colliders += 1;
        }
        let current_ids = physics_world
            .collider_ids()
            .into_iter()
            .collect::<BTreeSet<_>>();
        report.removed_colliders = previous_ids.difference(&current_ids).count();
        report
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Physics2DTraceRecord {
    pub frame_index: u64,
    pub phase: String,
    pub operation: String,
    pub entity_id: Option<EntityId>,
    pub query_kind: Option<String>,
    pub shape: Option<String>,
    pub layer: Option<u32>,
    pub mask: Option<u32>,
    pub hit_count: Option<usize>,
    pub pair_count: Option<usize>,
    pub result: String,
    pub error_code: Option<String>,
}

impl Physics2DTraceRecord {
    pub fn sync(frame_index: u64, phase: impl Into<String>, report: &Physics2DSyncReport) -> Self {
        Self {
            frame_index,
            phase: phase.into(),
            operation: "sync_from_world".to_string(),
            entity_id: report
                .diagnostics
                .first()
                .map(|diagnostic| diagnostic.entity_id.clone()),
            query_kind: None,
            shape: None,
            layer: None,
            mask: None,
            hit_count: Some(report.synced_colliders),
            pair_count: None,
            result: if report.diagnostics.is_empty() {
                "ok".to_string()
            } else {
                "diagnostic".to_string()
            },
            error_code: report
                .diagnostics
                .first()
                .map(|diagnostic| diagnostic.code.to_string()),
        }
    }

    pub fn pair_report(
        frame_index: u64,
        phase: impl Into<String>,
        report: &CollisionPairReport,
    ) -> Self {
        Self {
            frame_index,
            phase: phase.into(),
            operation: "build_collision_pairs".to_string(),
            entity_id: None,
            query_kind: None,
            shape: None,
            layer: None,
            mask: None,
            hit_count: None,
            pair_count: Some(report.pairs.len()),
            result: "ok".to_string(),
            error_code: None,
        }
    }
}

fn should_test_proxy(
    proxy: &&Physics2DColliderProxy,
    query_layer: PhysicsLayer,
    query_mask: PhysicsMask,
    include_sensors: bool,
) -> bool {
    proxy.enabled
        && (include_sensors || !proxy.is_sensor)
        && query_layer.matches(proxy.mask)
        && proxy.layer.matches(query_mask)
}

fn layers_match_pair(left: &Physics2DColliderProxy, right: &Physics2DColliderProxy) -> bool {
    left.layer.matches(right.mask) && right.layer.matches(left.mask)
}

fn shapes_overlap(
    left_center: Vec2,
    left_shape: Shape2D,
    right_center: Vec2,
    right_shape: Shape2D,
) -> bool {
    match (left_shape, right_shape) {
        (
            Shape2D::Aabb {
                half_extents: left_half,
            },
            Shape2D::Aabb {
                half_extents: right_half,
            },
        ) => aabb_aabb_overlap(left_center, left_half, right_center, right_half),
        (
            Shape2D::Circle {
                radius: left_radius,
            },
            Shape2D::Circle {
                radius: right_radius,
            },
        ) => circle_circle_overlap(left_center, left_radius, right_center, right_radius),
        (Shape2D::Circle { radius }, Shape2D::Aabb { half_extents }) => {
            circle_aabb_overlap(left_center, radius, right_center, half_extents)
        }
        (Shape2D::Aabb { half_extents }, Shape2D::Circle { radius }) => {
            circle_aabb_overlap(right_center, radius, left_center, half_extents)
        }
    }
}

fn aabb_aabb_overlap(
    left_center: Vec2,
    left_half: Vec2,
    right_center: Vec2,
    right_half: Vec2,
) -> bool {
    (left_center.x - right_center.x).abs() <= left_half.x + right_half.x
        && (left_center.y - right_center.y).abs() <= left_half.y + right_half.y
}

fn circle_circle_overlap(
    left_center: Vec2,
    left_radius: f32,
    right_center: Vec2,
    right_radius: f32,
) -> bool {
    let dx = left_center.x - right_center.x;
    let dy = left_center.y - right_center.y;
    let radius = left_radius + right_radius;
    dx * dx + dy * dy <= radius * radius
}

fn circle_aabb_overlap(
    circle_center: Vec2,
    radius: f32,
    aabb_center: Vec2,
    half_extents: Vec2,
) -> bool {
    let closest_x = circle_center.x.clamp(
        aabb_center.x - half_extents.x,
        aabb_center.x + half_extents.x,
    );
    let closest_y = circle_center.y.clamp(
        aabb_center.y - half_extents.y,
        aabb_center.y + half_extents.y,
    );
    let dx = circle_center.x - closest_x;
    let dy = circle_center.y - closest_y;
    dx * dx + dy * dy <= radius * radius
}

pub fn shape_label(shape: Shape2D) -> &'static str {
    match shape {
        Shape2D::Aabb { .. } => "aabb",
        Shape2D::Circle { .. } => "circle",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Hierarchy, Transform};
    use crate::math::Vec3;

    fn hierarchy() -> Hierarchy {
        Hierarchy {
            parent_id: None,
            sibling_order: 0,
        }
    }

    fn transform(x: f32, y: f32) -> Transform {
        Transform {
            local_position: Vec3 { x, y, z: 0.0 },
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        }
    }

    fn proxy(entity_id: &str, x: f32, y: f32, collider: Collider2D) -> Physics2DColliderProxy {
        Physics2DColliderProxy::from_transform_and_collider(
            EntityId::from(entity_id),
            &transform(x, y),
            &collider,
        )
    }

    #[test]
    fn physics_layer_mask_filters_bitwise() {
        assert!(PhysicsLayer(0b0010).matches(PhysicsMask(0b0110)));
        assert!(!PhysicsLayer(0b1000).matches(PhysicsMask(0b0110)));
    }

    #[test]
    fn physics2d_world_inserts_updates_and_removes_collider() {
        let mut world = Physics2DWorld::new();
        let entity_id = EntityId::from("entity-a");
        world.insert_or_update_collider(proxy("entity-a", 0.0, 0.0, Collider2D::aabb(Vec2::ONE)));
        assert_eq!(world.collider_count(), 1);
        world.insert_or_update_collider(proxy("entity-a", 2.0, 0.0, Collider2D::aabb(Vec2::ONE)));
        assert_eq!(world.collider(&entity_id).unwrap().world_position.x, 2.0);
        assert!(world.remove_collider(&entity_id).is_some());
        assert_eq!(world.collider_count(), 0);
    }

    #[test]
    fn physics2d_world_ignores_disabled_collider_in_queries() {
        let mut physics_world = Physics2DWorld::new();
        let mut collider = Collider2D::aabb(Vec2::ONE);
        collider.enabled = false;
        physics_world.insert_or_update_collider(proxy("entity-a", 0.0, 0.0, collider));

        let hits = physics_world.overlap_aabb(&OverlapAabb2D::new(Vec2::ZERO, Vec2::ONE));

        assert!(hits.is_empty());
    }

    #[test]
    fn physics2d_world_keeps_entity_id_order_stable() {
        let mut physics_world = Physics2DWorld::new();
        physics_world.insert_or_update_collider(proxy(
            "entity-b",
            0.0,
            0.0,
            Collider2D::aabb(Vec2::ONE),
        ));
        physics_world.insert_or_update_collider(proxy(
            "entity-a",
            0.0,
            0.0,
            Collider2D::aabb(Vec2::ONE),
        ));

        let ids = physics_world.collider_ids();

        assert_eq!(
            ids,
            vec![EntityId::from("entity-a"), EntityId::from("entity-b")]
        );
    }

    #[test]
    fn overlap_aabb_hits_intersecting_aabb() {
        let mut physics_world = Physics2DWorld::new();
        physics_world.insert_or_update_collider(proxy(
            "entity-a",
            0.5,
            0.0,
            Collider2D::aabb(Vec2::ONE),
        ));
        physics_world.insert_or_update_collider(proxy(
            "entity-b",
            4.0,
            0.0,
            Collider2D::aabb(Vec2::ONE),
        ));

        let hits = physics_world.overlap_aabb(&OverlapAabb2D::new(Vec2::ZERO, Vec2::ONE));

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entity_id, EntityId::from("entity-a"));
    }

    #[test]
    fn overlap_aabb_filters_by_layer_mask() {
        let mut physics_world = Physics2DWorld::new();
        let mut collider = Collider2D::aabb(Vec2::ONE);
        collider.layer = PhysicsLayer(0b0100);
        collider.mask = PhysicsMask(0b0010);
        physics_world.insert_or_update_collider(proxy("entity-a", 0.0, 0.0, collider));
        let mut query = OverlapAabb2D::new(Vec2::ZERO, Vec2::ONE);
        query.layer = PhysicsLayer(0b0010);
        query.mask = PhysicsMask(0b0100);

        let hits = physics_world.overlap_aabb(&query);

        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn overlap_circle_hits_circle_and_aabb() {
        let mut physics_world = Physics2DWorld::new();
        physics_world.insert_or_update_collider(proxy(
            "entity-a",
            0.5,
            0.0,
            Collider2D::circle(0.5),
        ));
        physics_world.insert_or_update_collider(proxy(
            "entity-b",
            1.4,
            0.0,
            Collider2D::aabb(Vec2 { x: 0.5, y: 0.5 }),
        ));

        let hits = physics_world.overlap_circle(&OverlapCircle2D::new(Vec2::ZERO, 1.0));

        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn overlap_query_excludes_sensor_when_requested() {
        let mut physics_world = Physics2DWorld::new();
        let mut collider = Collider2D::aabb(Vec2::ONE);
        collider.is_sensor = true;
        physics_world.insert_or_update_collider(proxy("entity-a", 0.0, 0.0, collider));
        let mut query = OverlapAabb2D::new(Vec2::ZERO, Vec2::ONE);
        query.include_sensors = false;

        let hits = physics_world.overlap_aabb(&query);

        assert!(hits.is_empty());
    }

    #[test]
    fn overlap_query_limit_applies_after_stable_order() {
        let mut physics_world = Physics2DWorld::new();
        physics_world.insert_or_update_collider(proxy(
            "entity-b",
            0.0,
            0.0,
            Collider2D::aabb(Vec2::ONE),
        ));
        physics_world.insert_or_update_collider(proxy(
            "entity-a",
            0.0,
            0.0,
            Collider2D::aabb(Vec2::ONE),
        ));
        let mut query = OverlapAabb2D::new(Vec2::ZERO, Vec2::ONE);
        query.limit = Some(1);

        let hits = physics_world.overlap_aabb(&query);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entity_id, EntityId::from("entity-a"));
    }

    #[test]
    fn collision_pair_report_detects_two_overlapping_colliders() {
        let mut physics_world = Physics2DWorld::new();
        physics_world.insert_or_update_collider(proxy(
            "entity-a",
            0.0,
            0.0,
            Collider2D::aabb(Vec2::ONE),
        ));
        physics_world.insert_or_update_collider(proxy(
            "entity-b",
            1.0,
            0.0,
            Collider2D::aabb(Vec2::ONE),
        ));

        let report = physics_world.build_collision_pairs();

        assert_eq!(report.pairs.len(), 1);
        assert_eq!(report.pairs[0].entity_a, EntityId::from("entity-a"));
        assert_eq!(report.pairs[0].entity_b, EntityId::from("entity-b"));
    }

    #[test]
    fn collision_pair_report_filters_non_matching_layers() {
        let mut physics_world = Physics2DWorld::new();
        let mut left = Collider2D::aabb(Vec2::ONE);
        left.layer = PhysicsLayer(0b0001);
        left.mask = PhysicsMask(0b0010);
        let mut right = Collider2D::aabb(Vec2::ONE);
        right.layer = PhysicsLayer(0b0100);
        right.mask = PhysicsMask(0b0001);
        physics_world.insert_or_update_collider(proxy("entity-a", 0.0, 0.0, left));
        physics_world.insert_or_update_collider(proxy("entity-b", 0.0, 0.0, right));

        let report = physics_world.build_collision_pairs();

        assert!(report.pairs.is_empty());
        assert_eq!(report.filtered_pair_count, 1);
    }

    #[test]
    fn collision_pair_report_sorts_pairs_stably() {
        let mut physics_world = Physics2DWorld::new();
        physics_world.insert_or_update_collider(proxy(
            "entity-c",
            0.0,
            0.0,
            Collider2D::aabb(Vec2::ONE),
        ));
        physics_world.insert_or_update_collider(proxy(
            "entity-a",
            0.0,
            0.0,
            Collider2D::aabb(Vec2::ONE),
        ));
        physics_world.insert_or_update_collider(proxy(
            "entity-b",
            0.0,
            0.0,
            Collider2D::aabb(Vec2::ONE),
        ));

        let report = physics_world.build_collision_pairs();
        let pairs = report
            .pairs
            .iter()
            .map(|pair| (pair.entity_a.as_str(), pair.entity_b.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(
            pairs,
            vec![
                ("entity-a", "entity-b"),
                ("entity-a", "entity-c"),
                ("entity-b", "entity-c"),
            ]
        );
    }

    #[test]
    fn collision_pair_report_marks_sensor_pair_without_auto_event() {
        let mut physics_world = Physics2DWorld::new();
        let mut collider = Collider2D::aabb(Vec2::ONE);
        collider.is_sensor = true;
        physics_world.insert_or_update_collider(proxy("entity-a", 0.0, 0.0, collider));
        physics_world.insert_or_update_collider(proxy(
            "entity-b",
            0.0,
            0.0,
            Collider2D::aabb(Vec2::ONE),
        ));

        let report = physics_world.build_collision_pairs();

        assert_eq!(report.pairs.len(), 1);
        assert!(report.pairs[0].is_sensor_pair);
    }

    #[test]
    fn physics2d_bridge_syncs_transform_and_collider_from_world() {
        let mut world = World::new();
        let entity_id = EntityId::from("entity-source");
        world.spawn_with_components(
            entity_id.clone(),
            "Source",
            "actor",
            true,
            hierarchy(),
            Some(transform(2.0, 3.0)),
            None,
        );
        world.insert_component_value(
            entity_id.clone(),
            crate::archetype::ComponentValue::Collider2D(Collider2D::aabb(Vec2::ONE)),
        );
        let mut physics_world = Physics2DWorld::new();

        let report = Physics2DBridge::sync_from_world(&world, &mut physics_world);

        assert_eq!(report.synced_colliders, 1);
        assert_eq!(
            physics_world.collider(&entity_id).unwrap().world_position,
            Vec2 { x: 2.0, y: 3.0 }
        );
    }

    #[test]
    fn physics2d_sync_report_exposes_projection_summary() {
        let mut world = World::new();
        let entity_id = EntityId::from("entity-source");
        world.spawn_with_components(
            entity_id,
            "Source",
            "actor",
            true,
            hierarchy(),
            Some(transform(2.0, 3.0)),
            None,
        );
        world.insert_component_value(
            EntityId::from("entity-source"),
            crate::archetype::ComponentValue::Collider2D(Collider2D::aabb(Vec2::ONE)),
        );
        let mut physics_world = Physics2DWorld::new();

        let report = Physics2DBridge::sync_from_world(&world, &mut physics_world);
        let projection = report.projection_summary();

        assert_eq!(projection.kind, ProjectionKind::Physics2D);
        assert_eq!(projection.source_domain, ProjectionDomain::World);
        assert_eq!(projection.target_domain, ProjectionDomain::Physics2D);
        assert_eq!(projection.projected_count, 1);
        assert_eq!(projection.error_count, 0);
    }

    #[test]
    fn physics2d_bridge_updates_proxy_after_transform_change() {
        let mut world = World::new();
        let entity_id = EntityId::from("entity-source");
        world.spawn_with_components(
            entity_id.clone(),
            "Source",
            "actor",
            true,
            hierarchy(),
            Some(transform(0.0, 0.0)),
            None,
        );
        world.insert_component_value(
            entity_id.clone(),
            crate::archetype::ComponentValue::Collider2D(Collider2D::aabb(Vec2::ONE)),
        );
        let mut physics_world = Physics2DWorld::new();
        Physics2DBridge::sync_from_world(&world, &mut physics_world);
        world.insert_transform(entity_id.clone(), transform(5.0, 0.0));

        Physics2DBridge::sync_from_world(&world, &mut physics_world);

        assert_eq!(
            physics_world.collider(&entity_id).unwrap().world_position.x,
            5.0
        );
    }

    #[test]
    fn physics2d_bridge_removes_proxy_when_collider_removed_or_entity_despawned() {
        let mut world = World::new();
        let entity_id = EntityId::from("entity-source");
        world.spawn_with_components(
            entity_id.clone(),
            "Source",
            "actor",
            true,
            hierarchy(),
            Some(transform(0.0, 0.0)),
            None,
        );
        world.insert_component_value(
            entity_id.clone(),
            crate::archetype::ComponentValue::Collider2D(Collider2D::aabb(Vec2::ONE)),
        );
        let mut physics_world = Physics2DWorld::new();
        Physics2DBridge::sync_from_world(&world, &mut physics_world);
        world.remove_component_value(&entity_id, &ComponentTypeId::collider2d());

        let report = Physics2DBridge::sync_from_world(&world, &mut physics_world);

        assert_eq!(report.removed_colliders, 1);
        assert_eq!(physics_world.collider_count(), 0);
    }

    #[test]
    fn physics2d_bridge_reports_missing_transform_for_collider() {
        let mut world = World::new();
        let entity_id = EntityId::from("entity-source");
        world.spawn_entity(entity_id.clone(), "Source", "actor", true, hierarchy());
        world.insert_component_value(
            entity_id.clone(),
            crate::archetype::ComponentValue::Collider2D(Collider2D::aabb(Vec2::ONE)),
        );
        let mut physics_world = Physics2DWorld::new();

        let report = Physics2DBridge::sync_from_world(&world, &mut physics_world);

        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "missing_transform");
    }
}
