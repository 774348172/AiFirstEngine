//! Borrowed storage census shared by decoded-cache and worker completion owners.
//! No serialization estimates, allocator overhead or retained resource leases.
use super::*;
use crate::{resource_memory::ResourceMemorySample, runtime_package::*};

impl RuntimeAssetHandle {
    pub(crate) fn observe_memory(&self, s: &mut ResourceMemorySample, owner: &str, role: &str) {
        for value in [
            &self.asset_guid,
            &self.asset_id,
            &self.asset_type,
            &self.cooked_asset_id,
            &self.bundle_id,
            &self.loader_kind,
            &self.version,
        ]
        .into_iter()
        .chain(
            [&self.sub_asset_id, &self.runtime_resource_id]
                .into_iter()
                .flatten(),
        ) {
            s.cpu_string(value, owner, &[role]);
        }
    }
}

impl RuntimeAssetRef {
    pub(crate) fn observe_memory(&self, s: &mut ResourceMemorySample, owner: &str, role: &str) {
        for value in [&self.id, &self.asset_type]
            .into_iter()
            .chain(self.guid.iter())
            .chain(self.sub_asset.iter())
        {
            s.cpu_string(value, owner, &[role]);
        }
    }
}

impl RuntimeAssetRecord {
    pub(crate) fn observe_memory(&self, s: &mut ResourceMemorySample, owner: &str, role: &str) {
        for v in [
            &self.asset_guid,
            &self.asset_id,
            &self.asset_type,
            &self.version,
            &self.cooked_asset_id,
            &self.bundle_id,
            &self.loader_kind,
        ]
        .into_iter()
        .chain(
            [&self.sub_asset_id, &self.hash, &self.source_map_debug]
                .into_iter()
                .flatten(),
        ) {
            s.cpu_string(v, owner, &[role]);
        }
        for values in [&self.dependencies, &self.flags] {
            s.cpu_vec(values, owner, &[role]);
            for value in values {
                s.cpu_string(value, owner, &[role]);
            }
        }
    }
}
impl CookedAssetRecord {
    pub(crate) fn observe_memory(&self, s: &mut ResourceMemorySample, owner: &str, role: &str) {
        for v in [&self.cooked_asset_id, &self.bundle_id].into_iter().chain(
            [&self.path, &self.compression, &self.hash]
                .into_iter()
                .flatten(),
        ) {
            s.cpu_string(v, owner, &[role]);
        }
    }
}
impl RuntimeAssetIndex {
    pub(crate) fn observe_memory(&self, s: &mut ResourceMemorySample) {
        for (key, record) in &self.records_by_guid {
            s.cpu_string(key, "assets", &["resident"]);
            record.observe_memory(s, "assets", "resident");
        }
        for (id, guid) in &self.guid_by_id {
            strings(s, [id, guid], "resident");
        }
        for (key, record) in &self.cooked_assets {
            s.cpu_string(key, "assets", &["resident"]);
            record.observe_memory(s, "assets", "resident");
        }
    }
}
impl RuntimePackageMountTable {
    pub(crate) fn observe_memory(&self, s: &mut ResourceMemorySample) {
        for (key, b) in &self.bundles {
            strings(
                s,
                [key, &b.bundle_id, &b.uri]
                    .into_iter()
                    .chain([&b.mount_id, &b.hash, &b.version].into_iter().flatten()),
                "resident",
            );
        }
        strings(s, &self.mounted, "resident");
    }
}

impl DecodedAsset {
    pub(crate) fn observe_memory(&self, s: &mut ResourceMemorySample, role: &str) {
        for v in [&self.cooked_asset_id, &self.asset_type, &self.source_debug] {
            s.cpu_string(v, "assets", &[role]);
        }
        if let Some(clip) = &self.audio_clip {
            s.cpu_arc(clip, "assets", &[role]);
            s.cpu_arc(&clip.samples, "assets", &[role]);
        }
        if let Some(bytes) = &self.bytes {
            s.cpu_arc(bytes, "assets", &[role]);
        }
        if let Some(payload) = &self.payload {
            s.cpu_arc(payload, "assets", &[role]);
            match payload.as_ref() {
                RuntimeDecodedPayload::Texture(texture) => {
                    let t = texture.payload();
                    s.cpu_arc(t, "assets", &[role]);
                    t.observe_memory(s, role);
                }
                RuntimeDecodedPayload::Render(render) => {
                    strings(s, [&render.source_digest, &render.recipe], role);
                    match &render.asset {
                        crate::render_contract::RenderAssetData::Mesh(mesh) => {
                            s.cpu_vec(&mesh.vertex_bytes, "assets", &[role]);
                            s.cpu_vec(&mesh.index_bytes, "assets", &[role]);
                            s.cpu_vec(&mesh.attributes, "assets", &[role]);
                            s.cpu_vec(&mesh.submeshes, "assets", &[role]);
                            string_vec(s, &mesh.material_slots, role);
                            strings(s, mesh.submeshes.iter().map(|v| &v.material_slot), role);
                        }
                        crate::render_contract::RenderAssetData::Material(m) => {
                            s.cpu_string(&m.shader_source, "assets", &[role]);
                            strings(
                                s,
                                m.parameters.keys().chain(m.texture_sampling.keys()),
                                role,
                            );
                            for (key, value) in &m.textures {
                                s.cpu_string(key, "assets", &[role]);
                                asset_ref(s, &value.reference, role);
                            }
                        }
                    }
                }
                RuntimeDecodedPayload::Environment(map) => {
                    s.cpu_arc(map, "assets", &[role]);
                    map.observe_memory(s, role);
                }
                RuntimeDecodedPayload::Prefab(p) => {
                    strings(
                        s,
                        [&p.schema_version, &p.id, &p.name]
                            .into_iter()
                            .chain(p.root_entity_id.iter()),
                        role,
                    );
                    entities(s, &p.entities, role);
                }
                RuntimeDecodedPayload::Scene(scene) => {
                    strings(
                        s,
                        [
                            &scene.schema_version,
                            &scene.id,
                            &scene.name,
                            &scene.background,
                            &scene.sky_color,
                        ]
                        .into_iter()
                        .chain(scene.main_camera.iter()),
                        role,
                    );
                    entities(s, &scene.entities, role);
                }
            }
        }
    }
}

fn strings<'a>(
    s: &mut ResourceMemorySample,
    values: impl IntoIterator<Item = &'a String>,
    role: &str,
) {
    for v in values {
        s.cpu_string(v, "assets", &[role]);
    }
}
fn string_vec(s: &mut ResourceMemorySample, values: &Vec<String>, role: &str) {
    s.cpu_vec(values, "assets", &[role]);
    strings(s, values, role);
}
fn asset_ref(s: &mut ResourceMemorySample, r: &RuntimeAssetRef, role: &str) {
    r.observe_memory(s, "assets", role);
}
pub(crate) fn json(s: &mut ResourceMemorySample, value: &serde_json::Value, role: &str) {
    match value {
        serde_json::Value::String(v) => s.cpu_string(v, "assets", &[role]),
        serde_json::Value::Array(v) => {
            s.cpu_vec(v, "assets", &[role]);
            for item in v {
                json(s, item, role);
            }
        }
        serde_json::Value::Object(v) => {
            for (key, item) in v {
                s.cpu_string(key, "assets", &[role]);
                json(s, item, role);
            }
        }
        _ => {}
    }
}
fn entities(s: &mut ResourceMemorySample, values: &Vec<RuntimeEntity>, role: &str) {
    s.cpu_vec(values, "assets", &[role]);
    for e in values {
        strings(
            s,
            [&e.schema_version, &e.id, &e.name, &e.kind]
                .into_iter()
                .chain(e.parent_id.iter()),
            role,
        );
        s.cpu_vec(&e.components, "assets", &[role]);
        for c in &e.components {
            s.cpu_string(&c.component_type, "assets", &[role]);
            json(s, &c.data, role);
        }
        if let Some(mesh) = &e.mesh {
            strings(
                s,
                [&mesh.layer]
                    .into_iter()
                    .chain(mesh.primitive.iter())
                    .chain(mesh.color.iter())
                    .chain(mesh.label.iter()),
                role,
            );
            for r in [&mesh.asset_ref, &mesh.material_ref, &mesh.texture_ref]
                .into_iter()
                .flatten()
            {
                asset_ref(s, r, role);
            }
        }
        if let Some(sprite) = &e.sprite_renderer2d {
            for r in [&sprite.sprite_ref, &sprite.material_ref]
                .into_iter()
                .flatten()
            {
                asset_ref(s, r, role);
            }
        }
        if let Some(a) = &e.animator2d {
            strings(
                s,
                [&a.controller_id, &a.registry_digest]
                    .into_iter()
                    .chain(a.initial_bools.keys()),
                role,
            );
        }
        if let Some(crate::spatial_contract::RuntimeSpatial::World3D {
            visibility,
            mesh_renderer,
            camera,
            light,
            ..
        }) = &e.spatial
        {
            if let Some(v) = visibility {
                s.cpu_string(&v.schema_version, "assets", &[role]);
            }
            if let Some(m) = mesh_renderer {
                s.cpu_string(&m.schema_version, "assets", &[role]);
                asset_ref(s, &m.mesh.0, role);
                string_vec(s, &m.layers, role);
                for (key, r) in &m.materials {
                    s.cpu_string(key, "assets", &[role]);
                    asset_ref(s, &r.0, role);
                }
            }
            if let Some(c) = camera {
                s.cpu_string(&c.schema_version, "assets", &[role]);
                string_vec(s, &c.layers, role);
                if let Some(p) = &c.color_pipeline {
                    p.observe_memory(s, "assets", role);
                }
            }
            if let Some(l) = light {
                s.cpu_string(&l.schema_version, "assets", &[role]);
                string_vec(s, &l.layers, role);
            }
        }
    }
}
