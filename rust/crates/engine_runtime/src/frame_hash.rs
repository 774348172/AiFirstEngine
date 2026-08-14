use crate::render_snapshot::RenderSnapshot;
use crate::world::World;

pub fn hash_frame(scene_id: &str, frame: u64, world: &World, snapshot: &RenderSnapshot) -> String {
    let mut text = String::new();
    text.push_str("scene=");
    text.push_str(scene_id);
    text.push_str(";frame=");
    text.push_str(&frame.to_string());
    text.push_str(";entities=");
    for id in world.entity_ids() {
        text.push_str(id.as_str());
        text.push('|');
    }
    text.push_str(";renderables=");
    for renderable in &snapshot.renderables {
        text.push_str(&renderable.entity_id);
        text.push(':');
        push_f32(&mut text, renderable.local_position.x);
        push_f32(&mut text, renderable.local_position.y);
        push_f32(&mut text, renderable.local_position.z);
        text.push(':');
        text.push_str(renderable.mesh_ref.as_deref().unwrap_or(""));
        text.push(':');
        text.push_str(renderable.material_ref.as_deref().unwrap_or(""));
        text.push(':');
        text.push_str(if renderable.visible { "1" } else { "0" });
        text.push(':');
        text.push_str(&renderable.layer);
        text.push('|');
    }
    fnv1a32(&text)
}

fn push_f32(text: &mut String, value: f32) {
    text.push_str(&format!("{:.6},", value));
}

fn fnv1a32(text: &str) -> String {
    let mut hash: u32 = 2166136261;
    for byte in text.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    format!("{:08x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Hierarchy, Renderable, Transform};
    use crate::ids::EntityId;
    use crate::render_snapshot::extract_render_snapshot;

    fn world_fixture() -> World {
        let mut world = World::new();
        world.spawn_with_components(
            EntityId::from("entity-player"),
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
        world
    }

    #[test]
    fn frame_hash_stays_stable_with_archetype_world_storage() {
        let world = world_fixture();
        let first_snapshot = extract_render_snapshot("scene-main", 1, &world);
        let second_snapshot = extract_render_snapshot("scene-main", 1, &world);
        let first_hash = hash_frame("scene-main", 1, &world, &first_snapshot);
        let second_hash = hash_frame("scene-main", 1, &world, &second_snapshot);
        assert_eq!(first_hash, second_hash);
        assert!(!first_hash.is_empty());
    }
}
