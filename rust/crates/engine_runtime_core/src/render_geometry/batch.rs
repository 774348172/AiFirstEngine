//! Bounded pure batch identities; never retains frames, payloads or native leases.
use super::DrawGeometry;
use crate::render_contract::{GeometryHandle, RenderBindingSet};
use std::collections::HashMap;
use std::sync::{Arc, Weak};

#[cfg(test)]
thread_local! {
    pub(super) static IDENTITY_COMPUTATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

type Inputs = (Arc<str>, GeometryHandle, Option<BindingIdentity>, u32, u32, Option<SkinIdentity>);

#[derive(Clone)]
struct SkinIdentity(Weak<crate::character_animation::SkinDeformation3D>);
impl PartialEq for SkinIdentity {
    fn eq(&self,other:&Self)->bool { Weak::ptr_eq(&self.0,&other.0) }
}
impl Eq for SkinIdentity {}
impl std::hash::Hash for SkinIdentity {
    fn hash<H:std::hash::Hasher>(&self,state:&mut H) { std::hash::Hash::hash(&self.0.as_ptr(),state); }
}

// Weak ownership reserves the allocation identity without pinning its payload.
// Arc::make_mut dissociates weak aliases, so mutation still changes this identity.
// Equal detached values may miss this memo but still produce the original digest.
#[derive(Clone)]
struct BindingIdentity(Weak<RenderBindingSet>);
impl PartialEq for BindingIdentity {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for BindingIdentity {}
impl std::hash::Hash for BindingIdentity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.0.as_ptr(), state);
    }
}

#[derive(Default)]
pub(super) struct IdentityMemo {
    identities: HashMap<Inputs, Arc<str>>,
    uncollected_misses: usize,
    #[cfg(test)]
    computations: usize,
}

impl IdentityMemo {
    pub(super) fn prune(&mut self) {
        if self.uncollected_misses >= 128 {
            self.identities.retain(|(_, _, binding, _, _, skin), _| {
                binding.as_ref().is_none_or(|b| b.0.strong_count() != 0)
                    && skin.as_ref().is_none_or(|s| s.0.strong_count()!=0)
            });
            self.uncollected_misses = 0;
        }
    }
    pub(super) fn resolve(&mut self, draw: &DrawGeometry) -> (Arc<str>, Arc<str>) {
        let pipeline = draw.pipeline_key("neutral");
        let key = (
            pipeline.clone(),
            draw.geometry_handle(),
            (!draw.is_opaque_shadow_depth_writer())
                .then(|| BindingIdentity(Arc::downgrade(&draw.material_binding))),
            draw.first_index,
            draw.index_count,
            draw.deformation.as_ref().map(|s|SkinIdentity(Arc::downgrade(s))),
        );
        if let Some(identity) = self.identities.get(&key) {
            return (pipeline, identity.clone());
        }
        self.uncollected_misses = self.uncollected_misses.saturating_add(1);
        #[cfg(test)]
        {
            self.computations += 1;
            IDENTITY_COMPUTATIONS.with(|n| n.set(n.get() + 1));
        }
        let identity: Arc<str> = draw.batch_identity().into();
        // A metadata reuse budget, never an admission limit for valid draws.
        if self.identities.len() < MAX_IDENTITIES {
            self.identities.insert(key, identity.clone());
        }
        (pipeline, identity)
    }

    pub(super) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        for ((pipeline, _, binding, _, _, _), identity) in &self.identities {
            sample.cpu_arc(pipeline, "snapshots", &["snapshot"]);
            sample.cpu_arc(identity, "snapshots", &["snapshot"]);
            if let Some(binding) = binding.as_ref().and_then(|b| b.0.upgrade()) {
                super::memory::binding(sample, &binding, "snapshot");
            }
        }
    }
}

const MAX_IDENTITIES: usize = 65_536;

#[cfg(test)]
thread_local! { static SORT_KEY_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

// OrderedF32 preserves bits but deliberately has no Ord. This private wrapper
// preserves the original comparator's total_cmp, including signed zero.
#[derive(Clone, Copy, PartialEq, Eq)]
struct DepthKey(crate::render_graph::OrderedF32);

impl Ord for DepthKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.to_f32().total_cmp(&other.0.to_f32())
    }
}
impl PartialOrd for DepthKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub(super) fn sort_opaque_segment(draws: &mut [Arc<DrawGeometry>], memo: &mut IdentityMemo) {
    draws.sort_by_cached_key(|draw| {
        #[cfg(test)]
        SORT_KEY_VISITS.with(|n| n.set(n.get() + 1));
        let (pipeline, batch) = memo.resolve(draw);
        let color_material = !draw.is_opaque_shadow_depth_writer();
        (
            draw.blend == crate::render_contract::MaterialBlend::AlphaMask,
            pipeline,
            color_material
                .then(|| draw.material_textures()[0].map(|h| h.raw()))
                .flatten(),
            color_material.then(|| draw.material_binding.binding_id.clone()),
            batch,
            DepthKey(draw.camera_depth),
            draw.instance_id,
        )
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_dynamic_material_bindings_are_not_pinned_by_batch_identity() {
        let frame = super::super::commands::tests::batch_test_frame(
            true,
            crate::render_contract::MaterialBlend::Opaque,
            1,
            1,
        );
        let mut memo = IdentityMemo::default();
        let binding = {
            let mut draw = frame.views[0].draws[0].as_ref().clone();
            draw.material_binding = Arc::new(draw.material_binding.as_ref().clone());
            let weak = Arc::downgrade(&draw.material_binding);
            memo.resolve(&draw);
            weak
        };
        assert!(
            binding.upgrade().is_none(),
            "batch identity pinned a dead dynamic binding"
        );
        for _ in 0..256 {
            memo.prune();
            let mut draw = frame.views[0].draws[0].as_ref().clone();
            draw.material_binding = Arc::new(draw.material_binding.as_ref().clone());
            memo.resolve(&draw);
        }
        memo.prune();
        assert!(memo.identities.len() <= 128);
        let mut draw = frame.views[0].draws[0].as_ref().clone();
        draw.material_binding = Arc::new(draw.material_binding.as_ref().clone());
        let original = memo.resolve(&draw).1;
        Arc::make_mut(&mut draw.material_binding).binding_id =
            "changed-with-only-weak-alias".into();
        let changed = memo.resolve(&draw).1;
        assert_ne!(original, changed);
        assert_eq!(changed.as_ref(), draw.batch_identity());
    }

    #[test]
    fn opaque_sort_matches_reference_without_tree_lookup_per_comparison() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let template = graph
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .find_map(|c| {
                if let crate::render_graph::RenderPassCommand::DrawGeometry { draw } = c {
                    Some(draw.as_ref())
                } else {
                    None
                }
            })
            .unwrap();
        let depths = [-100., -1., -0., 0., 1., 100.];
        let mut draws: Vec<_> = (0..2048)
            .map(|i| {
                let mut draw = template.clone();
                draw.instance_id = ((i * 197) % 2048) as u64;
                draw.camera_depth = depths[i % depths.len()].into();
                Arc::make_mut(&mut draw.material_binding).binding_id =
                    format!("material-{}", i % 17);
                if i % 3 == 0 {
                    draw.cull = crate::render_contract::MaterialCull::Back;
                }
                Arc::new(draw)
            })
            .collect();
        let mut expected = draws.clone();
        let keys: std::collections::BTreeMap<_, _> = draws
            .iter()
            .map(|draw| {
                (
                    draw.instance_id,
                    (
                        draw.pipeline_identity("neutral"),
                        (!draw.is_opaque_shadow_depth_writer())
                            .then(|| {
                                draw.material_binding
                                    .texture("baseColorTexture")
                                    .map(|h| h.raw())
                            })
                            .flatten(),
                        (!draw.is_opaque_shadow_depth_writer())
                            .then(|| draw.material_binding.binding_id.clone()),
                        draw.batch_identity(),
                    ),
                )
            })
            .collect();
        expected.sort_by(|a, b| {
            keys[&a.instance_id]
                .cmp(&keys[&b.instance_id])
                .then_with(|| a.camera_depth.to_f32().total_cmp(&b.camera_depth.to_f32()))
                .then(a.instance_id.cmp(&b.instance_id))
        });
        SORT_KEY_VISITS.with(|n| n.set(0));
        sort_opaque_segment(&mut draws, &mut IdentityMemo::default());
        assert_eq!(
            draws.iter().map(|d| d.instance_id).collect::<Vec<_>>(),
            expected.iter().map(|d| d.instance_id).collect::<Vec<_>>()
        );
        let visits = SORT_KEY_VISITS.with(|n| n.get());
        assert!(
            visits <= draws.len(),
            "sort key visits {visits} exceed {} draws",
            draws.len()
        );
    }

    #[test]
    fn shared_batch_state_does_not_reserialize_per_instance_and_changes_keep_full_identity() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let template = graph
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .find_map(|command| {
                if let crate::render_graph::RenderPassCommand::DrawGeometry { draw } = command {
                    Some(draw.as_ref())
                } else {
                    None
                }
            })
            .unwrap();
        let expected = template.batch_identity();
        let mut memo = IdentityMemo::default();
        let mut previous = None;
        for i in 0..2048 {
            let mut draw = template.clone();
            draw.instance_id = i;
            draw.world_transform[12] = (i as f32 * 0.001).into();
            let (_, identity) = memo.resolve(&draw);
            assert_eq!(identity.as_ref(), expected);
            if let Some(old) = &previous {
                assert!(Arc::ptr_eq(old, &identity));
            }
            previous = Some(identity);
        }
        assert_eq!(
            memo.computations, 1,
            "same batch state serialized and hashed per instance"
        );
        let mut changed = template.clone();
        Arc::make_mut(&mut changed.material_binding).binding_id = "different".into();
        let (_, identity) = memo.resolve(&changed);
        assert_eq!(identity.as_ref(), changed.batch_identity());
        assert_ne!(identity.as_ref(), expected);
        changed = template.clone();
        let binding = Arc::make_mut(&mut changed.geometry_binding);
        let crate::render_contract::BindingResource::Geometry(handle) =
            &mut binding.slots[0].resource
        else {
            panic!()
        };
        let mut raw = handle.raw();
        raw.generation += 1;
        *handle = crate::render_contract::GeometryHandle::try_from(raw).unwrap();
        assert_eq!(memo.resolve(&changed).1.as_ref(), changed.batch_identity());
        changed = template.clone();
        changed.first_index = 3;
        assert_eq!(memo.resolve(&changed).1.as_ref(), changed.batch_identity());
        assert_eq!(memo.computations, 4);
        changed = template.clone();
        changed.index_count = 6;
        assert_eq!(memo.resolve(&changed).1.as_ref(), changed.batch_identity());
        changed = template.clone();
        changed.blend = crate::render_contract::MaterialBlend::AlphaBlend;
        assert_eq!(memo.resolve(&changed).1.as_ref(), changed.batch_identity());
        assert_eq!(memo.computations, 6);
        let mut detached = template.clone();
        detached.material_binding = Arc::new(template.material_binding.as_ref().clone());
        assert_eq!(memo.resolve(&detached).1.as_ref(), expected);
        let old = detached.material_binding.clone();
        Arc::make_mut(&mut detached.material_binding).binding_id = "changed-after-memo".into();
        assert_eq!(old.as_ref(), template.material_binding.as_ref());
        assert_eq!(
            memo.resolve(&detached).1.as_ref(),
            detached.batch_identity()
        );
        assert_ne!(memo.resolve(&detached).1.as_ref(), expected);
    }
}
