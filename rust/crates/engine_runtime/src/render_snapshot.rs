use crate::math::Vec3;
use crate::world::World;

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSnapshot {
    pub scene_id: String,
    pub frame: u64,
    pub renderables: Vec<RenderSnapshotRenderable>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSnapshotRenderable {
    pub entity_id: String,
    pub local_position: Vec3,
    pub local_rotation: Vec3,
    pub local_scale: Vec3,
    pub mesh_ref: Option<String>,
    pub material_ref: Option<String>,
    pub visible: bool,
    pub layer: String,
}

pub fn extract_render_snapshot(
    scene_id: impl Into<String>,
    frame: u64,
    world: &World,
) -> RenderSnapshot {
    let renderables = world
        .alive_renderables()
        .into_iter()
        .map(
            |(entity_id, transform, renderable)| RenderSnapshotRenderable {
                entity_id: entity_id.as_str().to_string(),
                local_position: transform.local_position,
                local_rotation: transform.local_rotation,
                local_scale: transform.local_scale,
                mesh_ref: renderable.mesh_ref.clone(),
                material_ref: renderable.material_ref.clone(),
                visible: renderable.visible,
                layer: renderable.layer.clone(),
            },
        )
        .collect();
    RenderSnapshot {
        scene_id: scene_id.into(),
        frame,
        renderables,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Hierarchy, Renderable, Transform};
    use crate::ids::EntityId;

    #[test]
    fn render_snapshot_compatibility_output_uses_source_entity_id() {
        let mut world = World::new();
        let id = EntityId::from("entity-player");
        world.spawn_with_components(
            id,
            "Player",
            "player",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
            Some(Transform::identity()),
            Some(Renderable {
                mesh_ref: Some("model-player".to_string()),
                material_ref: Some("material-player".to_string()),
                visible: true,
                layer: "default".to_string(),
            }),
        );
        let snapshot = extract_render_snapshot("scene-main", 1, &world);
        assert_eq!(snapshot.renderables.len(), 1);
        assert_eq!(snapshot.renderables[0].entity_id, "entity-player");
        assert_eq!(
            snapshot.renderables[0].mesh_ref.as_deref(),
            Some("model-player")
        );
    }
}
