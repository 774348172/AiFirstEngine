//! Borrowed backing observations on the original instance owner.
use super::*;
use crate::resource_memory::ResourceMemorySample;

impl PreparedPrefab {
    pub(super) fn observe_memory(&self, sample: &mut ResourceMemorySample) {
        self.prepared.observe_memory(sample);
        self.prefab_ref
            .observe_memory(sample, "queues", "queuePayload");
        for id in self.parent_entity.iter().chain(self.root_source.iter()) {
            sample.cpu_arc(id.storage(), "world", &["queuePayload"]);
        }
        sample.cpu_vec(&self.handles, "queues", &["queuePayload"]);
        for handle in &self.handles {
            handle.observe_memory(sample, "queues", "queuePayload");
        }
    }
}

fn identities(
    runtime: &BTreeMap<SourceEntityId, RuntimeEntityId>,
    world: &BTreeMap<SourceEntityId, SourceEntityId>,
    sample: &mut ResourceMemorySample,
) {
    // IDs may also be retained by World, snapshots or another instance map.
    // Keep their canonical owner so one allocation reconciles across consumers.
    for id in runtime
        .keys()
        .chain(world.iter().flat_map(|(source, world)| [source, world]))
    {
        sample.cpu_arc(id.storage(), "world", &["resident"]);
    }
}
impl RuntimeInstanceLoader {
    pub(super) fn observe_instance_memory(&self, sample: &mut ResourceMemorySample) {
        for instance in self.scene_instances.values() {
            for text in [&instance.scene_asset_guid, &instance.scene_id] {
                sample.cpu_string(text, "world", &["resident"]);
            }
            sample.cpu_vec(&instance.root_entities, "world", &["resident"]);
            identities(
                &instance.source_to_runtime_entity,
                &instance.source_to_world_entity,
                sample,
            );
            sample.cpu_vec(&instance.owned_asset_handles, "world", &["resident"]);
            for handle in &instance.owned_asset_handles {
                handle.observe_memory(sample, "world", "resident");
            }
        }
        for instance in self.prefab_instances.values() {
            sample.cpu_string(&instance.prefab_asset_guid, "world", &["resident"]);
            if let Some(parent) = &instance.parent_entity {
                sample.cpu_arc(parent.storage(), "world", &["resident"]);
            }
            identities(
                &instance.source_to_runtime_entity,
                &instance.source_to_world_entity,
                sample,
            );
            sample.cpu_vec(&instance.owned_asset_handles, "world", &["resident"]);
            for handle in &instance.owned_asset_handles {
                handle.observe_memory(sample, "world", "resident");
            }
        }
        sample.partial("cpu.owner.world", "instances.strings+rootVectors+handleBacking+sharedIdentityPayload;stdMapNodes+arcControlBlocksOpaque");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_memory::ResourceMemorySample;
    use crate::runtime_asset::{RuntimeAssetIndex, RuntimePackageMountTable};

    fn text(value: &str) -> String {
        let mut result = String::with_capacity(4096);
        result.push_str(value);
        result
    }
    fn allocation(value: &String) -> (String, u64) {
        (
            format!("cpu-storage:{:x}", value.as_ptr() as usize),
            value.capacity() as u64,
        )
    }
    fn source(value: &SourceEntityId) -> (String, u64) {
        (
            format!("cpu-storage:{:x}", value.as_str().as_ptr() as usize),
            value.as_str().len() as u64,
        )
    }
    fn loader() -> RuntimeInstanceLoader {
        RuntimeInstanceLoader::new(RuntimeAssetLoader::new(
            "unused",
            RuntimeAssetIndex::new(vec![], vec![]),
            RuntimePackageMountTable::new(vec![]),
        ))
    }
    fn assert_present(sample: &ResourceMemorySample, expected: &[(String, u64)]) {
        for (id, bytes) in expected {
            assert!(
                sample
                    .allocations
                    .iter()
                    .any(|a| a.allocation_id == *id && a.bytes == *bytes),
                "missing instance backing {id}: {bytes} bytes"
            );
        }
        let mut unique = BTreeMap::new();
        for a in &sample.allocations {
            if let Some(old) = unique.insert(&a.allocation_id, (&a.owner, a.bytes)) {
                assert_eq!(old, (&a.owner, a.bytes), "conflicting aliased backing");
            }
        }
    }

    #[test]
    fn instance_memory_tracks_real_scene_prefab_backing_and_shared_ids_until_owner_release() {
        let mut loader = loader();
        let scene_id = text("scene");
        let prefab_guid = text("prefab-guid");
        let shared = SourceEntityId::from("shared-identity".repeat(256));
        let world_id = SourceEntityId::from("world-identity".repeat(256));
        let root_entities = Vec::<RuntimeEntityId>::with_capacity(64);
        let expected = vec![
            allocation(&scene_id),
            allocation(&prefab_guid),
            source(&shared),
            source(&world_id),
            (
                format!("cpu-storage:{:x}", root_entities.as_ptr() as usize),
                (root_entities.capacity() * std::mem::size_of::<RuntimeEntityId>()) as u64,
            ),
        ];
        loader.scene_instances.insert(
            RuntimeInstanceId(1),
            RuntimeSceneInstance {
                instance_id: RuntimeInstanceId(1),
                scene_asset_guid: text("scene-guid"),
                scene_id,
                root_entities,
                source_to_runtime_entity: [(shared.clone(), RuntimeEntityId::new(1, 1))].into(),
                source_to_world_entity: [(shared.clone(), world_id.clone())].into(),
                owned_asset_handles: vec![],
                state: RuntimeInstanceState::Active,
            },
        );
        loader.prefab_instances.insert(
            RuntimeInstanceId(2),
            RuntimePrefabInstance {
                instance_id: RuntimeInstanceId(2),
                prefab_asset_guid: prefab_guid,
                root_entity: None,
                parent_entity: Some(shared.clone()),
                target_scene_instance: Some(RuntimeInstanceId(1)),
                source_to_runtime_entity: [(shared.clone(), RuntimeEntityId::new(2, 1))].into(),
                source_to_world_entity: [(shared.clone(), world_id.clone())].into(),
                owned_asset_handles: vec![],
                state: RuntimeInstanceState::Active,
            },
        );
        let mut sample = ResourceMemorySample::new(1, 1, 0);
        loader.observe_memory(&mut sample);
        assert_present(&sample, &expected);
        loader.scene_instances.clear();
        let mut prefab_only = ResourceMemorySample::new(2, 1, 0);
        loader.observe_memory(&mut prefab_only);
        assert_present(&prefab_only, &expected[1..4]);
        assert!(!prefab_only
            .allocations
            .iter()
            .any(|a| a.allocation_id == expected[0].0));
        loader.prefab_instances.clear();
        let mut empty = ResourceMemorySample::new(3, 1, 0);
        loader.observe_memory(&mut empty);
        for (id, _) in expected {
            assert!(!empty.allocations.iter().any(|a| a.allocation_id == id));
        }
    }
}
