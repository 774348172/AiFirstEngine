//! Camera matrix changes within the existing successful native plan. The
//! skeleton owns no draw/instance/lease. Every reuse reads today's native owners.
use super::*;

enum Command {
    Control(Box<RhiCommand>),
    Draw {
        // Successful preparation admits one BeginView per view ID. Its exact
        // retained control owns target/depth; today's attachments still match
        // that control below. Repeating those Arcs per draw adds no constraint.
        view: u64,
        instances: usize,
        batch: bool,
        vertices: u32,
    },
}
pub(super) struct MovingViews(Vec<Command>);
#[cfg(test)]
thread_local! {pub(super) static CAPTURE_DRAW_VISITS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
thread_local! {pub(super) static TARGET_COPIES: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
thread_local! {pub(super) static READ_THREADS: std::cell::RefCell<std::collections::HashSet<std::thread::ThreadId>> = std::cell::RefCell::new(std::collections::HashSet::new());}

fn normalized(command: &RhiCommand) -> RhiCommand {
    let mut command = command.clone();
    if let RhiCommand::BeginView { view, .. } = &mut command {
        let view = Arc::make_mut(view);
        // Version is frame freshness, not a native dependency. The current
        // matrix is validated and uploaded; every other view field stays exact.
        view.version = 0;
        view.view_matrix = crate::render_contract::IDENTITY_MATRIX.map(Into::into);
    }
    command
}
fn eligible(draw: &DrawGeometry) -> bool {
    draw.standard_lit.is_none()
        && draw.environment.is_none()
        && draw.lighting.is_none()
        && draw.shadows.is_none()
        && !draw.view.is_shadow_map()
        && draw.view.color_pipeline.is_none()
}
// The group has four lanes, each with one initial job, and joins every lane.
// Small views keep their original inline execution and scratch economy.
fn read_partition_count(commands: usize) -> usize {
    if commands >= 1024 {
        4
    } else {
        1
    }
}
struct SelectionChunk {
    native: Vec<Option<Arc<native_template::Template>>>,
    sources: Vec<(RenderResourceHandle, Arc<GeometryBuffers>)>,
    pipelines: BTreeSet<Arc<str>>,
    textures: BTreeSet<RenderResourceHandle>,
    #[cfg(test)]
    thread: std::thread::ThreadId,
}
fn select_partition(
    commands: &[RhiCommand],
    requirements: &[Option<Arc<preparation::Requirements>>],
    meshes: &residency::RegistryLookup<'_, RenderResourceHandle, ResidentGeometry>,
    registry: native_template::Registry<'_>,
    lookup: native_template::Lookup<'_>,
    limits: &wgpu::Limits,
    total_commands: usize,
) -> Option<SelectionChunk> {
    let mut validation = crate::render_geometry::DrawViewValidation::default();
    let mut sources =
        residency::PlanSources::with_dense_capacity(total_commands, |h: &RenderResourceHandle| {
            h.index as usize
        });
    let mut current = native_template::Current::default();
    let mut native = Vec::with_capacity(commands.len());
    let mut pipelines = BTreeSet::new();
    let mut pipeline_bindings = preparation::Bindings::default();
    let mut textures = BTreeSet::new();
    let mut bindings = std::collections::HashSet::new();
    for (command, facts) in commands.iter().zip(requirements) {
        if let Some(payload) = command.geometry_payload() {
            let draw = payload.geometry_draw()?;
            draw.validate_shared_with_view(&mut validation).ok()?;
            let handle = draw.geometry_handle().raw();
            let resident = meshes.get(&handle);
            if !sources.include(handle, &draw.geometry, |_| resident.map(|m| &m.source))
                || draw.geometry.vertex_stride() > limits.max_vertex_buffer_array_stride
                || draw.geometry.vertex_byte_len() as u64 > limits.max_buffer_size
                || draw.geometry.index_byte_len() as u64 > limits.max_buffer_size
            {
                return None;
            }
            let facts = facts.as_ref()?;
            if pipeline_bindings.include_pipeline(facts, &mut pipelines)
                && !registry.has_pipeline(facts.pipeline())
            {
                return None;
            }
            if bindings.insert(Arc::as_ptr(&draw.material_binding)) {
                textures.extend(facts.textures().iter().flatten().map(|h| h.raw()));
            }
            native.push(lookup.current_template(registry, draw, facts, &mut current, resident));
        } else {
            native.push(None);
        }
    }
    Some(SelectionChunk {
        native,
        sources: sources.into_ordered_sources(),
        pipelines,
        textures,
        #[cfg(test)]
        thread: std::thread::current().id(),
    })
}
struct MatchRead {
    valid: bool,
    #[cfg(test)]
    thread: std::thread::ThreadId,
}
fn match_partition(
    commands: &[RhiCommand],
    templates: &[Option<Arc<native_template::Template>>],
    meshes: &residency::RegistryLookup<'_, RenderResourceHandle, ResidentGeometry>,
    registry: native_template::Registry<'_>,
) -> MatchRead {
    let mut validation = crate::render_geometry::DrawViewValidation::default();
    let mut current = native_template::Current::default();
    let valid = commands.iter().zip(templates).all(|(command, template)| {
        let Some(payload) = command.geometry_payload() else {
            return true;
        };
        let Some(draw) = payload.geometry_draw() else {
            return false;
        };
        draw.validate_shared_with_view(&mut validation).is_ok()
            && template.as_ref().is_some_and(|template| {
                current.check_in(
                    registry,
                    template,
                    draw,
                    meshes.get(&draw.geometry_handle().raw()),
                )
            })
    });
    MatchRead {
        valid,
        #[cfg(test)]
        thread: std::thread::current().id(),
    }
}
impl MovingViews {
    // A selection change may keep the same view/output contract. Interpret
    // today's draws once and admit today's resources; retain no draw or lease.
    pub(super) fn select(
        &self,
        backend: &RealWgpuBackend,
        plan: &RhiCommandPlan,
        cached: &ValidatedGeometryPlan,
        extent: [u32; 2],
        rect: Option<GameViewRect>,
    ) -> Option<Selection> {
        if plan.has_errors()
            || cached.extent != extent
            || cached.rect != rect
            || cached.output_format != backend.format
            || cached.schedule != plan.graph_schedule
        {
            return None;
        }
        let mut controls = self.0.iter().filter_map(|c| match c {
            Command::Control(c) => Some(c.as_ref()),
            _ => None,
        });
        let mut views = BTreeMap::new();
        let mut checked_begun = None;
        let mut requirements = Vec::with_capacity(plan.commands.len());
        let mut skeleton = Vec::with_capacity(plan.commands.len());
        // Interpret controls and owner-local requirement memos serially. Only
        // immutable native maps and completed facts enter the task group.
        for command in &plan.commands {
            if let RhiCommand::Draw {
                draw_kind: RhiDrawKind::Geometry,
                target,
                vertex_count,
                payload,
            } = command
            {
                let draw = payload.geometry_draw()?;
                let attachments = (
                    draw.target.as_ref(),
                    draw.view.as_ref(),
                    draw.depth_target.as_deref(),
                );
                if !eligible(draw)
                    || target.as_ref() != draw.target.as_ref()
                    || *vertex_count != draw.index_count
                    || !(checked_begun
                        .is_some_and(|old| matching_draw_attachments(old, attachments))
                        || views
                            .get(&draw.view.view_id)
                            .is_some_and(|old| matching_draw_attachments(*old, attachments)))
                {
                    return None;
                }
                checked_begun = Some(attachments);
                skeleton.push(Command::Draw {
                    view: draw.view.view_id,
                    instances: payload.geometry_instance_count(),
                    batch: matches!(payload, RhiDrawPayload::GeometryBatch { .. }),
                    vertices: *vertex_count,
                });
                requirements.push(Some(
                    backend
                        .geometry
                        .preparation
                        .resolve(draw, draw_color_format(draw, backend.format)),
                ));
            } else {
                let control = normalized(command);
                if controls.next()? != &control {
                    return None;
                }
                if let RhiCommand::BeginView {
                    target,
                    view,
                    depth_target,
                } = command
                {
                    geometry_viewport(view, extent[0], extent[1], rect).ok()?;
                    if views
                        .insert(
                            view.view_id,
                            (target.as_str(), view.as_ref(), depth_target.as_deref()),
                        )
                        .is_some()
                    {
                        return None;
                    }
                    checked_begun = None;
                }
                requirements.push(None);
                skeleton.push(Command::Control(Box::new(control)));
            }
        }
        if controls.next().is_some() {
            return None;
        }
        let meshes =
            residency::RegistryLookup::new(&backend.geometry.meshes, plan.commands.len(), |h| {
                h.index as usize
            });
        let registry = native_template::Registry::new(backend);
        let limits = backend.device.limits();
        let partitions = read_partition_count(plan.commands.len());
        let width = plan.commands.len().div_ceil(partitions);
        let chunks = backend.geometry.native_templates.with_lookup(|lookup| {
            crate::render_geometry::read_native_partitions(partitions, &|partition| {
                let start = partition * width;
                let end = (start + width).min(plan.commands.len());
                Ok(select_partition(
                    &plan.commands[start..end],
                    &requirements[start..end],
                    &meshes,
                    registry,
                    lookup,
                    &limits,
                    plan.commands.len(),
                ))
            })
            .ok()?
            .into_iter()
            .collect::<Option<Vec<_>>>()
        })?;
        let mut native = native_template::SelectionTemplates::with_capacity(plan.commands.len());
        let mut sources = Vec::new();
        let mut pipelines = BTreeSet::new();
        let mut textures = BTreeSet::new();
        let mut command_index = 0;
        for chunk in chunks {
            #[cfg(test)]
            READ_THREADS.with(|threads| {
                threads.borrow_mut().insert(chunk.thread);
            });
            sources.extend(chunk.sources);
            pipelines.extend(chunk.pipelines);
            textures.extend(chunk.textures);
            for template in chunk.native {
                if geometry_draw(&plan.commands[command_index]).is_some() {
                    native.push(template);
                } else {
                    native.control();
                }
                command_index += 1;
            }
        }
        // Independent partitions each checked resident sources. Missing
        // residents can still conflict across partitions; compare the complete
        // key before deduplicating the final ordered source closure.
        sources.sort_by_key(|(handle, _)| *handle);
        if sources
            .iter()
            .any(|(handle, _)| backend.geometry.streaming.pending(handle))
            || sources.windows(2).any(|pair| {
                pair[0].0 == pair[1].0
                    && !Arc::ptr_eq(&pair[0].1, &pair[1].1)
                    && pair[0].1 != pair[1].1
            })
            || !textures.iter().all(|h| {
                #[cfg(test)]
                super::super::CURRENT_TEXTURE_CLOSURE_QUERIES.with(|n| n.set(n.get() + 1));
                backend.textures.contains_key(h)
            })
        {
            return None;
        }
        sources.dedup_by(|next, previous| {
            if next.0 != previous.0 {
                return false;
            }
            std::mem::swap(&mut previous.1, &mut next.1);
            true
        });
        Some(Selection {
            moving_views: MovingViews(skeleton),
            extent,
            rect,
            requirements,
            native,
            pipelines,
            textures,
            sources,
        })
    }
    pub(super) fn capture(plan: &RhiCommandPlan) -> Option<Self> {
        plan.commands
            .iter()
            .map(|command| match command {
                RhiCommand::Draw {
                    draw_kind: RhiDrawKind::Geometry,
                    vertex_count,
                    payload,
                    ..
                } => {
                    let draw = payload.geometry_draw()?;
                    #[cfg(test)]
                    CAPTURE_DRAW_VISITS.with(|n| n.set(n.get() + 1));
                    eligible(draw).then(|| Command::Draw {
                        view: draw.view.view_id,
                        instances: payload.geometry_instance_count(),
                        batch: matches!(payload, RhiDrawPayload::GeometryBatch { .. }),
                        vertices: *vertex_count,
                    })
                }
                RhiCommand::BeginFrame { .. }
                | RhiCommand::BeginView { .. }
                | RhiCommand::Clear { .. }
                | RhiCommand::Submit
                | RhiCommand::Present { .. } => {
                    Some(Command::Control(Box::new(normalized(command))))
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .map(Self)
    }
    pub(super) fn matches(
        &self,
        backend: &RealWgpuBackend,
        plan: &RhiCommandPlan,
        cached: &ValidatedGeometryPlan,
        templates: &[Option<Arc<native_template::Template>>],
        extent: [u32; 2],
        rect: Option<GameViewRect>,
    ) -> bool {
        if plan.has_errors()
            || self.0.len() != plan.commands.len()
            || templates.len() != self.0.len()
            || cached.extent != extent
            || cached.rect != rect
            || cached.output_format != backend.format
            || cached.schedule != plan.graph_schedule
        {
            return false;
        }
        // A late visibility change must not first validate the stable prefix,
        // then validate it again in selection preparation. This bounded scan
        // reads typed handles only; full source/material/range checks follow.
        if templates
            .iter()
            .zip(&plan.commands)
            .any(|(native, command)| {
                geometry_draw(command).is_some_and(|draw| {
                    native
                        .as_ref()
                        .is_none_or(|native| native.mesh != draw.geometry_handle().raw())
                })
            })
        {
            return false;
        }
        let mut views = BTreeMap::new();
        let mut checked_begun: Option<DrawAttachments<'_>> = None;

        for (index, (old, command)) in self.0.iter().zip(&plan.commands).enumerate() {
            match (old, command) {
                (
                    Command::Control(old),
                    RhiCommand::BeginView {
                        target,
                        view,
                        depth_target,
                    },
                ) => {
                    checked_begun = None;
                    // This validates the logical view and its mapping into the
                    // physical surface, which may have a different extent.
                    if old.as_ref() != &normalized(command)
                        || geometry_viewport(view, extent[0], extent[1], rect).is_err()
                    {
                        return false;
                    }
                    views.insert(
                        view.view_id,
                        (target.as_str(), view.as_ref(), depth_target.as_deref()),
                    );
                }
                (Command::Control(old), next) if old.as_ref() == next => {}
                (
                    Command::Draw {
                        view,
                        instances,
                        batch,
                        vertices,
                    },
                    RhiCommand::Draw {
                        target: next_target,
                        draw_kind: RhiDrawKind::Geometry,
                        vertex_count,
                        payload,
                    },
                ) => {
                    let Some(draw) = payload.geometry_draw() else {
                        return false;
                    };
                    let attachments = (
                        draw.target.as_ref(),
                        draw.view.as_ref(),
                        draw.depth_target.as_deref(),
                    );
                    let begun = checked_begun
                        .is_some_and(|old| matching_draw_attachments(old, attachments))
                        || views
                            .get(view)
                            .is_some_and(|old| matching_draw_attachments(*old, attachments));
                    if !eligible(draw)
                        || next_target.as_ref() != draw.target.as_ref()
                        || *view != draw.view.view_id
                        || vertices != vertex_count
                        || *vertices != draw.index_count
                        || *instances != payload.geometry_instance_count()
                        || *batch != matches!(payload, RhiDrawPayload::GeometryBatch { .. })
                        || !begun
                    {
                        return false;
                    }
                    checked_begun = Some(attachments);
                    let Some(facts) = cached.requirements[index].as_ref() else {
                        return false;
                    };
                    let today = backend
                        .geometry
                        .preparation
                        .resolve(draw, draw_color_format(draw, backend.format));
                    if !Arc::ptr_eq(facts, &today) {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        let meshes =
            residency::RegistryLookup::new(&backend.geometry.meshes, plan.commands.len(), |h| {
                h.index as usize
            });
        let registry = native_template::Registry::new(backend);
        let partitions = read_partition_count(plan.commands.len());
        let width = plan.commands.len().div_ceil(partitions);
        let Ok(reads) = crate::render_geometry::read_native_partitions(partitions, &|partition| {
            let start = partition * width;
            let end = (start + width).min(plan.commands.len());
            Ok(match_partition(
                &plan.commands[start..end],
                &templates[start..end],
                &meshes,
                registry,
            ))
        }) else {
            return false;
        };
        for read in reads {
            #[cfg(test)]
            READ_THREADS.with(|threads| {
                threads.borrow_mut().insert(read.thread);
            });
            if !read.valid {
                return false;
            }
        }
        cached
            .meshes
            .iter()
            .all(|(h, _)| !backend.geometry.streaming.pending(h))
            && residency::sources_match(
                &cached.meshes,
                backend.geometry.meshes.len(),
                backend.geometry.meshes.iter().map(|(h, m)| (h, &m.source)),
                |h| backend.geometry.meshes.get(h).map(|m| &m.source),
            )
            && cached.textures.iter().all(|h| {
                #[cfg(test)]
                super::super::CURRENT_TEXTURE_CLOSURE_QUERIES.with(|n| n.set(n.get() + 1));
                backend.textures.contains_key(h)
            })
    }
    pub(super) fn observe(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.cpu_vec(&self.0, "renderer", &["resident"]);
        for command in &self.0 {
            let Command::Control(command) = command else {
                continue;
            };
            sample.cpu_storage(
                command.as_ref() as *const RhiCommand as usize,
                std::mem::size_of_val(command.as_ref()),
                "renderer",
                &["resident"],
            );
            // capture retains only these controls. Draw rows are scalar;
            // shared view data keeps its existing snapshot owner for dedup.
            match command.as_ref() {
                RhiCommand::BeginFrame { target }
                | RhiCommand::Clear { target, .. }
                | RhiCommand::Present { target } => {
                    sample.cpu_string(target, "renderer", &["resident"]);
                }
                RhiCommand::BeginView {
                    target,
                    view,
                    depth_target,
                } => {
                    sample.cpu_string(target, "renderer", &["resident"]);
                    if let Some(target) = depth_target {
                        sample.cpu_string(target, "renderer", &["resident"]);
                    }
                    sample.cpu_arc(view, "snapshots", &["resident"]);
                    for text in [
                        &view.source_camera_entity_id,
                        &view.provenance.source_entity_id,
                        &view.provenance.source_component,
                        &view.provenance.source_path,
                        &view.provenance.project_revision,
                        &view.provenance.operation_id,
                        &view.provenance.asset_id,
                        &view.provenance.content_digest,
                    ]
                    .into_iter()
                    .flatten()
                    {
                        sample.cpu_string(text, "snapshots", &["resident"]);
                    }
                    if let Some(pipeline) = &view.color_pipeline {
                        pipeline.observe_memory(sample, "snapshots", "resident");
                    }
                }
                _ => {}
            }
        }
        sample.partial(
            "cpu.owner.renderer",
            "movingViewControlBoxes+targetStrings+sharedViewBacking;allocatorArcControlBlocksOpaque",
        );
    }
}

// Call-local result of actual validation. Current native templates remain
// immutable through the only mutations below: absent mesh admission and removal
// of resources outside this exact selection. This is not a cross-frame proof.
pub(super) struct Selection {
    moving_views: MovingViews,
    extent: [u32; 2],
    rect: Option<GameViewRect>,
    requirements: Vec<Option<Arc<preparation::Requirements>>>,
    native: native_template::SelectionTemplates,
    pipelines: BTreeSet<Arc<str>>,
    textures: BTreeSet<RenderResourceHandle>,
    sources: Vec<(RenderResourceHandle, Arc<GeometryBuffers>)>,
}
impl Selection {
    #[cfg(test)]
    pub(super) fn sources_len_for_test(&self) -> usize {
        self.sources.len()
    }
    pub(super) fn install(
        self,
        backend: &mut RealWgpuBackend,
        plan: &RhiCommandPlan,
        staged: Option<parameters::Update>,
        previous_current: &mut template_backing::TemplateCollection<Arc<native_template::Template>>,
        previous_native: &mut Option<
            template_backing::TemplateCollection<Option<Arc<native_template::Template>>>,
        >,
        (outputs_prepared, capabilities_validated): (bool, bool),
    ) -> Result<(), String> {
        let selected =
            residency::RegistryLookup::new_ordered(&self.sources, plan.commands.len(), |h| {
                h.index as usize
            });
        // All exact admitted templates already passed today's current checks.
        // Only detached but equal recordings need scalar native lookups below.
        let mut current = native_template::Current::default();
        let discard: Vec<_> = backend
            .geometry
            .validated_plan
            .as_ref()
            .unwrap()
            .bundles
            .iter()
            .filter(|(_, b)| {
                !b.recipe.survives_selection(
                    backend,
                    &self.native,
                    &selected,
                    &self.pipelines,
                    &mut current,
                )
            })
            .map(|(key, _)| *key)
            .collect();
        // Both closures are ordered by the complete handle. One merge finds
        // uploads and actual retirements without building another native table.
        let mut retired_meshes = Vec::new();
        let mut missing = Vec::new();
        let mut resident = backend.geometry.meshes.keys().peekable();
        for (handle, source) in &self.sources {
            while resident.peek().is_some_and(|old| *old < handle) {
                let old = resident.next().unwrap();
                if !backend.geometry.streaming.pinned(old) {
                    retired_meshes.push(*old);
                }
            }
            if resident.peek().is_some_and(|old| *old == handle) {
                resident.next();
            } else {
                missing.push((*handle, source.clone()));
            }
        }
        retired_meshes.extend(
            resident
                .filter(|h| !backend.geometry.streaming.pinned(h))
                .copied(),
        );
        let retired_pipelines: Vec<_> = backend
            .geometry
            .pipelines
            .keys()
            .filter(|key| !self.pipelines.contains(key.as_str()))
            .cloned()
            .collect();
        backend.admit_gpu_retirement(
            0,
            discard.len() + usize::from(!retired_meshes.is_empty()) + retired_pipelines.len(),
        )?;
        for key in discard {
            let old = backend
                .geometry
                .validated_plan
                .as_mut()
                .unwrap()
                .bundles
                .remove(&key)
                .unwrap();
            backend.retire_gpu(old, 0);
        }
        let reserved = backend.reserve_geometry_meshes(&missing)?;
        let retired: Vec<_> = retired_meshes
            .into_iter()
            .map(|h| backend.geometry.meshes.remove(&h).unwrap())
            .collect();
        if !retired.is_empty() {
            backend.retire_gpu(retired, 0);
        }
        backend.upload_geometry_meshes(plan.frame_index, reserved)?;
        for key in retired_pipelines {
            let old = backend.geometry.pipelines.remove(&key).unwrap();
            backend.retire_gpu(old, 0);
        }
        backend.install_geometry_parameters(plan, staged)?;
        let (native, current) = self.native.complete(
            &backend.geometry.native_templates,
            backend,
            plan,
            &self.requirements,
        );
        backend.geometry.current_templates =
            template_backing::TemplateCollection::reuse(current, previous_current);
        // Selection already checked, sorted and deduplicated the complete
        // source closure. Publish that allocation directly; the previous plan
        // releases its exact sources at the original successful replacement.
        #[cfg(test)]
        residency::SOURCE_PUBLICATION_ALLOCATION.with(|p| p.set(self.sources.as_ptr() as usize));
        let mut next = ValidatedGeometryPlan::capture_prepared(
            plan,
            (self.extent, self.rect),
            backend.format,
            self.sources,
            self.textures,
            self.requirements,
            Some(self.moving_views),
        );
        if let Some(next) = &mut next {
            next.native_templates = template_backing::TemplateCollection::reuse(
                native,
                previous_native.as_mut().unwrap(),
            );
            next.outputs_prepared = outputs_prepared;
            next.capabilities_validated.set(capabilities_validated);
        }
        backend.finish_geometry_preparation(plan, next)
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    use crate::resource_memory::{memory_totals, ResourceMemorySample};

    #[test]
    fn moving_view_control_capacity_is_observed_without_retaining_aliases() {
        for capacity in [256, 4096] {
            for populated in [false, true] {
                let mut expected = Vec::new();
                let mut text = |value: &str, owner: &'static str| {
                    let mut value_with_capacity = String::with_capacity(capacity);
                    if populated {
                        value_with_capacity.push_str(value);
                    }
                    expected.push((
                        value_with_capacity.as_ptr() as usize,
                        value_with_capacity.capacity(),
                        owner,
                    ));
                    value_with_capacity
                };
                let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
                    &crate::rhi_command_plan::tests::retained_geometry_graph(1),
                );
                let mut view = plan
                    .commands
                    .iter()
                    .find_map(|command| match command {
                        RhiCommand::BeginView { view, .. } => Some(view.as_ref().clone()),
                        _ => None,
                    })
                    .unwrap();
                let frame_target = text("frame", "renderer");
                let clear_target = text("clear", "renderer");
                let view_target = text("view", "renderer");
                let depth_target = text("depth", "renderer");
                let alias_target = text("alias", "renderer");
                let present_target = text("present", "renderer");
                view.source_camera_entity_id = Some(text("camera", "snapshots"));
                for field in [
                    &mut view.provenance.source_entity_id,
                    &mut view.provenance.source_component,
                    &mut view.provenance.source_path,
                    &mut view.provenance.project_revision,
                    &mut view.provenance.operation_id,
                    &mut view.provenance.asset_id,
                    &mut view.provenance.content_digest,
                ] {
                    *field = Some(text("provenance", "snapshots"));
                }
                view.color_pipeline = Some(crate::color_pipeline::ColorPipeline3D {
                    schema_version: text("color", "snapshots"),
                    ..Default::default()
                });
                let view = Arc::new(view);
                expected.push((
                    Arc::as_ptr(&view) as usize,
                    std::mem::size_of_val(view.as_ref()),
                    "snapshots",
                ));
                let mut moving = MovingViews(vec![
                    Command::Control(Box::new(RhiCommand::BeginFrame {
                        target: frame_target,
                    })),
                    Command::Control(Box::new(RhiCommand::Clear {
                        target: clear_target,
                        color: [0.0.into(); 4],
                    })),
                    Command::Control(Box::new(RhiCommand::BeginView {
                        target: view_target,
                        view: view.clone(),
                        depth_target: Some(depth_target),
                    })),
                    Command::Control(Box::new(RhiCommand::BeginView {
                        target: alias_target,
                        view: view.clone(),
                        depth_target: None,
                    })),
                    Command::Control(Box::new(RhiCommand::Submit)),
                    Command::Control(Box::new(RhiCommand::Present {
                        target: present_target,
                    })),
                ]);
                expected.push((
                    moving.0.as_ptr() as usize,
                    moving.0.capacity() * std::mem::size_of::<Command>(),
                    "renderer",
                ));
                for command in &moving.0 {
                    if let Command::Control(command) = command {
                        expected.push((
                            command.as_ref() as *const RhiCommand as usize,
                            std::mem::size_of::<RhiCommand>(),
                            "renderer",
                        ));
                    }
                }
                let aliases = Arc::strong_count(&view);
                let mut sample = ResourceMemorySample::new(1, 1, 0);
                moving.observe(&mut sample);
                for (address, bytes, owner) in &expected {
                    let id = format!("cpu-storage:{address:x}");
                    let row = sample
                        .allocations
                        .iter()
                        .find(|row| row.allocation_id == id)
                        .unwrap_or_else(|| panic!("missing control backing {id} ({bytes} bytes)"));
                    assert_eq!(row.bytes, *bytes as u64);
                    assert_eq!(row.owner, *owner);
                    assert_eq!(row.roles, ["resident".to_string()].into());
                }
                let expected_bytes: usize = expected.iter().map(|row| row.1).sum();
                let totals = memory_totals(&sample.allocations).unwrap();
                assert_eq!(totals.cpu_allocated, expected_bytes as u64);
                moving.observe(&mut sample);
                assert_eq!(
                    memory_totals(&sample.allocations).unwrap().cpu_allocated,
                    totals.cpu_allocated,
                    "shared view and repeated observations count physical storage once"
                );
                assert_eq!(Arc::strong_count(&view), aliases);
                moving.0 = Vec::new();
                assert_eq!(Arc::strong_count(&view), 1);
                let mut after = ResourceMemorySample::new(2, 1, 0);
                moving.observe(&mut after);
                assert!(after.allocations.is_empty());
            }
        }
    }
}
