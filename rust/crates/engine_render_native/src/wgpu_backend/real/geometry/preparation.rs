//! CPU dependency interpretation owned by GeometryGpuState. Never a GPU admission proof.
use super::*;

pub(super) use crate::render_geometry::CommandRequirements as Requirements;

// Dependency interpretation for one validated plan scan. Borrowed CPU facts
// only: every native allocation, admission and retirement stays in its owner.
#[derive(Default)]
pub(super) struct Bindings<'a> {
    pub(super) lights: BTreeMap<&'a str, &'a crate::render_geometry::ClusteredLights>,
    pub(super) environments: BTreeMap<&'a str, &'a crate::environment_lighting::EnvironmentMap>,
    pub(super) shadows: BTreeMap<u64, &'a crate::render_geometry::ShadowMaps>,
    pub(super) materials: BTreeSet<lit::BorrowedTextureKey<'a>>,
    materials_seen: std::collections::HashSet<(usize, usize)>,
    last_light: Option<&'a crate::render_geometry::ClusteredLights>,
    last_environment: Option<&'a crate::environment_lighting::EnvironmentMap>,
    last_shadow: Option<(u64, &'a crate::render_geometry::ShadowMaps)>,
    last_material: Option<(usize, usize)>,
    last_pipeline: Option<Arc<str>>,
}
#[cfg(test)]
thread_local! {static DEPENDENCY_QUERIES: std::cell::Cell<[usize;5]> = const {std::cell::Cell::new([0;5])};}
#[cfg(test)]
fn dependency_query(i: usize) {
    DEPENDENCY_QUERIES.with(|v| {
        let mut n = v.get();
        n[i] += 1;
        v.set(n);
    });
}
impl<'a> Bindings<'a> {
    // Only this call's immutable scan mutates used. The strong CPU string
    // prevents allocation recycling; no resource residency is certified here.
    pub(super) fn include_pipeline(
        &mut self,
        facts: &Requirements,
        used: &mut BTreeSet<Arc<str>>,
    ) -> bool {
        if self
            .last_pipeline
            .as_ref()
            .is_some_and(|last| Arc::ptr_eq(last, facts.pipeline()))
        {
            return false;
        }
        self.last_pipeline = Some(facts.pipeline().clone());
        #[cfg(test)]
        dependency_query(4);
        used.insert(facts.pipeline().clone())
    }
    pub(super) fn include(&mut self, draw: &'a DrawGeometry, facts: &Requirements) {
        if let Some(light) = &draw.lighting {
            if !self
                .last_light
                .is_some_and(|last| std::ptr::eq(last, light.as_ref()))
            {
                #[cfg(test)]
                dependency_query(0);
                self.lights.entry(light.identity()).or_insert(light);
                self.last_light = Some(light);
            }
        }
        if let Some(environment) = &draw.environment {
            if !self
                .last_environment
                .is_some_and(|last| std::ptr::eq(last, environment.map.as_ref()))
            {
                #[cfg(test)]
                dependency_query(1);
                self.environments
                    .entry(environment.map.identity())
                    .or_insert(&environment.map);
                self.last_environment = Some(&environment.map);
            }
        }
        if let Some(shadows) = &draw.shadows {
            if !self.last_shadow.is_some_and(|(id, last)| {
                id == draw.view.view_id && std::ptr::eq(last, shadows.as_ref())
            }) {
                #[cfg(test)]
                dependency_query(2);
                self.shadows.insert(draw.view.view_id, shadows);
                self.last_shadow = Some((draw.view.view_id, shadows));
            }
        }
        if draw.standard_lit.is_some() && !draw.is_opaque_shadow_depth_writer() {
            let key = (
                Arc::as_ptr(&draw.material_binding) as usize,
                draw.environment
                    .as_ref()
                    .map_or(0, |e| Arc::as_ptr(e) as usize),
            );
            if self.last_material != Some(key) {
                self.last_material = Some(key);
                #[cfg(test)]
                dependency_query(3);
                if self.materials_seen.insert(key) {
                    self.materials.insert((
                        facts.textures().map(|texture| texture.map(|h| h.raw())),
                        draw.environment.as_ref().map(|e| e.map.identity()),
                    ));
                }
            }
        }
    }
}

// Instance identity covers immutable geometry, material and transform facts.
// Every pipeline input outside that allocation is listed here. Camera matrices,
// selection and native object identity deliberately are not preparation facts.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Context {
    format: wgpu::TextureFormat,
    shadow: bool,
    domain: RenderDomain,
    depth_target: bool,
    depth: MaterialDepth,
    environment: bool,
    lighting: bool,
    color: bool,
    shadow_mask: u8,
}
impl Context {
    fn of(draw: &DrawGeometry, format: wgpu::TextureFormat) -> Self {
        Self {
            format,
            shadow: draw.view.is_shadow_map(),
            domain: draw.view.domain,
            depth_target: draw.depth_target.is_some(),
            depth: draw.depth,
            environment: draw.environment.is_some(),
            lighting: draw.lighting.is_some(),
            color: draw.view.color_pipeline.is_some(),
            shadow_mask: draw.shadows.as_ref().map_or(0, |v| v.texture_mask()),
        }
    }
}
struct Entry {
    instance: std::sync::Weak<crate::render_geometry::DrawInstance>,
    requirements: Arc<Requirements>,
}
pub(super) struct Preparation {
    entries: std::cell::RefCell<std::collections::HashMap<(usize, Context), Entry>>,
    limit: usize,
}
impl Default for Preparation {
    fn default() -> Self {
        Self {
            entries: Default::default(),
            limit: 65_536,
        }
    }
}

impl Preparation {
    pub(super) fn resolve(
        &self,
        draw: &DrawGeometry,
        format: wgpu::TextureFormat,
    ) -> Arc<Requirements> {
        let retained_format = match format {
            wgpu::TextureFormat::Rgba8Unorm => Some("Rgba8Unorm"),
            wgpu::TextureFormat::Rgba8UnormSrgb => Some("Rgba8UnormSrgb"),
            wgpu::TextureFormat::Bgra8Unorm => Some("Bgra8Unorm"),
            wgpu::TextureFormat::Bgra8UnormSrgb => Some("Bgra8UnormSrgb"),
            wgpu::TextureFormat::Rgba16Float => Some("Rgba16Float"),
            _ => None,
        };
        if let Some(requirements) = retained_format.and_then(|f| draw.retained_requirements(f)) {
            return requirements;
        }
        #[cfg(test)]
        QUERIES.with(|n| n.set(n.get() + 1));
        let key = (
            Arc::as_ptr(draw.native_instance()) as usize,
            Context::of(draw, format),
        );
        let mut entries = self.entries.borrow_mut();
        if let Some(entry) = entries.get(&key) {
            // The Weak keeps the allocation identity unambiguous without keeping
            // its resources/lease alive. COW mutations get another allocation.
            if entry.instance.as_ptr() == Arc::as_ptr(draw.native_instance()) {
                return entry.requirements.clone();
            }
        }
        #[cfg(test)]
        INTERPRETATIONS.with(|n| n.set(n.get() + 1));
        let pipeline = draw.pipeline_key(&format!("{format:?}"));
        let intent_pipeline = if format == wgpu::TextureFormat::Rgba8Unorm {
            pipeline.clone()
        } else {
            draw.pipeline_key("Rgba8Unorm")
        };
        let requirements = Arc::new(Requirements::from_native_facts(
            "renderer",
            pipeline,
            intent_pipeline,
            draw.material_textures(),
        ));
        if self.limit != 0 {
            if entries.len() >= self.limit {
                entries.clear();
            }
            entries.insert(
                key,
                Entry {
                    instance: Arc::downgrade(draw.native_instance()),
                    requirements: requirements.clone(),
                },
            );
        }
        requirements
    }

    pub(super) fn observe(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
        retained: &[Option<Arc<Requirements>>],
    ) {
        // Hash buckets, Weak control blocks and allocator overhead are not
        // exposed by std. Report actual separately retained payloads only.
        let entries = self.entries.borrow();
        let mut seen = std::collections::HashSet::new();
        // All addresses remain live through this call. This only merges identical
        // CPU allocations with the same owner/role, never GPU admission facts.
        for requirements in entries
            .values()
            .map(|entry| &entry.requirements)
            .chain(retained.iter().flatten())
        {
            if seen.insert(Arc::as_ptr(requirements) as usize) {
                sample.cpu_arc(requirements, requirements.backing_owner(), &["resident"]);
            }
            for pipeline in [requirements.pipeline(), requirements.intent_pipeline()] {
                if seen.insert(Arc::as_ptr(pipeline) as *const () as usize) {
                    sample.cpu_arc(pipeline, "renderer", &["resident"]);
                }
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    pub(super) static QUERIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static INTERPRETATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_dependencies_query_actual_collections_once_without_hiding_changes() {
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let mut draw = plan
                .commands
                .iter()
                .find_map(geometry_draw)
                .unwrap()
                .clone();
            draw.standard_lit = Some(crate::render_contract::StandardLitParameters {
                surface: [0f32, 0.5, 1., 1.].map(Into::into),
                emissive: [0f32; 4].map(Into::into),
            });
            let projection = crate::render_projection_adapter::LightProjection {
                component: crate::spatial_contract::Light3D {
                    schema_version: "light3d.v1".into(),
                    enabled: true,
                    color: [1.; 3],
                    kind: crate::spatial_contract::LightKind::Directional {
                        illuminance_lux: 1.,
                    },
                    layers: vec![],
                    cast_shadows: true,
                },
                position: [0.; 3],
                direction: [0., 0., -1.],
                origin: [0.; 3],
            };
            let light =
                crate::render_geometry::ClusteredLights::build(&draw.view, &[projection]).unwrap();
            draw.lighting = Some(light.clone());
            draw.shadows = Some(
                crate::render_geometry::ShadowMaps::build(
                    &draw.view,
                    &light,
                    vec![crate::render_geometry::ShadowProjection {
                        atlas: "test-depth".into(),
                        slot: 0,
                        light_index: 0,
                        matrix: crate::render_contract::IDENTITY_MATRIX.map(Into::into),
                        viewport: [0, 0, 16, 16],
                        extent: [16, 16],
                        depth_range: [0f32.into(), 10f32.into()],
                    }],
                )
                .unwrap(),
            );
            let mut words = vec![0u32; 32 + 6 * (4 + 1) * 3 + 32 * 32 * 2];
            words[..4].copy_from_slice(&[crate::environment_lighting::ENVIRONMENT_MAGIC, 2, 2, 32]);
            let map = crate::environment_lighting::EnvironmentMap::from_bytes(
                &words
                    .iter()
                    .flat_map(|w| w.to_le_bytes())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            draw.environment = Some(Arc::new(crate::environment_lighting::EnvironmentLighting {
                map,
                intensity: 1f32.into(),
            }));
            let memo = Preparation::default();
            let facts = memo.resolve(&draw, wgpu::TextureFormat::Rgba8Unorm);
            let mut bindings = Bindings::default();
            let mut used = BTreeSet::new();
            DEPENDENCY_QUERIES.with(|n| n.set([0; 5]));
            for _ in 0..count {
                bindings.include(&draw, &facts);
                bindings.include_pipeline(&facts, &mut used);
            }
            actual.push((count, DEPENDENCY_QUERIES.with(|n| n.get())));
            assert_eq!(bindings.lights.len(), 1);
            assert_eq!(bindings.environments.len(), 1);
            assert_eq!(bindings.shadows.len(), 1);
            assert_eq!(bindings.materials.len(), 1);
            assert_eq!(used.len(), 1);
            let mut next = draw.clone();
            Arc::make_mut(&mut next.view).view_id += 1;
            bindings.include(&next, &facts);
            assert_eq!(
                bindings.shadows.len(),
                2,
                "same shadow allocation for a new view was lost"
            );
            let mut detached = draw.clone();
            detached.lighting = Some(Arc::new(light.as_ref().clone()));
            bindings.include(&detached, &facts);
            assert!(
                std::ptr::eq(bindings.lights[light.identity()], light.as_ref()),
                "first identity selection changed"
            );
            let mut changed = draw.clone();
            let mut projections = draw.shadows.as_ref().unwrap().projections().to_vec();
            projections[0].matrix[12] = 1f32.into();
            let maps =
                crate::render_geometry::ShadowMaps::build(&draw.view, &light, projections).unwrap();
            changed.shadows = Some(maps.clone());
            bindings.include(&changed, &facts);
            assert!(
                std::ptr::eq(bindings.shadows[&draw.view.view_id], maps.as_ref()),
                "new shadow value was hidden by preceding exact reference"
            );
            let changed_pipeline = Requirements::from_native_facts(
                "renderer",
                "changed-pipeline".into(),
                "changed-pipeline".into(),
                *facts.textures(),
            );
            assert!(bindings.include_pipeline(&changed_pipeline, &mut used));
            assert_eq!(used.len(), 2);
            let mut shadow = draw.clone();
            Arc::make_mut(&mut shadow.view).target_kind =
                crate::render_contract::TargetKind::ShadowMap;
            shadow.blend = MaterialBlend::Opaque;
            shadow.depth = MaterialDepth::TestAndWrite;
            let mut after_shadow = Bindings::default();
            after_shadow.include(&shadow, &facts);
            assert!(after_shadow.materials.is_empty());
            after_shadow.include(&draw, &facts);
            assert_eq!(
                after_shadow.materials.len(),
                1,
                "opaque depth writer hid a subsequent color material"
            );
        }
        eprintln!(
            "actual light/environment/shadow/material/pipeline collection queries: {actual:?}"
        );
        assert!(
            actual.iter().all(|(_, n)| *n == [1; 5]),
            "repeated exact dependencies still queried collections: {actual:?}"
        );
    }
    #[test]
    fn retained_and_fallback_requirements_share_one_memory_owner_for_shared_backing() {
        let graph = crate::render_geometry::retained_command_test_graph();
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let draw = plan.commands.iter().find_map(geometry_draw).unwrap();
        let memo = Preparation::default();
        let facts = [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Bgra8Unorm,
        ]
        .map(|format| Some(memo.resolve(draw, format)));
        let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
        draw.observe_memory(&mut sample, "snapshot");
        memo.observe(&mut sample, &facts);
        let totals = crate::resource_memory::memory_totals(&sample.allocations).unwrap();
        assert!(totals.cpu_allocated > 0);
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn retained_commands_are_consumed_natively_but_never_replace_current_resource_admission() {
        let graph = crate::render_geometry::retained_command_test_graph();
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        QUERIES.with(|n| n.set(0));
        let first = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        assert_eq!(
            QUERIES.with(|n| n.get()),
            0,
            "native expanded retained command into old preparation"
        );
        let uploaded = backend.geometry.uploaded_meshes;
        // Readback completed the old frame. Removing actual storage must force
        // readmission even with the exact same retained command and plan.
        backend.geometry.meshes.clear();
        let second = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        assert_eq!(first, second);
        assert_eq!(backend.geometry.uploaded_meshes, uploaded + 1);
        assert_eq!(QUERIES.with(|n| n.get()), 0);
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn stable_preparation_real_consumers_reuse_facts_but_readmit_missing_meshes() {
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            let mut plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
            let uploaded = backend.geometry.uploaded_meshes;
            let old = plan.commands.weak_backing();
            for command in &mut plan.commands {
                match command {
                    RhiCommand::BeginView { view, .. } => {
                        Arc::make_mut(view).view_matrix[12] = 0.25f32.into();
                    }
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => {
                        Arc::make_mut(&mut Arc::make_mut(draw).view).view_matrix[12] =
                            0.25f32.into();
                    }
                    _ => {}
                }
            }
            drop(graph);
            assert!(old.upgrade().is_none());
            // Offscreen readback above completed the old submission. Simulate a
            // cold resident table while the CPU facts remain warm. A memo hit
            // must neither suppress upload nor retain a dead native buffer.
            backend.geometry.meshes.clear();
            INTERPRETATIONS.with(|n| n.set(0));
            backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
            actual.push((count, INTERPRETATIONS.with(|n| n.get())));
            assert_eq!(backend.geometry.uploaded_meshes, uploaded + 1);
            assert_eq!(
                backend
                    .geometry
                    .validated_plan
                    .as_ref()
                    .unwrap()
                    .meshes
                    .len(),
                1
            );
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
        eprintln!(
            "real prepare + recipe interpretations after view/command/resident change: {actual:?}"
        );
        assert!(
            actual.iter().all(|(_, visits)| *visits == 0),
            "downstream reinterpreted stable facts: {actual:?}"
        );
    }

    #[test]
    fn stable_preparation_context_cow_expiry_and_capacity_are_conservative() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let draw = plan
            .commands
            .iter()
            .find_map(geometry_draw)
            .unwrap()
            .clone();
        let memo = Preparation::default();
        let base = memo.resolve(&draw, wgpu::TextureFormat::Rgba8Unorm);
        assert!(Arc::ptr_eq(
            &base,
            &memo.resolve(&draw, wgpu::TextureFormat::Rgba8Unorm)
        ));
        let mut variants = Vec::new();
        let mut v = draw.clone();
        v.depth_target = Some("depth".into());
        variants.push(v);
        let mut v = draw.clone();
        v.depth = MaterialDepth::TestAndWrite;
        variants.push(v);
        let mut v = draw.clone();
        Arc::make_mut(&mut v.view).domain = RenderDomain::World2d;
        variants.push(v);
        let mut v = draw.clone();
        Arc::make_mut(&mut v.view).target_kind = TargetKind::ShadowMap;
        variants.push(v);
        let mut v = draw.clone();
        v.blend = MaterialBlend::AlphaBlend;
        variants.push(v);
        let mut v = draw.clone();
        v.world_transform[0] = (-1f32).into();
        variants.push(v);
        let mut v = draw.clone();
        Arc::make_mut(&mut v.view).color_pipeline = Some(Default::default());
        variants.push(v);
        let mut v = draw.clone();
        v.lighting = Some(crate::render_geometry::ClusteredLights::build(&v.view, &[]).unwrap());
        variants.push(v);
        for v in variants {
            let next = memo.resolve(&v, wgpu::TextureFormat::Rgba8Unorm);
            assert!(
                !Arc::ptr_eq(&base, &next),
                "changed pipeline context reused old facts"
            );
            assert_eq!(next.pipeline, v.pipeline_key("Rgba8Unorm"));
            assert_eq!(next.intent_pipeline, v.pipeline_key("Rgba8Unorm"));
            assert_eq!(next.textures, v.material_textures());
        }
        let changed_format = memo.resolve(&draw, wgpu::TextureFormat::Rgba16Float);
        assert!(!Arc::ptr_eq(&base, &changed_format));
        assert_eq!(changed_format.pipeline, draw.pipeline_key("Rgba16Float"));
        assert_eq!(
            changed_format.intent_pipeline,
            draw.pipeline_key("Rgba8Unorm")
        );
        assert_ne!(changed_format.pipeline, changed_format.intent_pipeline);
        // Same visible instance ID is not identity. A changed immutable allocation
        // must not inherit an old result even if its values happen to compare equal.
        let mut detached = draw.clone();
        Arc::make_mut(&mut detached.instance).instance_id = draw.instance_id;
        assert!(!Arc::ptr_eq(
            &base,
            &memo.resolve(&detached, wgpu::TextureFormat::Rgba8Unorm)
        ));
        let weak = Arc::downgrade(draw.native_instance());
        let lease = Arc::downgrade(draw.native_resource_lease());
        drop(detached);
        drop(draw);
        drop(plan);
        drop(graph);
        assert!(weak.upgrade().is_none(), "preparation retained an instance");
        assert!(
            lease.upgrade().is_none(),
            "preparation retained a resource lease"
        );
        for limit in [0, 1] {
            let memo = Preparation {
                limit,
                ..Default::default()
            };
            let graph = crate::rhi_command_plan::tests::retained_geometry_graph(4);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            for _ in 0..2 {
                for draw in plan.commands.iter().filter_map(geometry_draw) {
                    let fact = memo.resolve(draw, wgpu::TextureFormat::Rgba8Unorm);
                    assert_eq!(fact.pipeline, draw.pipeline_key("Rgba8Unorm"));
                }
            }
            assert!(memo.entries.borrow().len() <= limit);
        }
    }

    #[test]
    fn stable_preparation_moving_views_reuse_only_stable_dependencies_at_multiple_scales() {
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let memo = Preparation::default();
            let old = ValidatedGeometryPlan::capture(
                &plan,
                [640, 480],
                None,
                wgpu::TextureFormat::Rgba8Unorm,
            )
            .unwrap();
            let draws: Vec<_> = plan
                .commands
                .iter()
                .filter_map(geometry_draw)
                .cloned()
                .collect();
            let facts: Vec<_> = draws
                .iter()
                .map(|d| memo.resolve(d, wgpu::TextureFormat::Rgba8Unorm))
                .collect();
            // A moving view makes full-command success ineligible; pure resource
            // interpretation must remain reusable even after old commands expire.
            drop(plan);
            drop(graph);
            INTERPRETATIONS.with(|n| n.set(0));
            for (mut draw, prior) in draws.into_iter().zip(facts) {
                Arc::make_mut(&mut draw.view).view_matrix[12] = 0.25f32.into();
                let fact = memo.resolve(&draw, wgpu::TextureFormat::Rgba8Unorm);
                assert_eq!(fact.pipeline, draw.pipeline_key("Rgba8Unorm"));
                assert_eq!(fact.textures, draw.material_textures());
                assert_eq!(prior.pipeline, fact.pipeline);
            }
            assert!(old.command_backing.upgrade().is_none());
            actual.push((count, INTERPRETATIONS.with(|n| n.get())));
        }
        eprintln!("actual resource/pipeline interpretations after camera-only change: {actual:?}");
        assert!(
            actual.iter().all(|(_, visits)| *visits == 0),
            "stable preparation reinterpreted after view-only change: {actual:?}"
        );
    }
}
