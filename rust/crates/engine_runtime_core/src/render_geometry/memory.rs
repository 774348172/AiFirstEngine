//! Direct immutable descriptor backing shared by snapshot and render consumers.
use super::*;
use crate::resource_memory::ResourceMemorySample;
pub(super) fn binding(s: &mut ResourceMemorySample, b: &Arc<RenderBindingSet>, role: &str) {
    s.cpu_arc(b, "snapshots", &[role]);
    s.cpu_vec(&b.slots, "snapshots", &[role]);
    for v in [&b.binding_id, &b.sampler, &b.debug_label] {
        s.cpu_string(v, "snapshots", &[role]);
    }
    for slot in &b.slots {
        s.cpu_string(&slot.name, "snapshots", &[role]);
    }
}
fn provenance(s: &mut ResourceMemorySample, p: &Arc<Provenance>, role: &str) {
    s.cpu_arc(p, "snapshots", &[role]);
    provenance_fields(s, p, role);
}
pub(super) fn provenance_fields(s: &mut ResourceMemorySample, p: &Provenance, role: &str) {
    for v in [
        &p.source_entity_id,
        &p.source_component,
        &p.source_path,
        &p.project_revision,
        &p.operation_id,
        &p.asset_id,
        &p.content_digest,
    ]
    .into_iter()
    .flatten()
    {
        s.cpu_string(v, "snapshots", &[role]);
    }
}
pub(super) fn asset_reference(
    s: &mut ResourceMemorySample,
    r: &crate::runtime_package::RuntimeAssetRef,
    role: &str,
) {
    for value in [&r.id, &r.asset_type].into_iter().chain(
        [r.guid.as_ref(), r.sub_asset.as_ref()]
            .into_iter()
            .flatten(),
    ) {
        s.cpu_string(value, "snapshots", &[role]);
    }
}
impl PreparedGeometry {
    pub(crate) fn observe_memory(&self, s: &mut ResourceMemorySample, role: &str) {
        self.buffers.observe_memory(s, role);
        s.cpu_arc(&self.buffers, "assets", &["decodedCache", role]);
        binding(s, &self.geometry_binding, role);
        binding(s, &self.material_binding, role);
        provenance(s, &self.geometry_source, role);
        provenance(s, &self.material_source, role);
        s.cpu_string(&self.material.shader_source, "snapshots", &[role]);
        s.cpu_string(&self.submesh.material_slot, "snapshots", &[role]);
        for key in self.material.parameters.keys() {
            s.cpu_string(key, "snapshots", &[role]);
        }
        for (key, value) in &self.material.textures {
            s.cpu_string(key, "snapshots", &[role]);
            for v in [&value.reference.id, &value.reference.asset_type] {
                s.cpu_string(v, "snapshots", &[role]);
            }
            for v in [&value.reference.guid, &value.reference.sub_asset]
                .into_iter()
                .flatten()
            {
                s.cpu_string(v, "snapshots", &[role]);
            }
        }
    }
}
pub(super) fn instance_backing(
    s: &mut ResourceMemorySample,
    value: &Arc<DrawInstance>,
    role: &str,
) {
    s.cpu_arc(value, "snapshots", &[role]);
    s.cpu_arc(&value.resources, "snapshots", &[role]);
    s.cpu_arc(&value.geometry, "assets", &[role]);
    value.geometry.observe_memory(s, role);
    binding(s, &value.geometry_binding, role);
    binding(s, &value.material_binding, role);
    for p in [
        &value.geometry_source,
        &value.material_source,
        &value.provenance,
    ] {
        provenance(s, p, role);
    }
}
impl DrawGeometry {
    pub(crate) fn observe_memory(&self, s: &mut ResourceMemorySample, role: &str) {
        if let Some(command) = &self.command {
            s.cpu_arc(command, "snapshots", &[role]);
            command.observe_memory(s, role);
        }
        instance_backing(s, &self.instance, role);
        s.cpu_arc(&self.view, "snapshots", &[role]);
        if let Some(pipeline) = &self.view.color_pipeline {
            pipeline.observe_memory(s, "snapshots", role);
        }
        if let Some(e) = &self.environment {
            s.cpu_arc(e, "snapshots", &[role]);
            s.cpu_arc(&e.map, "assets", &[role]);
            e.map.observe_memory(s, role);
        }
        if let Some(l) = &self.lighting {
            s.cpu_arc(l, "snapshots", &[role]);
            l.observe_memory(s, role);
        }
        if let Some(shadows) = &self.shadows {
            s.cpu_arc(shadows, "snapshots", &[role]);
            shadows.observe_memory(s, role);
        }
        s.cpu_arc(&self.target, "snapshots", &[role]);
        if let Some(v) = &self.depth_target {
            s.cpu_arc(v, "snapshots", &[role]);
        }
    }
}
impl GeometryFrame {
    pub(crate) fn observe_memory(&self, s: &mut ResourceMemorySample) {
        s.cpu_string(&self.target, "snapshots", &["snapshot"]);
        s.cpu_vec(&self.views, "snapshots", &["snapshot"]);
        let mut tables = std::collections::HashSet::new();
        for view in &self.views {
            if let Some(pipeline) = &view.view.color_pipeline {
                pipeline.observe_memory(s, "snapshots", "snapshot");
            }
            if let Some(target) = &view.color_target {
                s.cpu_string(&target.target_id, "snapshots", &["snapshot"]);
                s.cpu_string(&target.format, "snapshots", &["snapshot"]);
                s.cpu_string(&target.color_space, "snapshots", &["snapshot"]);
            }
            s.cpu_vec(&view.draws, "snapshots", &["snapshot"]);
            if let Some(batches) = &view.batches {
                s.cpu_vec(batches, "snapshots", &["snapshot"]);
                for batch in batches {
                    s.cpu_arc(batch, "snapshots", &["snapshot"]);
                    batch.observe_memory(s, "snapshot");
                    if tables.insert(Arc::as_ptr(&batch.instances)) {
                        for instance in batch.instances.iter() {
                            instance_backing(s, instance, "snapshot");
                        }
                    }
                }
            }
            s.cpu_vec(&view.trace, "snapshots", &["snapshot"]);
            if let Some(v) = &view.depth_target {
                s.cpu_string(v, "snapshots", &["snapshot"]);
            }
            for draw in &view.draws {
                s.cpu_arc(draw, "snapshots", &["snapshot"]);
                draw.observe_memory(s, "snapshot");
            }
        }
    }
}

impl GeometryBatch {
    pub(crate) fn observe_memory(&self, s: &mut ResourceMemorySample, role: &str) {
        s.cpu_string(&self.schema_version, "snapshots", &[role]);
        s.cpu_arc(&self.instances, "snapshots", &[role]);
        s.cpu_vec(&self.instances, "snapshots", &[role]);
        self.visible.observe_memory(s, role);
        s.cpu_arc(&self.draw, "snapshots", &[role]);
        self.draw.observe_memory(s, role);
    }
}
