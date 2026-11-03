use super::*;
use crate::resource_memory::{memory_totals, MemoryCoverage, ResourceMemorySample};

// Record the actual allocations before moving their owners through preparation
// and commit. Content length deliberately differs from allocation capacity.
fn fixture(capacity: usize, populated: bool) -> (PreparedRuntimeEntities, Vec<(String, u64)>) {
    let mut expected = Vec::new();
    let mut text = |content: &str| {
        let mut value = String::with_capacity(capacity);
        value.push_str(content);
        expected.push((
            format!("cpu-storage:{:x}", value.as_ptr() as usize),
            value.capacity() as u64,
        ));
        value
    };
    let renderable = ComponentValue::Renderable(Renderable {
        mesh_ref: populated.then(|| text("mesh")),
        material_ref: populated.then(|| text("material")),
        visible: true,
        // Even an empty String can own a nonempty backing allocation.
        layer: text(if populated { "main" } else { "" }),
    });
    let sprite = ComponentValue::SpriteRenderer2D(SpriteRenderer2D {
        sprite_ref: populated.then(|| text("sprite")),
        material_ref: populated.then(|| text("sprite-material")),
        ..Default::default()
    });
    let animator = ComponentValue::Animator2D(crate::components::Animator2D {
        controller_id: text("controller"),
        controller_index: 0,
        registry_digest: text("digest"),
        enabled: true,
        initial_bools: if populated {
            [(text("moving"), true)].into()
        } else {
            BTreeMap::new()
        },
    });
    let mut asset_ref = |kind: &str| crate::runtime_package::RuntimeAssetRef {
        id: text(kind),
        asset_type: text(kind),
        guid: populated.then(|| text("guid")),
        sub_asset: populated.then(|| text("subasset")),
    };
    let audio = ComponentValue::AudioSource(crate::audio::AudioSource {
        clip_ref: asset_ref("audio"),
        volume: 1.,
    });
    let effect_ref = asset_ref("particle-effect");
    let particle = ComponentValue::ParticleEffect(crate::runtime_particles::ParticleEffect {
        effect_ref,
        play_on_awake: true,
        paused: false,
        parameters: if populated {
            [(
                text("intensity"),
                crate::particle_effect::ParticleValue::Float(1.),
            )]
            .into()
        } else {
            BTreeMap::new()
        },
    });
    let id = SourceEntityId::from("typed-memory-entity");
    (
        PreparedRuntimeEntities {
            entities: vec![PreparedRuntimeEntity {
                source_id: id.clone(),
                world_id: id.clone(),
                name: "typed".into(),
                kind: "actor".into(),
                enabled: true,
                hierarchy: Hierarchy {
                    parent_id: None,
                    sibling_order: 0,
                },
                components: vec![renderable, sprite, animator, audio, particle],
            }],
            source_to_world: [(id.clone(), id.clone())].into(),
            root_sources: vec![id],
            remapped_reference_count: 0,
        },
        expected,
    )
}

fn assert_backing(
    sample: &ResourceMemorySample,
    expected: &[(String, u64)],
    owner: &str,
    role: &str,
) {
    for (id, bytes) in expected {
        assert!(
            sample.allocations.iter().any(|a| {
                a.allocation_id == *id
                    && a.bytes == *bytes
                    && a.owner == owner
                    && a.roles.contains(role)
            }),
            "missing typed nested backing {id}, capacity {bytes}, owner {owner}, role {role}"
        );
    }
}

#[test]
fn prepared_typed_nested_capacity_is_observed_without_retaining_storage() {
    for capacity in [256, 4096] {
        for populated in [false, true] {
            let (prepared, expected) = fixture(capacity, populated);
            let mut sample = ResourceMemorySample::new(1, 1, 0);
            prepared.observe_memory(&mut sample);
            assert_backing(&sample, &expected, "queues", "queuePayload");
            assert_eq!(sample.coverage["cpu.owner.queues"], MemoryCoverage::Partial);
            let totals = memory_totals(&sample.allocations).unwrap();
            prepared.observe_memory(&mut sample);
            assert_eq!(memory_totals(&sample.allocations).unwrap(), totals);
            drop(prepared);
            let mut after = ResourceMemorySample::new(2, 1, 0);
            PreparedRuntimeEntities {
                entities: vec![],
                source_to_world: BTreeMap::new(),
                root_sources: vec![],
                remapped_reference_count: 0,
            }
            .observe_memory(&mut after);
            assert!(after.allocations.is_empty());
        }
    }
}

#[test]
fn committed_typed_nested_capacity_moves_to_world_and_leaves_on_despawn() {
    for capacity in [256, 4096] {
        for populated in [false, true] {
            let (prepared, expected) = fixture(capacity, populated);
            let id = prepared.entities[0].world_id.clone();
            let mut world = World::new();
            prepared.commit(&mut world);
            let mut sample = ResourceMemorySample::new(1, 1, 0);
            world.observe_memory(&mut sample);
            assert_backing(&sample, &expected, "world", "resident");
            assert_eq!(sample.coverage["cpu.owner.world"], MemoryCoverage::Partial);
            let totals = memory_totals(&sample.allocations).unwrap();
            world.observe_memory(&mut sample);
            assert_eq!(memory_totals(&sample.allocations).unwrap(), totals);
            world.try_despawn_entity(&id).unwrap();
            let mut after = ResourceMemorySample::new(2, 1, 0);
            world.observe_memory(&mut after);
            assert!(
                expected
                    .iter()
                    .all(|(id, _)| { after.allocations.iter().all(|a| a.allocation_id != *id) }),
                "despawned typed payload is still reported"
            );
        }
    }
}
