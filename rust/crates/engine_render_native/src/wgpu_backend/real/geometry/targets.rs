//! Independent color outputs in the existing native geometry resource owner.
use super::*;
use crate::render_graph::{RenderResourceFormat, RenderResourceUsage};

#[cfg(test)]
thread_local! {
    pub(super) static OUTPUT_LAYOUT_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static OUTPUT_PREPARATION_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static OUTPUT_DESCRIPTOR_CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(in crate::wgpu_backend) fn primary(plan: &RhiCommandPlan) -> Option<&str> {
    plan.commands
        .iter()
        .find_map(|c| match c {
            RhiCommand::BeginFrame { target } => Some(target.as_str()),
            _ => None,
        })
        .or_else(|| {
            plan.commands.iter().find_map(|c| match c {
                RhiCommand::BeginView { target, view, .. } if !view.is_shadow_map() => {
                    Some(target.as_str())
                }
                RhiCommand::Clear { target, .. } => Some(target.as_str()),
                RhiCommand::Draw {
                    target, payload, ..
                } if !payload
                    .geometry_draw()
                    .is_some_and(|draw| draw.view.is_shadow_map()) =>
                {
                    Some(target.as_ref())
                }
                _ => None,
            })
        })
}

impl RealWgpuBackend {
    pub(in crate::wgpu_backend) fn geometry_color_layout(
        &self,
        plan: &RhiCommandPlan,
    ) -> Result<BTreeMap<String, [u32; 2]>, String> {
        #[cfg(test)]
        OUTPUT_LAYOUT_CALLS.with(|n| n.set(n.get() + 1));
        color_layout(
            plan,
            plan.commands.iter(),
            self.format,
            self.device.limits().max_texture_dimension_2d,
        )
    }
}

// Keep descriptor/consumer validation independent of a live device. The iterator also lets
// owner tests bound command traversal without a GPU or a wall-clock performance threshold.
fn color_layout<'a>(
    plan: &RhiCommandPlan,
    commands: impl Iterator<Item = &'a RhiCommand> + Clone,
    surface_format: wgpu::TextureFormat,
    max_texture_dimension_2d: u32,
) -> Result<BTreeMap<String, [u32; 2]>, String> {
    crate::color_pipeline::validate_plan(plan)?;
    let primary = primary(plan);
    // Resolve roles belong to the plan, not to each draw. Never rescan the
    // full command stream for every consumer of the same HDR/display target.
    let internal_sources: std::collections::BTreeSet<_> = commands
        .clone()
        .filter_map(|c| match c {
            RhiCommand::ResolveColor3d { resolve } => Some(resolve.source.as_str()),
            _ => None,
        })
        .collect();
    let mut layout = BTreeMap::new();
    for command in commands.clone() {
        let id = match command {
            RhiCommand::BeginView { view, .. } if view.is_shadow_map() => continue,
            c if c
                .geometry_draw()
                .is_some_and(|draw| draw.view.is_shadow_map()) =>
            {
                continue
            }
            RhiCommand::BeginView { target, .. } | RhiCommand::Clear { target, .. } => {
                target.as_str()
            }
            RhiCommand::Draw { target, .. } => target.as_ref(),
            RhiCommand::ResolveColor3d { resolve } => resolve.target.as_str(),
            RhiCommand::Present { target } => {
                if primary.is_some_and(|p| p != target) {
                    return Err("geometry.secondary_present_unsupported".into());
                }
                continue;
            }
            _ => continue,
        };
        if primary.is_none() || primary == Some(id) {
            continue;
        }
        let internal = internal_sources.contains(id);
        // Reuse only a descriptor validated in this immutable call. Consumers
        // below still validate independently, including after a repeated ID.
        let extent = if let Some(extent) = layout.get(id) {
            *extent
        } else {
            let schedule = plan
                .graph_schedule
                .as_ref()
                .ok_or("geometry.output_schedule_missing")?;
            let resource = schedule
                .resources
                .iter()
                .find(|r| r.resource_id == id)
                .ok_or("geometry.output_resource_missing")?;
            #[cfg(test)]
            OUTPUT_DESCRIPTOR_CHECKS.with(|n| n.set(n.get() + 1));
            let format = match resource.format {
                RenderResourceFormat::Rgba16Float if internal => wgpu::TextureFormat::Rgba16Float,
                RenderResourceFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
                RenderResourceFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
                RenderResourceFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
                RenderResourceFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
                _ => return Err("geometry.output_format_unsupported".into()),
            };
            // Pipeline/clear recipes belong to this backend's fixed color format.
            if !internal && format != surface_format {
                return Err("geometry.output_format_mismatch".into());
            }
            if !resource.usage.contains(&RenderResourceUsage::ColorTarget)
                || resource.usage.contains(&RenderResourceUsage::Present)
                || resource.usage.contains(&RenderResourceUsage::DepthTarget)
                || (resource.usage.contains(&RenderResourceUsage::Sampled) && !internal)
                || resource.resource_kind != crate::render_graph::RenderResourceKind::Texture
            {
                return Err("geometry.output_usage_unsupported".into());
            }
            let extent = [resource.size.width, resource.size.height];
            if extent.contains(&0) || extent.iter().any(|v| *v > max_texture_dimension_2d) {
                return Err("geometry.output_extent_invalid".into());
            }
            layout.insert(id.to_owned(), extent);
            extent
        };
        match command {
            RhiCommand::BeginView { view, .. }
                if view.domain == RenderDomain::World3d
                    && (view.target_kind != TargetKind::Window || internal)
                    && view.target_extent == extent => {}
            c if c.geometry_draw().is_some_and(|draw| {
                (draw.view.target_kind != TargetKind::Window || internal)
                    && draw.view.target_extent == extent
            }) => {}
            RhiCommand::Draw {
                payload: RhiDrawPayload::Particles { view, .. },
                ..
            } if view.scene_view.is_some() => (),
            RhiCommand::Clear { .. } | RhiCommand::ResolveColor3d { .. } => (),
            _ => return Err("geometry.output_consumer_unsupported".into()),
        }
    }
    if layout.len() > 47 {
        return Err("geometry.output_capacity".into());
    }
    let external = layout
        .keys()
        .filter(|id| !internal_sources.contains(id.as_str()))
        .count();
    if external > 15 {
        return Err("geometry.output_capacity".into());
    }
    Ok(layout)
}

impl RealWgpuBackend {
    pub(in crate::wgpu_backend) fn prepare_geometry_color_targets(
        &mut self,
        plan: &RhiCommandPlan,
    ) -> Result<(), String> {
        #[cfg(test)]
        OUTPUT_PREPARATION_CALLS.with(|n| n.set(n.get() + 1));
        if let Some(cached) = self.geometry.validated_plan.as_mut() {
            cached.outputs_prepared = false;
        }
        // Resource IDs and extents do not determine the allocation format.
        // Resolve sources may change between HDR and display storage across plans.
        let layout: BTreeMap<_, _> = self
            .geometry_color_layout(plan)?
            .into_iter()
            .map(|(id, extent)| {
                let format = plan
                    .graph_schedule
                    .as_ref()
                    .and_then(|s| s.resources.iter().find(|r| r.resource_id == id))
                    .map(|r| match r.format {
                        RenderResourceFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
                        RenderResourceFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
                        _ => self.format,
                    })
                    .unwrap_or(self.format);
                (id, (extent, format))
            })
            .collect();
        let new_bytes: u64 = layout
            .iter()
            .filter(|(id, (extent, format))| {
                self.geometry
                    .colors
                    .get(*id)
                    .is_none_or(|(w, h, v)| [*w, *h] != *extent || v.texture().format() != *format)
            })
            .map(|(_, ([w, h], format))| {
                u64::from(*w)
                    * u64::from(*h)
                    * if *format == wgpu::TextureFormat::Rgba16Float {
                        8
                    } else {
                        4
                    }
            })
            .sum();
        self.admit_geometry_backing(new_bytes)?;
        let removed: Vec<_> = self
            .geometry
            .colors
            .iter()
            .filter(|(id, (w, h, view))| {
                layout.get(*id) != Some(&([*w, *h], view.texture().format()))
            })
            .map(|(id, _)| id.clone())
            .collect();
        let bytes = removed
            .iter()
            .map(|id| {
                let (w, h, ref view) = self.geometry.colors[id];
                u64::from(w)
                    * u64::from(h)
                    * if view.texture().format() == wgpu::TextureFormat::Rgba16Float {
                        8
                    } else {
                        4
                    }
            })
            .sum();
        self.admit_gpu_retirement(bytes, removed.len())?;
        for id in removed {
            let old = self.geometry.colors.remove(&id).unwrap();
            let bytes = u64::from(old.0)
                * u64::from(old.1)
                * if old.2.texture().format() == wgpu::TextureFormat::Rgba16Float {
                    8
                } else {
                    4
                };
            self.retire_gpu(old, bytes);
        }
        for (id, ([width, height], format)) in layout {
            self.geometry.colors.entry(id).or_insert_with(|| {
                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("geometry-independent-color-output"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::COPY_SRC
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                (width, height, texture.create_view(&Default::default()))
            });
        }
        Ok(())
    }

    /// Capture one current independent output. This is an ordered GPU readback,
    /// not a second render or an OS presentation claim. No native handle escapes.
    pub fn read_geometry_color_target_rgba(&mut self, target: &str) -> Result<Vec<u8>, String> {
        let texture = self
            .geometry
            .colors
            .get(target)
            .ok_or("geometry.output_not_resident")?
            .2
            .texture()
            .clone();
        if texture.format() == wgpu::TextureFormat::Rgba16Float {
            return Err("color.hdr_rgba8_readback_unsupported".into());
        }
        self.read_texture_rgba_bytes(&texture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_graph::*;
    #[test]
    #[ignore = "real GPU required"]
    fn gpu_successful_capability_proof_reuses_only_same_commands_and_schedule() {
        // A secondary output has an explicit format constraint; the primary
        // surface alone may legally accept the backend's selected format.
        let plan = outputs(64, RenderResourceFormat::Rgba8Unorm, 0.25);
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        // First successful native-plan proof may be established only after capture.
        backend.validate_graph_capabilities(&plan).unwrap();
        OUTPUT_LAYOUT_CALLS.with(|n| n.set(0));
        for _ in 0..8 {
            backend.validate_graph_capabilities(&plan).unwrap();
        }
        assert_eq!(
            OUTPUT_LAYOUT_CALLS.with(|n| n.get()),
            0,
            "same successful plan reconstructed and revalidated its output layout"
        );
        let mut changed = plan.clone();
        changed.commands.push(RhiCommand::Present {
            target: "color3d-1-hdr".into(),
        });
        assert!(backend.validate_graph_capabilities(&changed).is_err());
        let mut schedule_changed = plan.clone();
        let schedule = Arc::make_mut(schedule_changed.graph_schedule.as_mut().unwrap());
        schedule.resources[0].format = RenderResourceFormat::Depth24Plus;
        assert_eq!(
            backend.validate_graph_capabilities(&schedule_changed),
            Err("graph.unsupported_backend_depth_format".into())
        );
        backend.format = wgpu::TextureFormat::Bgra8Unorm;
        assert!(backend.validate_graph_capabilities(&plan).is_err());
    }

    #[test]
    #[ignore = "real GPU required"]
    fn gpu_successful_surface_output_preparation_is_reused_by_real_draw() {
        let plan = color_plan(false);
        let mut backend = RealWgpuBackend::new_offscreen_measured(64, 64, 2).unwrap();
        OUTPUT_PREPARATION_CALLS.with(|n| n.set(0));
        assert!(backend
            .prepare_surface_resources(&plan, [64, 64], None)
            .unwrap());
        let first_checks = OUTPUT_PREPARATION_CALLS.with(|n| n.get());
        assert_eq!(first_checks, 1);
        assert!(backend
            .prepare_surface_resources(&plan, [64, 64], None)
            .unwrap());
        assert_eq!(
            OUTPUT_PREPARATION_CALLS.with(|n| n.get()),
            first_checks,
            "same successful plan repeated the output/post preparation"
        );
        let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        assert_eq!(
            OUTPUT_PREPARATION_CALLS.with(|n| n.get()),
            first_checks,
            "real draw repeated already successful surface preparation"
        );
        let mut reference = RealWgpuBackend::new_offscreen_measured(64, 64, 2).unwrap();
        let expected = reference.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        assert_eq!(pixels, expected);
        let changed = color_plan_with(false, 1., crate::color_pipeline::AntiAliasing3D::Fxaa);
        let actual = backend.render_plan_to_rgba_bytes(&changed, 64, 64).unwrap();
        assert_ne!(
            actual, pixels,
            "changed exposure retained stale post parameters"
        );
        let expected = reference
            .render_plan_to_rgba_bytes(&changed, 64, 64)
            .unwrap();
        assert_eq!(actual, expected);
    }
    #[test]
    fn color_target_validation_traversal_is_bounded_by_commands_not_draws_squared() {
        OUTPUT_DESCRIPTOR_CHECKS.with(|n| n.set(0));
        let mut plan = color_plan(false);
        let before_resolve = plan
            .commands
            .iter()
            .position(|c| matches!(c, RhiCommand::ResolveColor3d { .. }))
            .unwrap();
        plan.commands.splice(
            before_resolve..before_resolve,
            (0..512).map(|_| RhiCommand::Clear {
                target: "color3d-1-hdr".into(),
                color: crate::render_graph::color([0., 0., 0., 1.]),
            }),
        );
        let visits = std::cell::Cell::new(0);
        let commands = plan
            .commands
            .iter()
            .inspect(|_| visits.set(visits.get() + 1));
        let layout = color_layout(&plan, commands, wgpu::TextureFormat::Rgba8Unorm, 16384).unwrap();
        assert_eq!(layout["color3d-1-hdr"], [64, 64]);
        assert_eq!(layout["color3d-1-display"], [64, 64]);
        assert_eq!(
            OUTPUT_DESCRIPTOR_CHECKS.with(|n| n.get()),
            2,
            "immutable output descriptors must be validated once, not for every consumer"
        );
        assert!(
            visits.get() <= plan.commands.len() * 8,
            "{} visits for {} commands",
            visits.get(),
            plan.commands.len()
        );
        let mut invalid = plan.clone();
        invalid.commands.push(RhiCommand::Draw {
            target: "color3d-1-hdr".into(),
            draw_kind: RhiDrawKind::TestGeometry,
            vertex_count: 3,
            payload: RhiDrawPayload::TestGeometry {
                debug_label: "invalid HDR consumer".into(),
            },
        });
        assert_eq!(
            color_layout(
                &invalid,
                invalid.commands.iter(),
                wgpu::TextureFormat::Rgba8Unorm,
                16384
            ),
            Err("geometry.output_consumer_unsupported".into())
        );
        // A repeated valid target must not hide a later illegal consumer, even
        // when frame identity and resource descriptors have not changed.
        plan.commands.push(RhiCommand::Present {
            target: "color3d-1-hdr".into(),
        });
        assert_eq!(
            color_layout(
                &plan,
                plan.commands.iter(),
                wgpu::TextureFormat::Rgba8Unorm,
                16384
            ),
            Err("geometry.secondary_present_unsupported".into())
        );
    }
    fn color_plan(swapped: bool) -> RhiCommandPlan {
        color_plan_with(swapped, 0., crate::color_pipeline::AntiAliasing3D::Fxaa)
    }
    fn color_plan_with(
        swapped: bool,
        exposure: f32,
        aa: crate::color_pipeline::AntiAliasing3D,
    ) -> RhiCommandPlan {
        use crate::render_contract::*;
        use crate::render_geometry::{GeometryFrame, GeometryViewInput};
        let mut camera = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
        camera.clear_policy = ClearPolicy::Color {
            color: [4., 0.5, 0.18, 1.],
        };
        camera.color_pipeline = Some(crate::color_pipeline::ColorPipeline3D {
            exposure_ev: exposure.into(),
            anti_aliasing: aa,
            ..Default::default()
        });
        let frame = GeometryFrame::build(
            "main",
            [64, 64],
            &[GeometryViewInput {
                view: &camera,
                pose: CameraPose::default(),
            }],
            &[],
            &crate::render_resource::RenderResourceManager::new(),
            DiagnosticLevel::Off,
        )
        .unwrap();
        let mut graph = RenderGraph::new("s2d-format-transition", 1);
        graph.output_target = Some("main".into());
        graph
            .resources
            .push(RenderResource::surface_backbuffer("main", 64, 64));
        frame.append_to_graph(&mut graph);
        if swapped {
            // Resource names are opaque graph IDs, not a format declaration.
            let wire = serde_json::to_string(&graph)
                .unwrap()
                .replace("color3d-1-hdr", "s2d-swap-placeholder")
                .replace("color3d-1-display", "color3d-1-hdr")
                .replace("s2d-swap-placeholder", "color3d-1-display");
            graph = serde_json::from_str(&wire).unwrap();
        }
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        crate::color_pipeline::validate_plan(&plan).unwrap();
        plan
    }

    #[test]
    #[ignore = "requires real GPU"]
    fn gpu_s2d_joint_budget_rejects_color_batch_before_allocating() {
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        backend
            .set_geometry_visual_resident_limit(4096 * 64 + 64 * 64 * 12 - 1)
            .unwrap();
        let result = backend.prepare_geometry_color_targets(&color_plan(false));
        assert_eq!(result, Err("geometry.visual_resident_budget".into()));
        assert!(
            backend.geometry.colors.is_empty(),
            "failed batch allocated textures"
        );
    }

    #[test]
    #[ignore = "requires real GPU"]
    fn gpu_s2d_joint_budget_counts_shared_sources_and_preserves_failed_replacements() {
        use crate::resource_memory::{memory_totals, ResourceMemorySample};
        fn agree(backend: &RealWgpuBackend) -> u64 {
            let mut report = ResourceMemorySample::new(0, 0, 0);
            backend.observe_memory(&mut report);
            let bytes = backend.geometry_backing_bytes();
            assert_eq!(
                bytes,
                memory_totals(&report.allocations).unwrap().gpu_allocated
            );
            bytes
        }
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let first = color_plan(false);
        let next = color_plan(true);
        let pixels = backend.render_plan_to_rgba_bytes(&first, 64, 64).unwrap();
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        let fixed = agree(&backend);
        assert_eq!(fixed, 4096 * 64 + 64 * 64 * (8 + 4 + 4 + 4) + 144 + 2 * 32);
        // Raster output plus its aligned readback are now admitted native owners.
        let capture = 64 * 64 * 4 + 256 * 64;
        backend
            .set_geometry_visual_resident_limit(fixed + capture)
            .unwrap();
        assert_eq!(
            backend.render_plan_to_rgba_bytes(&first, 64, 64).unwrap(),
            pixels
        );
        backend.set_geometry_visual_resident_limit(fixed).unwrap();
        assert_eq!(
            backend.set_geometry_visual_resident_limit(fixed - 1),
            Err("geometry.visual_resident_budget".into())
        );
        assert_eq!(
            backend.prepare_geometry_color_targets(&next),
            Err("geometry.visual_resident_budget".into())
        );
        assert_eq!(
            backend.geometry.colors["color3d-1-hdr"]
                .2
                .texture()
                .format(),
            wgpu::TextureFormat::Rgba16Float
        );
        assert_eq!(agree(&backend), fixed);
        // Old post bindings still own both sources even after target replacement.
        // Adding lifecycle aliases must not count the same native allocation twice.
        backend
            .set_geometry_visual_resident_limit(fixed + 64 * 64 * 12)
            .unwrap();
        backend.prepare_geometry_color_targets(&next).unwrap();
        assert_eq!(agree(&backend), fixed + 64 * 64 * 12);
        backend.prepare_color_resolves(&next).unwrap();
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(agree(&backend), fixed);
        backend
            .set_geometry_visual_resident_limit(fixed + capture)
            .unwrap();
        assert_eq!(
            backend.render_plan_to_rgba_bytes(&next, 64, 64).unwrap(),
            pixels
        );
    }

    #[test]
    #[ignore = "requires real GPU"]
    fn gpu_s2d_joint_budget_combines_color_and_depth_before_exceeding_limit() {
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        backend
            .set_geometry_visual_resident_limit(4096 * 64 + 64 * 64 * 16 - 1)
            .unwrap();
        let first = color_plan(false);
        backend.prepare_geometry_color_targets(&first).unwrap();
        assert_eq!(
            backend
                .render_plan_to_rgba_bytes(&first, 64, 64)
                .unwrap_err(),
            "geometry.visual_resident_budget"
        );
        assert!(backend.geometry.depths.is_empty());
        assert_eq!(backend.gpu_lifecycle_stats().submitted, 0);
        assert_eq!(backend.geometry_backing_bytes(), 4096 * 64 + 64 * 64 * 12);
    }

    #[test]
    #[ignore = "requires real GPU"]
    fn gpu_s2d_same_extent_format_transition_replaces_backing_and_preserves_pixels() {
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let first = color_plan(false);
        let next = color_plan(true);
        let reference = backend.render_plan_to_rgba_bytes(&first, 64, 64).unwrap();
        let old = backend.geometry.colors["color3d-1-hdr"].2.texture().clone();
        backend.prepare_geometry_color_targets(&next).unwrap();
        let current = backend.geometry.colors["color3d-1-hdr"].2.texture();
        assert_eq!(
            current.format(),
            wgpu::TextureFormat::Rgba8Unorm,
            "same extent does not make HDR and display backing interchangeable"
        );
        assert_ne!(&old, current);
        assert_eq!(old.format(), wgpu::TextureFormat::Rgba16Float);
        assert_eq!(
            backend.render_plan_to_rgba_bytes(&next, 64, 64).unwrap(),
            reference
        );
        assert_eq!(
            backend.render_plan_to_rgba_bytes(&first, 64, 64).unwrap(),
            reference
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
    }

    #[test]
    #[ignore = "requires real GPU"]
    fn gpu_s2d_fixed_profile_warmup_reuses_backing_and_parameter_changes_bound_variants() {
        use crate::color_pipeline::AntiAliasing3D;
        use crate::resource_memory::{memory_totals, ResourceMemorySample};
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let cold = crate::platform_time::Instant::now();
        let fixed = color_plan(false);
        let reference = backend.render_plan_to_rgba_bytes(&fixed, 64, 64).unwrap();
        let cold_ms = cold.elapsed().as_secs_f64() * 1000.;
        let backing: Vec<_> = backend
            .geometry
            .colors
            .iter()
            .map(|(k, v)| (k.clone(), v.2.texture().clone()))
            .collect();
        let warm = crate::platform_time::Instant::now();
        for _ in 0..8 {
            assert_eq!(
                backend.render_plan_to_rgba_bytes(&fixed, 64, 64).unwrap(),
                reference
            );
        }
        let warm_ms = warm.elapsed().as_secs_f64() * 1000. / 8.;
        let mut fixed_sample = ResourceMemorySample::new(0, 0, 0);
        backend.observe_memory(&mut fixed_sample);
        let fixed_bytes = memory_totals(&fixed_sample.allocations)
            .unwrap()
            .gpu_allocated;
        let textures: BTreeMap<_, _> = fixed_sample
            .allocations
            .iter()
            .filter(|a| {
                a.domain == crate::resource_memory::MemoryDomain::Gpu
                    && a.allocation_id.starts_with("texture:")
            })
            .map(|a| (&a.allocation_id, a.bytes))
            .collect();
        assert_eq!(
            textures.len(),
            4,
            "HDR, display, scene depth and the existing particle fallback depth have distinct backing"
        );
        assert_eq!(backend.geometry.depths.len(), 1);
        assert_eq!(textures.values().sum::<u64>(), 64 * 64 * (8 + 4 + 4 + 4));
        for i in 0..16 {
            let ev = if i % 2 == 0 { -2. } else { 2. };
            let aa = if i % 3 == 0 {
                AntiAliasing3D::None
            } else {
                AntiAliasing3D::Fxaa
            };
            let plan = color_plan_with(false, ev, aa);
            let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
            assert_ne!(pixels, reference, "exposure cannot be silently dropped");
            for (id, texture) in &backing {
                assert_eq!(
                    backend.geometry.colors[id].2.texture(),
                    texture,
                    "parameter change reallocated target"
                );
            }
            let mut sample = ResourceMemorySample::new(i, 0, 0);
            backend.observe_memory(&mut sample);
            assert_eq!(sample.capacities["colorResolvePasses"], 2);
            assert_eq!(
                sample.capacities["colorResolvePipelines"], 2,
                "parameters became shader variants"
            );
            // A synchronous capture has completed its submissions. Collect through
            // the existing owner before comparing resident/retired backing totals.
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
            let mut drained = ResourceMemorySample::new(i, 0, 0);
            backend.observe_memory(&mut drained);
            assert_eq!(
                memory_totals(&drained.allocations).unwrap().gpu_allocated,
                fixed_bytes
            );
            assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
        }
        println!("s2d_dev_color_only_cold_ms={cold_ms:.3} warm_mean_with_readback_ms={warm_ms:.3} warm_frames=8 parameter_frames=16 pipelines=2 tracked_gpu_bytes={fixed_bytes}");
    }
    fn outputs(size: u32, format: RenderResourceFormat, value: f32) -> RhiCommandPlan {
        let mut graph = RenderGraph::new("independent-output-owner", 1);
        graph.output_target = Some("main".into());
        graph
            .resources
            .push(RenderResource::surface_backbuffer("main", 64, 64));
        graph.resources.push(RenderResource {
            resource_id: "second".into(),
            resource_name: "second".into(),
            resource_kind: crate::render_graph::RenderResourceKind::Texture,
            format,
            size: RenderResourceSize {
                width: size,
                height: size,
            },
            usage: vec![RenderResourceUsage::ColorTarget],
            lifetime: RenderResourceLifetime::Persistent,
        });
        for (id, color) in [
            ("main", [1., 0., 0., 1.]),
            ("second", [value, value, value, 1.]),
        ] {
            graph.passes.push(RenderPass {
                pass_id: id.into(),
                pass_name: id.into(),
                pass_kind: RenderPassKind::Clear,
                view_id: "global".into(),
                reads: vec![],
                writes: vec![id.into()],
                color_targets: vec![id.into()],
                depth_target: None,
                commands: vec![RenderPassCommand::Clear {
                    target: id.into(),
                    color: crate::render_graph::color(color),
                }]
                .into(),
                debug_source: None,
            });
        }
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        plan
    }
    #[test]
    #[ignore = "requires real GPU"]
    fn gpu_independent_output_formats_and_color_only_plans() {
        let instance = wgpu::Instance::new(&Default::default());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        for (graph_format, native, expected) in [
            (
                RenderResourceFormat::Rgba8Unorm,
                wgpu::TextureFormat::Rgba8Unorm,
                64,
            ),
            (
                RenderResourceFormat::Bgra8Unorm,
                wgpu::TextureFormat::Bgra8Unorm,
                64,
            ),
            (
                RenderResourceFormat::Rgba8UnormSrgb,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                137,
            ),
            (
                RenderResourceFormat::Bgra8UnormSrgb,
                wgpu::TextureFormat::Bgra8UnormSrgb,
                137,
            ),
        ] {
            let mut backend = RealWgpuBackend::from_shared_device_queue(
                &device,
                &queue,
                native,
                64,
                64,
                "independent output formats",
            );
            let plan = outputs(32, graph_format, 0.25);
            let primary = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
            let secondary = backend.read_geometry_color_target_rgba("second").unwrap();
            assert!(primary.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
            assert!(
                secondary
                    .chunks_exact(4)
                    .all(|p| p[..3].iter().all(|v| v.abs_diff(expected) <= 1) && p[3] == 255),
                "{native:?}: {:?}",
                &secondary[..4]
            );
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
    }
    #[test]
    #[ignore = "requires real GPU"]
    fn gpu_independent_output_resize_and_retirement_follow_actual_completion() {
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let output = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("independent-output-retirement"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = output.create_view(&Default::default());
        let first = outputs(64, RenderResourceFormat::Rgba8Unorm, 0.25);
        backend.render_to_view(&first, &view, None).unwrap();
        let before = backend.gpu_lifecycle_stats();
        // Use the real allocation/retirement owner directly without polling away
        // the still-uncollected completion between resize and observation.
        let next = outputs(32, RenderResourceFormat::Rgba8Unorm, 0.75);
        backend.prepare_geometry_color_targets(&next).unwrap();
        let sample = backend.gpu_lifecycle_stats();
        if before.completed < before.submitted {
            assert!(sample.retiring_bytes >= 64 * 64 * 4);
            let mut memory = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
            backend.observe_memory(&mut memory);
            assert!(memory
                .allocations
                .iter()
                .any(|a| a.domain == crate::resource_memory::MemoryDomain::Gpu
                    && a.bytes == 64 * 64 * 4
                    && a.roles.contains("retiring")));
            assert!(memory
                .allocations
                .iter()
                .any(|a| a.domain == crate::resource_memory::MemoryDomain::Gpu
                    && a.bytes == 32 * 32 * 4
                    && a.roles.contains("resident")));
        }
        backend.render_to_view(&next, &view, None).unwrap();
        backend.render_to_view(&first, &view, None).unwrap();
        assert_eq!(backend.gpu_lifecycle_stats().submitted, 3);
        assert!(backend
            .read_geometry_color_target_rgba("second")
            .unwrap()
            .chunks_exact(4)
            .all(|p| p == [64, 64, 64, 255]));
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
        assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
        backend.abandon_device_epoch();
        assert!(backend.geometry.colors.is_empty());
    }
}
