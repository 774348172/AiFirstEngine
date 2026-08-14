use engine_runtime::ids::EntityId;
use engine_runtime::physics2d::Shape2D;
use engine_runtime::world::World;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntitySelectionSource {
    AuthoringScene,
    OpenedRuntimePackage,
    ActiveGameViewRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectorContextAnchor {
    AuthoringEntity {
        entity_id: String,
    },
    RuntimeEntity {
        entity_id: String,
        source: EntitySelectionSource,
    },
}

impl EntitySelectionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthoringScene => "authoring_scene",
            Self::OpenedRuntimePackage => "opened_runtime_package",
            Self::ActiveGameViewRuntime => "active_game_view_runtime",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePickStatus {
    Hit,
    Miss,
    BlockedByAui,
    Unsupported,
}

impl RuntimePickStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Miss => "miss",
            Self::BlockedByAui => "blocked_by_aui",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeWorldPickRequest {
    pub x: f32,
    pub y: f32,
    pub viewport_width: Option<f32>,
    pub viewport_height: Option<f32>,
    pub aui_consumed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorldPickReport {
    pub status: RuntimePickStatus,
    pub selected_entity_id: Option<String>,
    pub candidate_count: usize,
    pub diagnostic: String,
}

#[derive(Debug, Clone, PartialEq)]
struct RuntimePickCandidate {
    entity_id: String,
    sorting_layer: i16,
    order_in_layer: i32,
    sort_z: f32,
    fallback_order: String,
}

pub struct WorldPickCollector;

impl WorldPickCollector {
    pub fn pick(world: &World, request: RuntimeWorldPickRequest) -> RuntimeWorldPickReport {
        if request.aui_consumed {
            return RuntimeWorldPickReport {
                status: RuntimePickStatus::BlockedByAui,
                selected_entity_id: None,
                candidate_count: 0,
                diagnostic: "blocked_by_aui".to_string(),
            };
        }

        let (world_x, world_y) = pointer_to_world_2d(request);
        let mut candidate_count = 0;
        let mut supported_count = 0;
        let mut candidates = Vec::new();
        let mut used_fallback_order = false;

        for entity_id in world.entity_ids() {
            let Some(meta) = world.entity(entity_id) else {
                continue;
            };
            if !meta.alive || !meta.enabled {
                continue;
            }
            let Some(transform) = world.transform(entity_id) else {
                continue;
            };

            let mut supported = false;
            let mut hit = false;
            if let Some(collider) = world.collider2d(entity_id) {
                if collider.enabled {
                    supported = true;
                    hit = collider_contains(
                        transform.local_position.x + collider.offset.x,
                        transform.local_position.y + collider.offset.y,
                        world_x,
                        world_y,
                        collider.shape,
                    );
                }
            }

            if !supported {
                if let Some(sprite) = world.sprite_renderer2d(entity_id) {
                    if sprite.visible {
                        supported = true;
                        hit = aabb_contains(
                            transform.local_position.x,
                            transform.local_position.y,
                            fallback_half_extent(transform.local_scale.x),
                            fallback_half_extent(transform.local_scale.y),
                            world_x,
                            world_y,
                        );
                    }
                }
            }

            if !supported {
                if let Some(renderable) = world.renderable(entity_id) {
                    if renderable.visible {
                        supported = true;
                        hit = aabb_contains(
                            transform.local_position.x,
                            transform.local_position.y,
                            fallback_half_extent(transform.local_scale.x),
                            fallback_half_extent(transform.local_scale.y),
                            world_x,
                            world_y,
                        );
                    }
                }
            }

            if !supported {
                continue;
            }
            supported_count += 1;
            if !hit {
                continue;
            }

            candidate_count += 1;
            let (sorting_layer, order_in_layer, sort_z, fallback_order) =
                if let Some(sprite) = world.sprite_renderer2d(entity_id) {
                    (
                        sprite.sorting_layer,
                        sprite.order_in_layer,
                        sprite.sort_z,
                        entity_id.as_str().to_string(),
                    )
                } else {
                    used_fallback_order = true;
                    (
                        0,
                        0,
                        transform.local_position.z,
                        entity_id.as_str().to_string(),
                    )
                };
            candidates.push(RuntimePickCandidate {
                entity_id: entity_id.as_str().to_string(),
                sorting_layer,
                order_in_layer,
                sort_z,
                fallback_order,
            });
        }

        if candidates.is_empty() {
            if supported_count == 0 {
                return RuntimeWorldPickReport {
                    status: RuntimePickStatus::Unsupported,
                    selected_entity_id: None,
                    candidate_count: 0,
                    diagnostic: "no_supported_runtime_bounds".to_string(),
                };
            }
            return RuntimeWorldPickReport {
                status: RuntimePickStatus::Miss,
                selected_entity_id: None,
                candidate_count: 0,
                diagnostic: format!("miss world_x={world_x:.3} world_y={world_y:.3}"),
            };
        }

        candidates.sort_by(|left, right| {
            right
                .sorting_layer
                .cmp(&left.sorting_layer)
                .then_with(|| right.order_in_layer.cmp(&left.order_in_layer))
                .then_with(|| {
                    right
                        .sort_z
                        .partial_cmp(&left.sort_z)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| left.fallback_order.cmp(&right.fallback_order))
        });
        let selected = candidates
            .first()
            .map(|candidate| candidate.entity_id.clone());
        RuntimeWorldPickReport {
            status: RuntimePickStatus::Hit,
            selected_entity_id: selected,
            candidate_count,
            diagnostic: if used_fallback_order {
                "pick_order=fallback".to_string()
            } else {
                "pick_order=sprite_renderer2d".to_string()
            },
        }
    }
}

fn pointer_to_world_2d(request: RuntimeWorldPickRequest) -> (f32, f32) {
    let width = request.viewport_width.unwrap_or(800.0).max(1.0);
    let height = request.viewport_height.unwrap_or(600.0).max(1.0);
    let pixels_per_world_unit = 40.0;
    (
        (request.x - width * 0.5) / pixels_per_world_unit,
        (height * 0.5 - request.y) / pixels_per_world_unit,
    )
}

fn collider_contains(
    center_x: f32,
    center_y: f32,
    point_x: f32,
    point_y: f32,
    shape: Shape2D,
) -> bool {
    match shape {
        Shape2D::Aabb { half_extents } => aabb_contains(
            center_x,
            center_y,
            half_extents.x,
            half_extents.y,
            point_x,
            point_y,
        ),
        Shape2D::Circle { radius } => {
            let dx = point_x - center_x;
            let dy = point_y - center_y;
            (dx * dx) + (dy * dy) <= radius * radius
        }
    }
}

fn aabb_contains(
    center_x: f32,
    center_y: f32,
    half_x: f32,
    half_y: f32,
    point_x: f32,
    point_y: f32,
) -> bool {
    point_x >= center_x - half_x
        && point_x <= center_x + half_x
        && point_y >= center_y - half_y
        && point_y <= center_y + half_y
}

fn fallback_half_extent(scale: f32) -> f32 {
    (scale.abs() * 0.5).max(0.5)
}

#[allow(dead_code)]
fn _entity_id(value: impl Into<String>) -> EntityId {
    EntityId::new(value)
}
