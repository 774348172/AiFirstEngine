//! Bounded full execution states per thread; no asset/pipeline registry or digest-only match.
use crate::render_contract::{
    GeometryIndexFormat, MaterialBlend, MaterialCull, MaterialDepth, RenderDomain, VertexAttribute,
};
use std::borrow::Cow;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct PipelineInputs<'a> {
    pub shadow: bool,
    pub shadow_mask: u8,
    pub scene_lights: bool,
    pub color_pipeline: bool,
    pub lit_mask: Option<u8>,
    pub stride: u32,
    pub attributes: Cow<'a, [VertexAttribute]>,
    pub index_format: GeometryIndexFormat,
    pub color_format: Cow<'a, str>,
    pub depth_target: bool,
    pub blend: MaterialBlend,
    pub cull: MaterialCull,
    pub mirrored: bool,
    pub depth: MaterialDepth,
    pub decal_bias: Option<i32>,
    pub domain: RenderDomain,
    pub textured: bool,
    pub flat: bool,
    pub opaque: bool,
}

impl PipelineInputs<'_> {
    fn into_owned(self) -> PipelineInputs<'static> {
        PipelineInputs {
            shadow: self.shadow,
            shadow_mask: self.shadow_mask,
            scene_lights: self.scene_lights,
            color_pipeline: self.color_pipeline,
            lit_mask: self.lit_mask,
            stride: self.stride,
            attributes: Cow::Owned(self.attributes.into_owned()),
            index_format: self.index_format,
            color_format: Cow::Owned(self.color_format.into_owned()),
            depth_target: self.depth_target,
            blend: self.blend,
            cull: self.cull,
            mirrored: self.mirrored,
            depth: self.depth,
            decal_bias: self.decal_bias,
            domain: self.domain,
            textured: self.textured,
            flat: self.flat,
            opaque: self.opaque,
        }
    }
}

#[derive(Default)]
struct IdentityMemo {
    entries: Vec<(PipelineInputs<'static>, Arc<str>)>,
    // Sorted fingerprints only locate candidates. Full inputs still prove a hit.
    index: Vec<(u64, usize)>,
    next: usize,
    last: Option<usize>,
    #[cfg(test)]
    comparisons: usize,
}

const MAX_IDENTITIES: usize = 64;

impl IdentityMemo {
    fn resolve(
        &mut self,
        inputs: PipelineInputs<'_>,
        compute: impl FnOnce() -> String,
    ) -> Arc<str> {
        if let Some(slot) = self.last {
            #[cfg(test)]
            {
                self.comparisons += 1;
            }
            let (old, digest) = &self.entries[slot];
            if old == &inputs {
                return digest.clone();
            }
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        inputs.hash(&mut hash);
        self.resolve_hashed(inputs, hash.finish(), compute)
    }

    fn resolve_hashed(
        &mut self,
        inputs: PipelineInputs<'_>,
        fingerprint: u64,
        compute: impl FnOnce() -> String,
    ) -> Arc<str> {
        let start = self.index.partition_point(|(old, _)| *old < fingerprint);
        for &(old, slot) in &self.index[start..] {
            if old != fingerprint {
                break;
            }
            #[cfg(test)]
            {
                self.comparisons += 1;
            }
            let (key, digest) = &self.entries[slot];
            if key == &inputs {
                self.last = Some(slot);
                return digest.clone();
            }
        }
        let digest: Arc<str> = compute().into();
        let entry = (inputs.into_owned(), digest.clone());
        let slot = if self.entries.len() == MAX_IDENTITIES {
            let slot = self.next;
            self.index.retain(|(_, old)| *old != slot);
            self.entries[slot] = entry;
            self.next = (slot + 1) % MAX_IDENTITIES;
            slot
        } else {
            let slot = self.entries.len();
            self.entries.push(entry);
            slot
        };
        self.index.push((fingerprint, slot));
        self.index.sort_unstable();
        self.last = Some(slot);
        digest
    }
}

thread_local! {
    static IDENTITY: RefCell<IdentityMemo> = RefCell::new(IdentityMemo::default());
}

pub(super) fn identity(inputs: PipelineInputs<'_>, compute: impl FnOnce() -> String) -> Arc<str> {
    IDENTITY.with(|memo| memo.borrow_mut().resolve(inputs, compute))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_collisions_preserve_full_keys_and_fifo_slot_replacement() {
        let mut memo = IdentityMemo::default();
        let state = |index| {
            let mut input = inputs();
            input.color_format = Cow::Owned(format!("format-{index}"));
            input
        };
        for index in 0..=MAX_IDENTITIES {
            assert_eq!(
                memo.resolve_hashed(state(index), 7, || format!("digest-{index}"))
                    .as_ref(),
                format!("digest-{index}")
            );
            assert!(memo.entries.len() <= MAX_IDENTITIES);
            assert_eq!(memo.index.len(), memo.entries.len());
        }
        assert_eq!(
            memo.resolve_hashed(state(1), 7, || panic!("live collision key lost"))
                .as_ref(),
            "digest-1"
        );
        assert_eq!(
            memo.resolve_hashed(state(0), 7, || "rebuilt-zero".into())
                .as_ref(),
            "rebuilt-zero"
        );
        assert_eq!(
            memo.resolve_hashed(state(MAX_IDENTITIES), 7, || panic!(
                "newest collision key lost"
            ))
            .as_ref(),
            format!("digest-{MAX_IDENTITIES}")
        );
        let mut slots = memo.index.iter().map(|(_, slot)| *slot).collect::<Vec<_>>();
        slots.sort_unstable();
        assert_eq!(slots, (0..MAX_IDENTITIES).collect::<Vec<_>>());
    }

    #[test]
    fn resident_pipeline_states_do_not_scan_the_whole_fifo_on_each_lookup() {
        let mut memo = IdentityMemo::default();
        let state = |index| {
            let mut input = inputs();
            input.color_format = Cow::Owned(format!("format-{index}"));
            input
        };
        for index in 0..MAX_IDENTITIES {
            memo.resolve(state(index), || format!("digest-{index}"));
        }
        memo.comparisons = 0;
        for index in 0..2048 {
            let index = index % MAX_IDENTITIES;
            assert_eq!(
                memo.resolve(state(index), || panic!("resident state missed"))
                    .as_ref(),
                format!("digest-{index}")
            );
        }
        assert!(
            memo.comparisons <= 4096,
            "resident pipeline lookup performed {} full comparisons for 2048 hits",
            memo.comparisons
        );
    }

    #[test]
    fn opaque_depth_pipeline_identity_ignores_only_unconsumed_material_inputs() {
        use crate::render_contract::*;
        use crate::render_graph::RenderPassCommand;
        use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let mut draw = graph
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .find_map(|c| {
                if let RenderPassCommand::DrawGeometry { draw } = c {
                    Some(draw.as_ref().clone())
                } else {
                    None
                }
            })
            .unwrap();
        let view = Arc::make_mut(&mut draw.view);
        view.view_kind = ViewKind::Shadow;
        view.target_kind = TargetKind::ShadowMap;
        view.depth_policy = DepthPolicy::TestAndWrite;
        view.clear_color = None;
        view.depth_clear = Some(1f32.into());
        draw.target = "shadow-atlas".into();
        draw.depth_target = Some(draw.target.clone());
        draw.depth = MaterialDepth::TestAndWrite;
        draw.standard_lit = Some(StandardLitParameters {
            surface: [0f32, 0.5, 1., 1.].map(Into::into),
            emissive: [0f32; 4].map(Into::into),
        });
        draw.geometry = Arc::new(
            serde_json::from_value(serde_json::json!({
                "vertexBytes":vec![0u8;96],"indexBytes":[0,0,1,0,2,0],
                "vertexStride":32,"vertexCount":3,"indexCount":3,"indexFormat":"uint16",
                "attributes":[{"semantic":"position","format":"float32x3","offset":0},
                    {"semantic":"normal","format":"float32x3","offset":12},
                    {"semantic":"texCoord0","format":"float32x2","offset":24}]
            }))
            .unwrap(),
        );
        let opaque = draw.pipeline_key("rgba8");
        let mask_without_texture = {
            let mut masked = draw.clone();
            masked.blend = MaterialBlend::AlphaMask;
            masked.alpha_cutoff = Some(0.5f32.into());
            masked.pipeline_key("rgba8")
        };
        for mask in 0..32 {
            let mut changed = draw.clone();
            let binding = Arc::make_mut(&mut changed.material_binding);
            for (index, name) in STANDARD_LIT_TEXTURE_SLOTS.into_iter().enumerate() {
                if mask & (1 << index) != 0 {
                    binding.slots.push(NamedRenderBinding::new(
                        name,
                        RenderResourceHandle {
                            kind: RenderResourceKind::Texture,
                            index: index as u64 + 10,
                            generation: 1,
                        },
                    ));
                }
            }
            assert_eq!(changed.texture_mask(), mask);
            changed.validate().unwrap();
            assert_eq!(
                changed.pipeline_key("rgba8"),
                opaque,
                "position-only opaque depth split on unused texture layout {mask}"
            );
            changed.blend = MaterialBlend::AlphaMask;
            changed.alpha_cutoff = Some(0.5f32.into());
            if mask != 0 {
                assert_ne!(changed.pipeline_key("rgba8"), mask_without_texture);
            }
        }
        let mut changed = draw.clone();
        changed.standard_lit = None;
        changed.ignore_vertex_color = true;
        changed.opaque_alpha = true;
        assert_eq!(
            changed.pipeline_key("rgba8"),
            opaque,
            "color-only flags split opaque depth"
        );
        changed = draw.clone();
        changed.cull = MaterialCull::Back;
        assert_ne!(changed.pipeline_key("rgba8"), opaque);
        let before_mirror = changed.pipeline_key("rgba8");
        changed.world_transform[0] = (-1f32).into();
        assert_ne!(changed.pipeline_key("rgba8"), before_mirror);
    }

    #[test]
    fn alternating_live_pipeline_states_do_not_rehash_shader_sources() {
        let mut memo = IdentityMemo::default();
        let mut computations = 0;
        let mut identities = [None, None];
        let started = crate::platform_time::Instant::now();
        for i in 0..2048 {
            let state = i % 2;
            let mut input = inputs();
            input.blend = if state == 0 {
                MaterialBlend::Opaque
            } else {
                MaterialBlend::AlphaMask
            };
            let digest = memo.resolve(input, || {
                computations += 1;
                crate::canonical_digest::sha256_prefixed(
                    format!(
                        "{state}|{}|{}",
                        super::super::UNLIT_SHADER_SOURCE,
                        super::super::UNLIT_GEOMETRY_SHADER_SOURCE
                    )
                    .as_bytes(),
                )
            });
            if let Some(previous) = &identities[state] {
                assert_eq!(&digest, previous);
            } else {
                identities[state] = Some(digest);
            }
        }
        eprintln!(
            "alternating pipeline computations={computations}, elapsedNs={}",
            started.elapsed().as_nanos()
        );
        assert_eq!(
            computations, 2,
            "alternating valid states rehash shader sources per draw"
        );
    }

    #[test]
    fn repeated_pipeline_identity_shares_the_existing_digest_allocation() {
        let mut memo = IdentityMemo::default();
        let first = memo.resolve(inputs(), || "same-typed-pipeline-digest".into());
        let second = memo.resolve(inputs(), || panic!("unchanged typed inputs"));
        assert_eq!(
            first.as_ptr(),
            second.as_ptr(),
            "each immutable draw copied the same cached pipeline string"
        );
    }
    fn inputs() -> PipelineInputs<'static> {
        PipelineInputs {
            shadow: false,
            shadow_mask: 0,
            scene_lights: false,
            color_pipeline: false,
            lit_mask: None,
            stride: 12,
            attributes: Cow::Borrowed(&[]),
            index_format: GeometryIndexFormat::Uint16,
            color_format: Cow::Borrowed("rgba8"),
            depth_target: true,
            blend: MaterialBlend::Opaque,
            cull: MaterialCull::None,
            mirrored: false,
            depth: MaterialDepth::TestAndWrite,
            decal_bias: None,
            domain: RenderDomain::World3d,
            textured: false,
            flat: false,
            opaque: true,
        }
    }

    #[test]
    fn repeated_pipeline_inputs_compute_once_and_replacement_is_bounded() {
        let mut memo = IdentityMemo::default();
        let mut computations = 0;
        for _ in 0..10000 {
            assert_eq!(
                memo.resolve(inputs(), || {
                    computations += 1;
                    "original-digest".into()
                })
                .as_ref(),
                "original-digest"
            );
        }
        assert_eq!(
            computations, 1,
            "each draw recomputed the same executable digest"
        );
        let mut changed = inputs();
        changed.flat = true;
        assert_eq!(memo.resolve(changed, || "flat".into()).as_ref(), "flat");
        assert_eq!(
            memo.resolve(inputs(), || panic!("live original state was displaced"))
                .as_ref(),
            "original-digest"
        );
        assert_eq!(memo.entries.len(), 2);
    }

    #[test]
    fn identity_memo_evicts_oldest_at_the_bound_without_aliasing_inputs() {
        let mut memo = IdentityMemo::default();
        for i in 0..=MAX_IDENTITIES {
            let mut input = inputs();
            input.stride += i as u32 * 4;
            assert_eq!(
                memo.resolve(input, || format!("state-{i}")).as_ref(),
                format!("state-{i}")
            );
            assert!(memo.entries.len() <= MAX_IDENTITIES);
        }
        assert_eq!(memo.entries.len(), MAX_IDENTITIES);
        assert!(memo
            .entries
            .iter()
            .all(|(key, _)| key.stride != inputs().stride));
        assert_eq!(
            memo.resolve(inputs(), || "rebuilt-original".into())
                .as_ref(),
            "rebuilt-original"
        );
        let mut latest = inputs();
        latest.stride += MAX_IDENTITIES as u32 * 4;
        assert_eq!(
            memo.resolve(latest, || panic!("live newest state was evicted"))
                .as_ref(),
            format!("state-{MAX_IDENTITIES}")
        );
        assert_eq!(memo.entries.len(), MAX_IDENTITIES);
    }

    #[test]
    fn every_execution_input_invalidates_without_using_a_hash_as_equality() {
        let mut variants = vec![inputs(); 14];
        variants[0].stride = 16;
        variants[1].attributes = Cow::Owned(vec![VertexAttribute {
            semantic: crate::render_contract::VertexSemantic::Position,
            format: crate::render_contract::VertexFormat::Float32x3,
            offset: 0,
        }]);
        variants[2].index_format = GeometryIndexFormat::Uint32;
        variants[3].color_format = Cow::Borrowed("bgra8");
        variants[4].depth_target = false;
        variants[5].blend = MaterialBlend::AlphaBlend;
        variants[6].cull = MaterialCull::Back;
        variants[7].mirrored = true;
        variants[8].depth = MaterialDepth::TestOnly;
        variants[9].domain = RenderDomain::World2d;
        variants[10].textured = true;
        variants[11].flat = true;
        variants[12].opaque = false;
        variants[13].color_pipeline = true;
        let mut memo = IdentityMemo::default();
        for variant in variants {
            memo.resolve(inputs(), || "base".into());
            let mut computed = false;
            assert_eq!(
                memo.resolve(variant.clone(), || {
                    computed = true;
                    "changed".into()
                })
                .as_ref(),
                "changed"
            );
            assert!(computed);
            assert_eq!(
                memo.resolve(variant, || panic!("identical input recomputed"))
                    .as_ref(),
                "changed"
            );
        }
    }
}
