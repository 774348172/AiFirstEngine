//! Production-owned immutable command facts. Current selection, attachments and
//! native residency never become retained facts. Graph/RHI carry the same token.
use super::*;
use std::collections::HashMap;
use std::sync::{OnceLock, Weak};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Context {
    shadow: bool,
    domain: RenderDomain,
    depth_target: bool,
    depth: MaterialDepth,
    environment: bool,
    lighting: bool,
    color: bool,
    shadow_mask: u8,
    blend: MaterialBlend,
    cull: MaterialCull,
    mirrored: bool,
    decal_bias: Option<i32>,
    standard_lit: bool,
    opaque_alpha: bool,
    ignore_vertex_color: bool,
}
impl Context {
    fn matches_view(&self, draw: &DrawGeometry) -> bool {
        self.shadow == draw.view.is_shadow_map()
            && self.domain == draw.view.domain
            && self.depth_target == draw.depth_target.is_some()
            && self.depth == draw.depth
            && self.environment == draw.environment.is_some()
            && self.lighting == draw.lighting.is_some()
            && self.color == draw.view.color_pipeline.is_some()
            && self.shadow_mask == draw.shadows.as_ref().map_or(0, |s| s.texture_mask())
    }
    fn of(draw: &DrawGeometry) -> Self {
        Self {
            shadow: draw.view.is_shadow_map(),
            domain: draw.view.domain,
            depth_target: draw.depth_target.is_some(),
            depth: draw.depth,
            environment: draw.environment.is_some(),
            lighting: draw.lighting.is_some(),
            color: draw.view.color_pipeline.is_some(),
            shadow_mask: draw.shadows.as_ref().map_or(0, |s| s.texture_mask()),
            blend: draw.blend,
            cull: draw.cull,
            mirrored: draw.mirrored(),
            decal_bias: draw.decal_bias,
            standard_lit: draw.standard_lit.is_some(),
            opaque_alpha: draw.opaque_alpha,
            ignore_vertex_color: draw.ignore_vertex_color,
        }
    }
}

#[derive(Debug)]
#[doc(hidden)]
pub struct CommandRequirements {
    // Allocation responsibility survives sharing with the native plan. Pipeline
    // identity strings belong to the existing renderer identity memo separately.
    pub(crate) backing_owner: &'static str,
    pub(crate) pipeline: Arc<str>,
    pub(crate) intent_pipeline: Arc<str>,
    pub(crate) textures: [Option<TextureHandle>; 5],
}
#[derive(Debug)]
struct FormatRequirements {
    format: String,
    facts: Arc<CommandRequirements>,
}

// Source identity is independent of the frame's lease and instance table. Each
// Weak reserves an exact immutable allocation, preventing pointer recycling,
// without retaining packet resources or obsolete native leases.
type SourceKey = ([usize; 5], u32, u32);
#[derive(Debug)]
struct Source {
    geometry: Weak<GeometryBuffers>,
    geometry_binding: Weak<RenderBindingSet>,
    material_binding: Weak<RenderBindingSet>,
    geometry_source: Weak<Provenance>,
    material_source: Weak<Provenance>,
    key: SourceKey,
}
impl Source {
    fn key(draw: &DrawGeometry) -> SourceKey {
        (
            [
                Arc::as_ptr(&draw.geometry) as usize,
                Arc::as_ptr(&draw.geometry_binding) as usize,
                Arc::as_ptr(&draw.material_binding) as usize,
                Arc::as_ptr(&draw.geometry_source) as usize,
                Arc::as_ptr(&draw.material_source) as usize,
            ],
            draw.first_index,
            draw.index_count,
        )
    }
    fn new(draw: &DrawGeometry) -> Self {
        Self {
            geometry: Arc::downgrade(&draw.geometry),
            geometry_binding: Arc::downgrade(&draw.geometry_binding),
            material_binding: Arc::downgrade(&draw.material_binding),
            geometry_source: Arc::downgrade(&draw.geometry_source),
            material_source: Arc::downgrade(&draw.material_source),
            key: Self::key(draw),
        }
    }
    fn live(&self) -> bool {
        self.geometry.strong_count() != 0
            && self.geometry_binding.strong_count() != 0
            && self.material_binding.strong_count() != 0
            && self.geometry_source.strong_count() != 0
            && self.material_source.strong_count() != 0
    }
}
#[derive(Debug)]
pub(crate) struct RetainedDrawCommand {
    source: Source,
    context: Context,
    // Two common formats are bounded; additional formats use strict fallback.
    formats: [OnceLock<FormatRequirements>; 2],
}
impl RetainedDrawCommand {
    pub(super) fn attach_to(
        self: &Arc<Self>,
        draw: &mut DrawGeometry,
        static_proven: bool,
    ) -> bool {
        if !self.matches(draw) {
            return false;
        }
        draw.command = Some(self.clone());
        if static_proven {
            draw.static_instance = Some(Arc::downgrade(&draw.instance));
        }
        true
    }
    fn matches(&self, draw: &DrawGeometry) -> bool {
        // Attachment already checked the complete source/context. The existing
        // static witness identifies the same immutable instance, including its
        // resources and winding. Public COW access clears both metadata fields;
        // internal instance replacement dissociates the Weak. View-local fields
        // stay mutable and must still be compared on every consumption.
        if draw.static_instance_proven()
            && draw
                .command
                .as_ref()
                .is_some_and(|command| std::ptr::eq(command.as_ref(), self))
        {
            return self.context.matches_view(draw);
        }
        #[cfg(test)]
        SOURCE_CONTEXT_CHECKS.with(|n| n.set(n.get() + 1));
        self.source.key == Source::key(draw) && self.context == Context::of(draw)
    }
    pub(crate) fn requirements(
        &self,
        draw: &DrawGeometry,
        format: &str,
    ) -> Option<Arc<CommandRequirements>> {
        if !self.matches(draw) {
            return None;
        }
        for entry in &self.formats {
            let fact = entry.get_or_init(|| {
                #[cfg(test)]
                INTERPRETATIONS.with(|n| n.set(n.get() + 1));
                let pipeline = draw.pipeline_key_uncached(format);
                FormatRequirements {
                    format: format.into(),
                    facts: Arc::new(CommandRequirements {
                        backing_owner: "snapshots",
                        intent_pipeline: if format == "Rgba8Unorm" {
                            pipeline.clone()
                        } else {
                            draw.pipeline_key_uncached("Rgba8Unorm")
                        },
                        pipeline,
                        textures: draw.material_textures(),
                    }),
                }
            });
            if fact.format == format {
                return Some(fact.facts.clone());
            }
        }
        None
    }
    pub(super) fn observe_memory(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
        role: &str,
    ) {
        for fact in self.formats.iter().filter_map(OnceLock::get) {
            sample.cpu_string(&fact.format, "snapshots", &[role]);
            sample.cpu_arc(&fact.facts, fact.facts.backing_owner, &[role]);
            sample.cpu_arc(&fact.facts.pipeline, "renderer", &[role]);
            sample.cpu_arc(&fact.facts.intent_pipeline, "renderer", &[role]);
        }
    }
}

#[derive(Debug)]
pub(super) struct CommandCache {
    entries: HashMap<(SourceKey, Context), Arc<RetainedDrawCommand>>,
    limit: usize,
    next_prune: usize,
}
impl Default for CommandCache {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            limit: 65_536,
            next_prune: 128,
        }
    }
}
impl CommandCache {
    pub(super) fn prune(&mut self) {
        if self.entries.len() >= self.next_prune.min(self.limit) {
            self.entries.retain(|_, command| command.source.live());
            // Collect dead dynamic sources before warmup ends. A stable live
            // workset is scanned again only after another interval of growth.
            self.next_prune = self.entries.len().saturating_add(128);
        }
    }
    /// Warm immutable cache access. No mutation, eviction, residency or uploaded
    /// success is inferred here; the complete resource/context key still matches.
    pub(super) fn attach_existing(&self, draw: &mut DrawGeometry, static_proven: bool) -> bool {
        #[cfg(test)]
        COMMAND_LOOKUPS.with(|n| n.set(n.get() + 1));
        let key = (Source::key(draw), Context::of(draw));
        let Some(command) = self.entries.get(&key) else {
            return false;
        };
        draw.command = Some(command.clone());
        if static_proven {
            draw.static_instance = Some(Arc::downgrade(&draw.instance));
        }
        true
    }
    pub(super) fn attach(&mut self, draw: &mut DrawGeometry, static_proven: bool) {
        let key = (Source::key(draw), Context::of(draw));
        if let Some(command) = self.entries.get(&key) {
            draw.command = Some(command.clone());
        } else {
            if self.entries.len() < self.limit {
                #[cfg(test)]
                COMMANDS_BUILT.with(|n| n.set(n.get() + 1));
                let command = Arc::new(RetainedDrawCommand {
                    source: Source::new(draw),
                    context: key.1,
                    formats: std::array::from_fn(|_| OnceLock::new()),
                });
                self.entries.insert(key, command.clone());
                draw.command = Some(command);
            } else {
                draw.command = None;
            }
        }
        if static_proven {
            draw.static_instance = Some(Arc::downgrade(&draw.instance));
        }
    }
    pub(super) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        for command in self.entries.values() {
            sample.cpu_arc(command, "snapshots", &["snapshot"]);
            command.observe_memory(sample, "snapshot");
        }
        sample.partial(
            "cpu.owner.snapshots",
            "retainedCommandHashBucketsAndWeakControlBackingNotInstrumented",
        );
    }
}

#[cfg(test)]
thread_local! {
    pub(super) static COMMANDS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static COMMAND_LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static INTERPRETATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_CONTEXT_CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attached_static_command_consumes_instance_proof_but_checks_current_view_context() {
        let frame = commands::tests::batch_test_frame(true, MaterialBlend::Opaque, 4, 1);
        let mut draw = frame.views[0].draws[0].as_ref().clone();
        let mut cache = CommandCache::default();
        cache.attach(&mut draw, true);
        let command = draw.command.as_ref().unwrap().clone();
        let facts = draw.retained_requirements("Rgba8Unorm").unwrap();
        SOURCE_CONTEXT_CHECKS.with(|n| n.set(0));
        for _ in 0..256 {
            assert!(Arc::ptr_eq(
                &draw.retained_requirements("Rgba8Unorm").unwrap(),
                &facts
            ));
        }
        assert_eq!(
            SOURCE_CONTEXT_CHECKS.with(std::cell::Cell::get),
            0,
            "attached immutable instance reinterpreted source and winding"
        );
        for change in 0..10 {
            let mut changed = draw.clone();
            match change {
                0 => Arc::make_mut(&mut changed.view).view_matrix[12] = 0.25f32.into(),
                1 => Arc::make_mut(&mut changed.view).domain = RenderDomain::World2d,
                2 => changed.depth_target = None,
                3 => changed.depth = MaterialDepth::TestOnly,
                4 => Arc::make_mut(&mut changed.view).target_kind = TargetKind::ShadowMap,
                5 => Arc::make_mut(&mut changed.view).color_pipeline = Some(Default::default()),
                6 => changed.lighting = None,
                7 => Arc::make_mut(&mut changed.instance).world_transform[0] = (-1f32).into(),
                8 => Arc::make_mut(&mut changed.instance).index_count = 0,
                _ => changed.base_color[0] = 0.25f32.into(),
            }
            let expected = command.source.key == Source::key(&changed)
                && command.context == Context::of(&changed);
            assert_eq!(
                command.requirements(&changed, "Rgba8Unorm").is_some(),
                expected,
                "change {change} transferred stale command context"
            );
            assert_eq!(
                changed.pipeline_key("Rgba8Unorm"),
                changed.pipeline_key_uncached("Rgba8Unorm")
            );
        }
        let lease = Arc::downgrade(&draw.resource_lease);
        drop(draw);
        drop(frame);
        assert!(lease.upgrade().is_none(), "command pinned an old lease");
    }

    #[test]
    fn dead_dynamic_command_metadata_is_reclaimed_before_capacity() {
        let frame = commands::tests::batch_test_frame(true, MaterialBlend::Opaque, 1, 1);
        let mut cache = CommandCache::default();
        for _ in 0..256 {
            cache.prune();
            let mut draw = frame.views[0].draws[0].as_ref().clone();
            draw.geometry_binding = Arc::new(draw.geometry_binding.as_ref().clone());
            cache.attach(&mut draw, false);
        }
        cache.prune();
        assert!(
            cache.entries.len() <= 128,
            "dead dynamic source metadata accumulated: {}",
            cache.entries.len()
        );
    }

    #[test]
    fn retained_resource_commands_survive_dynamic_instances_without_sharing_validation() {
        for count in [4, 32, 256] {
            let frame = commands::tests::batch_test_frame(true, MaterialBlend::Opaque, 4, 1);
            let mut draw = frame.views[0].draws[0].as_ref().clone();
            let mut cache = CommandCache::default();
            cache.attach(&mut draw, true);
            let original = draw.command.as_ref().unwrap().clone();
            let facts = original.requirements(&draw, "Rgba8Unorm").unwrap();
            COMMANDS_BUILT.with(|n| n.set(0));
            INTERPRETATIONS.with(|n| n.set(0));
            for index in 0..count {
                let mut moving = draw.clone();
                moving.world_transform[12] = (index as f32 * 0.125).into();
                moving.base_color[0] = ((index % 8) as f32 / 8.0).into();
                cache.attach(&mut moving, false);
                assert!(Arc::ptr_eq(moving.command.as_ref().unwrap(), &original));
                assert!(Arc::ptr_eq(
                    &moving.retained_requirements("Rgba8Unorm").unwrap(),
                    &facts
                ));
                assert_eq!(
                    moving.pipeline_key("Rgba8Unorm"),
                    moving.pipeline_key_uncached("Rgba8Unorm")
                );
            }
            assert_eq!(
                COMMANDS_BUILT.with(std::cell::Cell::get),
                0,
                "scale {count}"
            );
            assert_eq!(
                INTERPRETATIONS.with(std::cell::Cell::get),
                0,
                "scale {count}"
            );
            // A shared pure command must not transfer a different instance's
            // successful static validation, including direct internal COW access.
            let mut invalid = draw.clone();
            Arc::make_mut(&mut invalid.instance).world_transform[0] = f32::NAN.into();
            cache.attach(&mut invalid, false);
            assert!(Arc::ptr_eq(invalid.command.as_ref().unwrap(), &original));
            assert!(proof::draw(&Arc::new(invalid), &mut DrawViewValidation::default()).is_err());
        }
    }

    #[test]
    fn retained_commands_reuse_exact_source_but_do_not_pin_resources_or_reject_extra_formats() {
        let frame = commands::tests::batch_test_frame(true, MaterialBlend::Opaque, 4, 1);
        let mut draw = frame.views[0].draws[0].as_ref().clone();
        let mut cache = CommandCache::default();
        let wire = serde_json::to_value(&draw).unwrap();
        cache.attach(&mut draw, false);
        let original = draw.command.as_ref().unwrap().clone();
        let weak = Arc::downgrade(&draw.instance);
        let lease = Arc::downgrade(&draw.resource_lease);
        assert_eq!(wire, serde_json::to_value(&draw).unwrap());
        for format in ["Rgba8Unorm", "Rgba16Float", "Bgra8UnormSrgb"] {
            assert_eq!(
                draw.pipeline_key(format),
                draw.pipeline_key_uncached(format)
            );
        }
        let first = original.requirements(&draw, "Rgba8Unorm").unwrap();
        assert!(Arc::ptr_eq(
            &first,
            &original.requirements(&draw, "Rgba8Unorm").unwrap()
        ));
        assert!(original.requirements(&draw, "Bgra8UnormSrgb").is_none());
        for change in 0..16 {
            let mut changed = draw.clone();
            match change {
                0 => changed.base_color[0] = 0.3f32.into(),
                1 => Arc::make_mut(&mut changed.view).domain = RenderDomain::World2d,
                2 => changed.depth_target = None,
                3 => changed.depth = MaterialDepth::TestOnly,
                4 => Arc::make_mut(&mut changed.view).target_kind = TargetKind::ShadowMap,
                5 => Arc::make_mut(&mut changed.view).color_pipeline = Some(Default::default()),
                6 => changed.lighting = None,
                7 => Arc::make_mut(&mut changed.material_binding)
                    .binding_id
                    .push_str("-reimport"),
                8 => changed.world_transform[0] = (-1f32).into(),
                9 => changed.instance = Arc::new(changed.instance.as_ref().clone()),
                10 => changed.cull = MaterialCull::Front,
                11 => changed.blend = MaterialBlend::AlphaMask,
                12 => changed.decal_bias = Some(-1),
                13 => changed.standard_lit = None,
                14 => changed.opaque_alpha = !draw.opaque_alpha,
                _ => changed.ignore_vertex_color = !draw.ignore_vertex_color,
            }
            assert_eq!(
                original.requirements(&changed, "Rgba8Unorm").is_some(),
                matches!(change, 0 | 9),
                "change {change}"
            );
            assert_eq!(
                changed.pipeline_key("Rgba8Unorm"),
                changed.pipeline_key_uncached("Rgba8Unorm")
            );
        }
        let mut camera = draw.clone();
        Arc::make_mut(&mut camera.view).view_matrix[12] = 0.25f32.into();
        cache.attach(&mut camera, false);
        assert!(Arc::ptr_eq(camera.command.as_ref().unwrap(), &original));
        assert!(Arc::ptr_eq(
            &first,
            &camera.retained_requirements("Rgba8Unorm").unwrap()
        ));
        let mut full = CommandCache {
            limit: 0,
            ..Default::default()
        };
        full.attach(&mut camera, false);
        assert!(camera.command.is_none());
        assert_eq!(
            camera.pipeline_key("Rgba8Unorm"),
            camera.pipeline_key_uncached("Rgba8Unorm")
        );
        drop(camera);
        drop(draw);
        drop(frame);
        assert!(
            weak.upgrade().is_none(),
            "command pinned immutable instance"
        );
        assert!(lease.upgrade().is_none(), "command pinned resource lease");
    }
}

impl CommandRequirements {
    /// Pure native interpretation facts, never a resource or validation proof.
    #[doc(hidden)]
    pub fn from_native_facts(
        backing_owner: &'static str,
        pipeline: Arc<str>,
        intent_pipeline: Arc<str>,
        textures: [Option<TextureHandle>; 5],
    ) -> Self {
        Self {
            backing_owner,
            pipeline,
            intent_pipeline,
            textures,
        }
    }
    #[doc(hidden)]
    pub fn pipeline(&self) -> &Arc<str> {
        &self.pipeline
    }
    #[doc(hidden)]
    pub fn intent_pipeline(&self) -> &Arc<str> {
        &self.intent_pipeline
    }
    #[doc(hidden)]
    pub fn textures(&self) -> &[Option<TextureHandle>; 5] {
        &self.textures
    }
    #[doc(hidden)]
    pub fn backing_owner(&self) -> &'static str {
        self.backing_owner
    }
}
