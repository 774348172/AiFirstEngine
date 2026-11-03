//! One native recording recipe. Dynamic parameter contents are not encoder commands.
use super::super::memory::{GpuMemoryCensus, RetiredGpuResource};
use super::*;
use crate::resource_memory::ResourceMemorySample;
use std::ops::Range;
#[path = "encoding_backing.rs"]
mod backing;

#[derive(Clone, PartialEq, Eq)]
struct Batch {
    native: Arc<native_template::Template>,
    lights: Option<wgpu::BindGroup>,
    shadows: Option<wgpu::BindGroup>,
    instance_range: Range<u32>,
}
impl std::ops::Deref for Batch {
    type Target = native_template::Template;
    fn deref(&self) -> &Self::Target {
        &self.native
    }
}

#[derive(Clone)]
pub(super) struct Recipe {
    color_format: Option<wgpu::TextureFormat>,
    depth: bool,
    parameter_binding: wgpu::BindGroup,
    parameter_buffer: wgpu::Buffer,
    view_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    batches: Vec<Batch>,
    extra_buffers: Vec<wgpu::Buffer>,
    textures: Vec<wgpu::Texture>,
    backing: backing::RecipeBacking,
}

impl PartialEq for Recipe {
    fn eq(&self, other: &Self) -> bool {
        self.color_format == other.color_format
            && self.depth == other.depth
            && self.parameter_binding == other.parameter_binding
            && self.parameter_buffer == other.parameter_buffer
            && self.view_buffer == other.view_buffer
            && self.index_buffer == other.index_buffer
            && self.batches == other.batches
            && self.same_dependencies(other)
    }
}
impl Eq for Recipe {}
impl Recipe {
    pub(super) fn indirect_seeds(&self) -> Vec<[u32; 5]> {
        self.batches
            .iter()
            .map(|batch| {
                [
                    batch.index_range.end - batch.index_range.start,
                    batch.instance_range.end - batch.instance_range.start,
                    batch.index_range.start,
                    batch.base_vertex as u32,
                    batch.instance_range.start,
                ]
            })
            .collect()
    }
    fn bind_one<'a>(&'a self, pass: &mut impl wgpu::util::RenderEncoder<'a>, batch: &'a Batch) {
        pass.set_bind_group(0, Some(&self.parameter_binding), &[]);
        pass.set_pipeline(&batch.pipeline);
        if let Some(material) = &batch.material {
            pass.set_bind_group(1, Some(material), &[]);
        }
        if let Some(lights) = &batch.lights {
            pass.set_bind_group(2, Some(lights), &[]);
        }
        if let Some(shadows) = &batch.shadows {
            pass.set_bind_group(3, Some(shadows), &[]);
        }
        pass.set_vertex_buffer(0, batch.vertices.slice(..));
        pass.set_vertex_buffer(1, self.index_buffer.slice(..));
        pass.set_index_buffer(batch.indices.slice(..), batch.index_format);
    }
    pub(super) fn record_queries<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        _queries: &wgpu::QuerySet,
    ) -> u64 {
        for (index, batch) in self.batches.iter().enumerate() {
            self.bind_one(pass, batch);
            pass.begin_occlusion_query(index as u32);
            pass.draw_indexed(
                batch.index_range.clone(),
                batch.base_vertex,
                batch.instance_range.clone(),
            );
            pass.end_occlusion_query();
        }
        self.batches.len() as u64
    }
    pub(super) fn record_indirect<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        arguments: &'a wgpu::Buffer,
    ) -> u64 {
        for (index, batch) in self.batches.iter().enumerate() {
            self.bind_one(pass, batch);
            pass.draw_indexed_indirect(arguments, index as u64 * 20);
        }
        self.batches.len() as u64
    }
    fn same_dependencies(&self, other: &Self) -> bool {
        if self.extra_buffers == other.extra_buffers
            && self.textures == other.textures
            && self.batches.iter().zip(&other.batches).all(|(a, b)| {
                Arc::ptr_eq(&a.native, &b.native)
                    || (a.native.textures == b.native.textures
                        && a.native.environment == b.native.environment)
            })
        {
            return true;
        }
        // The scalar test oracle stores stable dependencies in the old unique
        // closure. Production recipes store them in shared templates. Compare
        // the actual complete dependency set when organization differs.
        fn buffers(recipe: &Recipe) -> Vec<&wgpu::Buffer> {
            let mut buffers: Vec<_> = recipe
                .extra_buffers
                .iter()
                .chain(
                    recipe
                        .batches
                        .iter()
                        .filter_map(|b| b.native.environment_buffer()),
                )
                .collect();
            buffers.sort_unstable();
            buffers.dedup();
            buffers
        }
        fn textures(recipe: &Recipe) -> Vec<&wgpu::Texture> {
            let mut textures: Vec<_> = recipe
                .textures
                .iter()
                .chain(
                    recipe
                        .batches
                        .iter()
                        .flat_map(|b| b.native.texture_objects()),
                )
                .collect();
            textures.sort_unstable();
            textures.dedup();
            textures
        }
        buffers(self) == buffers(other) && textures(self) == textures(other)
    }
}

impl Recipe {
    // Compare the already admitted singleton stream directly with this recipe.
    // This borrows the current successful plan, never remembers residency, and
    // avoids allocating/cloning a second equivalent recipe for unchanged ranges.
    // Grouped batches, view bindings and detached templates keep full capture.
    pub(super) fn matches_prepared_commands(
        &self,
        backend: &RealWgpuBackend,
        commands: &[RhiCommand],
        first_instance: u32,
    ) -> bool {
        if self.batches.len() != commands.len()
            || !self.extra_buffers.is_empty()
            || !self.textures.is_empty()
        {
            return false;
        }
        let Some(first) = commands.first().and_then(geometry_draw) else {
            return false;
        };
        let Some(arena) = &backend.geometry.parameters else {
            return false;
        };
        let Some(native) = backend
            .geometry
            .validated_plan
            .as_ref()
            .and_then(|p| p.native_for(commands, backend.format))
        else {
            return false;
        };
        self.color_format
            == (!first.view.is_shadow_map()).then(|| draw_color_format(first, backend.format))
            && self.depth == first.depth_target.is_some()
            && self.parameter_binding == arena.binding
            && self.parameter_buffer == arena.buffer
            && self.view_buffer == arena.view_buffer
            && self.index_buffer == arena.index_buffer
            && self
                .batches
                .iter()
                .zip(commands)
                .zip(native)
                .enumerate()
                .all(|(index, ((batch, command), template))| {
                    let Some(draw) = geometry_draw(command) else {
                        return false;
                    };
                    let ordinal = first_instance + index as u32;
                    let slot = arena.selection.as_ref().map_or(Some(ordinal), |selection| {
                        selection.get(ordinal as usize).copied()
                    });
                    command.geometry_instance_count() == 1
                        && draw.lighting.is_none()
                        && draw.shadows.is_none()
                        && batch.lights.is_none()
                        && batch.shadows.is_none()
                        && template
                            .as_ref()
                            .is_some_and(|template| Arc::ptr_eq(template, &batch.native))
                        && slot.is_some_and(|slot| batch.instance_range == (slot..slot + 1))
                })
    }
    pub(super) fn capture_commands(
        backend: &RealWgpuBackend,
        commands: &[RhiCommand],
        first_instance: u32,
    ) -> Result<Self, String> {
        #[cfg(test)]
        RECIPE_CAPTURE_ROWS.with(|n| n.set(n.get() + commands.len()));
        if commands.is_empty() || commands.iter().any(|c| geometry_draw(c).is_none()) {
            return Err("geometry.invalid_draw_range".into());
        }
        let prepared = backend
            .geometry
            .validated_plan
            .as_ref()
            .and_then(|p| p.requirements_for(commands, backend.format));
        let native = backend
            .geometry
            .validated_plan
            .as_ref()
            .and_then(|p| p.native_for(commands, backend.format));
        Ok(Self::capture_stream(
            backend,
            commands.iter().enumerate().map(|(index, c)| {
                (
                    geometry_draw(c).expect("checked geometry range"),
                    c.geometry_instance_count() as u32,
                    prepared.and_then(|facts| facts[index].as_ref()),
                    native.and_then(|templates| templates[index].as_ref()),
                )
            }),
            first_instance,
        ))
    }
    #[cfg(test)]
    pub(super) fn capture(
        backend: &RealWgpuBackend,
        draws: &[&DrawGeometry],
        first_instance: u32,
    ) -> Self {
        Self::capture_counts(backend, draws, &vec![1; draws.len()], first_instance)
    }
    #[cfg(test)]
    pub(super) fn capture_counts(
        backend: &RealWgpuBackend,
        draws: &[&DrawGeometry],
        counts: &[u32],
        first_instance: u32,
    ) -> Self {
        #[cfg(test)]
        RECIPE_DESCRIPTOR_TEMP_ROWS.with(|n| n.set(n.get() + counts.len() + 1));
        let mut offsets = Vec::with_capacity(counts.len() + 1);
        offsets.push(first_instance);
        for count in counts {
            offsets.push(offsets.last().copied().unwrap() + count);
        }
        let first = draws[0];
        let arena = backend.geometry.parameters.as_ref().unwrap();
        let mut recipe = Self {
            color_format: (!first.view.is_shadow_map())
                .then(|| draw_color_format(first, backend.format)),
            depth: first.depth_target.is_some(),
            parameter_binding: arena.binding.clone(),
            parameter_buffer: arena.buffer.clone(),
            view_buffer: arena.view_buffer.clone(),
            index_buffer: arena.index_buffer.clone(),
            batches: Vec::new(),
            extra_buffers: Vec::new(),
            textures: Vec::new(),
            backing: Default::default(),
        };
        // Borrow today's native objects for this capture only. Build the
        // original ordered unique dependency closure before cloning its handles,
        // rather than cloning every reference and discarding most after sorting.
        let mut extra_buffers = Vec::new();
        let mut textures = Vec::new();
        let mut start = 0;
        while start < draws.len() {
            let draw = draws[start];
            let requirements = backend
                .geometry
                .preparation
                .resolve(draw, draw_color_format(draw, backend.format));
            let key = requirements.pipeline().clone();
            let mut end = start + 1;
            while end < draws.len()
                && batch_state_compatible(draw, draws[end])
                && backend
                    .geometry
                    .preparation
                    .resolve(draws[end], draw_color_format(draws[end], backend.format))
                    .pipeline
                    == key
            {
                end += 1;
            }
            #[cfg(test)]
            NATIVE_OBJECT_RESOLUTIONS.with(|n| n.set(n.get() + 1));
            let handle = draw.geometry_handle().raw();
            let mesh = &backend.geometry.meshes[&handle];
            let mut material = None;
            let mut lights = None;
            let mut shadows = None;
            // The position-only opaque writer has no fragment bindings. Masked
            // writers and color passes keep their actual sampling semantics.
            if !draw.is_opaque_shadow_depth_writer() {
                if draw.shadows.is_some() {
                    let binding = &backend.geometry.shadow_bindings[&draw.view.view_id];
                    shadows = Some(binding.binding.clone());
                    extra_buffers.push(&binding.buffer);
                    textures.extend(binding.depths.iter().map(|v| v.texture()));
                }
                if let Some(light) = &draw.lighting {
                    let binding = &backend.geometry.light_buffers[light.identity()];
                    lights = Some(binding.binding.clone());
                    extra_buffers.push(&binding.buffer);
                }
                if draw.standard_lit.is_some() {
                    let key = lit::texture_key(draw);
                    material = Some(backend.geometry.lit_bindings[&key].clone());
                    for handle in key.0.into_iter().flatten() {
                        textures.push(backend.textures[&handle]._view.texture());
                    }
                    if let Some(environment) = &key.1 {
                        extra_buffers
                            .push(&backend.geometry.environment_buffers[environment].buffer);
                    }
                } else if let Some(handle) = requirements.textures()[0] {
                    let texture = &backend.textures[&handle.raw()];
                    material = Some(texture.bind_group.clone());
                    textures.push(texture._view.texture());
                }
            }
            let index_format = match draw.geometry.index_format() {
                GeometryIndexFormat::Uint16 => wgpu::IndexFormat::Uint16,
                GeometryIndexFormat::Uint32 => wgpu::IndexFormat::Uint32,
            };
            let first = draw.first_index
                + (mesh.indices.start
                    / match index_format {
                        wgpu::IndexFormat::Uint16 => 2,
                        wgpu::IndexFormat::Uint32 => 4,
                    }) as u32;
            recipe.batches.push(Batch {
                native: Arc::new(native_template::Template {
                    intent: Default::default(),
                    skin: backend.geometry.skins.resolve(draw),
                    mesh: handle,
                    pipeline: backend.geometry.pipelines[key.as_ref()].clone(),
                    pipeline_key: key,
                    vertices: backend
                        .geometry
                        .skins
                        .resolve(draw)
                        .map_or_else(|| mesh.vertices.buffer().clone(), |s| s.current.clone()),
                    indices: mesh.indices.buffer().clone(),
                    index_format,
                    index_range: first..first + draw.index_count,
                    base_vertex: if draw.deformation.is_some() {
                        0
                    } else {
                        (mesh.vertices.start / u64::from(draw.geometry.vertex_stride())) as i32
                    },
                    dependencies: Arc::new(native_template::MaterialDependencies {
                        material,
                        material_key: None,
                        textures: Vec::new(),
                        environment: None,
                    }),
                }),
                lights,
                shadows,
                instance_range: offsets[start]..offsets[end],
            });
            start = end;
        }
        extra_buffers.sort_unstable();
        extra_buffers.dedup();
        textures.sort_unstable();
        textures.dedup();
        recipe.extra_buffers = extra_buffers.into_iter().cloned().collect();
        recipe.textures = textures
            .into_iter()
            .inspect(|_| {
                #[cfg(test)]
                RECIPE_TEXTURE_CLONES.with(|n| n.set(n.get() + 1));
            })
            .cloned()
            .collect();
        recipe
    }

    fn capture_stream<'a>(
        backend: &RealWgpuBackend,
        draws: impl Iterator<
            Item = (
                &'a DrawGeometry,
                u32,
                Option<&'a Arc<preparation::Requirements>>,
                Option<&'a Arc<native_template::Template>>,
            ),
        >,
        first_instance: u32,
    ) -> Self {
        let mut draws = draws.peekable();
        let first = draws.peek().expect("nonempty geometry range").0;
        let arena = backend.geometry.parameters.as_ref().unwrap();
        let mut recipe = Self {
            color_format: (!first.view.is_shadow_map())
                .then(|| draw_color_format(first, backend.format)),
            depth: first.depth_target.is_some(),
            parameter_binding: arena.binding.clone(),
            parameter_buffer: arena.buffer.clone(),
            view_buffer: arena.view_buffer.clone(),
            index_buffer: arena.index_buffer.clone(),
            batches: Vec::new(),
            extra_buffers: Vec::new(),
            textures: Vec::new(),
            backing: Default::default(),
        };
        // Borrow today's native objects for this capture only. Build the
        // original ordered unique dependency closure before cloning its handles,
        // rather than cloning every reference and discarding most after sorting.
        let mut extra_buffers = Vec::new();
        let mut textures = Vec::new();
        // Mixed UI/particle plans do not get a geometry-only plan proof. Their
        // geometry ranges still consume the same native template owner after
        // current admission. Retain facts through this borrowed range so memo
        // addresses cannot recycle even when the bounded persistent memo is full.
        let mut resolved = std::collections::HashMap::new();
        let mut resolve = |draw: &DrawGeometry, facts: &Arc<preparation::Requirements>| {
            let key = native_template::Templates::key(draw, facts);
            let (_, native) = resolved.entry(key).or_insert_with(|| {
                #[cfg(test)]
                NATIVE_OBJECT_RESOLUTIONS.with(|n| n.set(n.get() + 1));
                (
                    facts.clone(),
                    backend
                        .geometry
                        .native_templates
                        .resolve(backend, draw, facts),
                )
            });
            Arc::clone(native)
        };
        let mut last_light: Option<(&str, &lights::LightBuffer)> = None;
        let mut last_shadow: Option<(u64, &shadows::ShadowBinding)> = None;
        let mut logical_offset = first_instance;
        let native_offset = |offset: u32| {
            arena
                .selection
                .as_ref()
                .and_then(|selection| selection.get(offset as usize))
                .copied()
                .unwrap_or(offset)
        };
        while let Some((draw, count, prepared, prepared_native)) = draws.next() {
            let owned;
            let requirements = if let Some(prepared) = prepared {
                prepared
            } else {
                owned = backend
                    .geometry
                    .preparation
                    .resolve(draw, draw_color_format(draw, backend.format));
                &owned
            };
            let native = prepared_native
                .cloned()
                .unwrap_or_else(|| resolve(draw, requirements));
            let offset = native_offset(logical_offset);
            let mut end = offset + count;
            logical_offset += count;
            while let Some((next, count, next_prepared, next_native)) = draws.peek() {
                if native_offset(logical_offset) != end {
                    break;
                }
                let owned;
                let next_requirements = if let Some(prepared) = next_prepared {
                    *prepared
                } else {
                    owned = backend
                        .geometry
                        .preparation
                        .resolve(next, draw_color_format(next, backend.format));
                    &owned
                };
                let owned_native;
                let next_native = if let Some(prepared) = next_native {
                    *prepared
                } else {
                    owned_native = resolve(next, next_requirements);
                    &owned_native
                };
                if Arc::ptr_eq(&native, next_native) {
                    if draw.view != next.view
                        || draw.lighting != next.lighting
                        || draw.environment != next.environment
                        || draw.target != next.target
                        || draw.depth_target != next.depth_target
                    {
                        break;
                    }
                } else {
                    #[cfg(test)]
                    STABLE_DESCRIPTOR_CHECKS.with(|n| n.set(n.get() + 1));
                    if !batch_state_compatible(draw, next)
                        || next_requirements.pipeline() != requirements.pipeline()
                    {
                        break;
                    }
                }
                end += *count;
                logical_offset += *count;
                draws.next();
            }
            let mut lights = None;
            let mut shadows = None;
            if !draw.is_opaque_shadow_depth_writer() {
                if draw.shadows.is_some() {
                    let id = draw.view.view_id;
                    let binding =
                        if let Some((_, binding)) = last_shadow.filter(|(key, _)| *key == id) {
                            binding
                        } else {
                            #[cfg(test)]
                            CAPTURE_BINDING_QUERIES.with(|n| {
                                let mut v = n.get();
                                v[1] += 1;
                                n.set(v);
                            });
                            let binding = &backend.geometry.shadow_bindings[&id];
                            extra_buffers.push(&binding.buffer);
                            textures.extend(binding.depths.iter().map(|v| v.texture()));
                            last_shadow = Some((id, binding));
                            binding
                        };
                    shadows = Some(binding.binding.clone());
                }
                if let Some(light) = &draw.lighting {
                    let id = light.identity();
                    let binding =
                        if let Some((_, binding)) = last_light.filter(|(key, _)| *key == id) {
                            binding
                        } else {
                            #[cfg(test)]
                            CAPTURE_BINDING_QUERIES.with(|n| {
                                let mut v = n.get();
                                v[0] += 1;
                                n.set(v);
                            });
                            let binding = &backend.geometry.light_buffers[id];
                            extra_buffers.push(&binding.buffer);
                            last_light = Some((id, binding));
                            binding
                        };
                    lights = Some(binding.binding.clone());
                }
            }
            // Stable dependencies are owned by the shared native template;
            // dynamic view bindings are recorded separately above.
            recipe.batches.push(Batch {
                native,
                lights,
                shadows,
                instance_range: offset..end,
            });
        }
        extra_buffers.sort_unstable();
        extra_buffers.dedup();
        textures.sort_unstable();
        textures.dedup();
        recipe.extra_buffers = extra_buffers.into_iter().cloned().collect();
        recipe.textures = textures
            .into_iter()
            .inspect(|_| {
                #[cfg(test)]
                RECIPE_TEXTURE_CLONES.with(|n| n.set(n.get() + 1));
            })
            .cloned()
            .collect();
        recipe
    }

    /// This only establishes that preparation will keep the referenced objects
    /// live. It never admits reuse: the complete post-prepare recipe must match.
    #[cfg(test)]
    pub(super) fn survives_prepare(
        &self,
        backend: &RealWgpuBackend,
        meshes: &BTreeMap<RenderResourceHandle, Arc<GeometryBuffers>>,
        pipelines: &BTreeSet<Arc<str>>,
    ) -> bool {
        let selected =
            residency::RegistryLookup::new(meshes, self.batches.len(), |h| h.index as usize);
        let mut current = native_template::Current::for_plan(backend, self.batches.len());
        self.survives_prepare_with(backend, &selected, pipelines, &mut current)
    }

    pub(super) fn survives_selection<'a>(
        &self,
        backend: &'a RealWgpuBackend,
        admitted: &native_template::SelectionTemplates,
        selected: &residency::RegistryLookup<'_, RenderResourceHandle, Arc<GeometryBuffers>>,
        pipelines: &BTreeSet<Arc<str>>,
        current: &mut native_template::Current<'a>,
    ) -> bool {
        let geometry = &backend.geometry;
        geometry.parameters.as_ref().is_some_and(|arena| {
            self.parameter_binding == arena.binding
                && self.parameter_buffer == arena.buffer
                && self.view_buffer == arena.view_buffer
                && self.index_buffer == arena.index_buffer
        }) && self.batches.iter().all(|b| {
            b.lights.is_none()
                && b.shadows.is_none()
                && (admitted.admitted(&b.native)
                    || (b.native.material_key.is_none()
                        && selected.get(&b.mesh).is_some()
                        && pipelines.contains(&b.pipeline_key)
                        && current.recording_current(backend, &b.native)))
        })
    }

    // All recordings share one immutable preparation borrow. Registry and
    // material checks must not rebuild per fragment of that same workset.
    pub(super) fn survives_prepare_with<'a>(
        &self,
        backend: &'a RealWgpuBackend,
        selected: &residency::RegistryLookup<'_, RenderResourceHandle, Arc<GeometryBuffers>>,
        pipelines: &BTreeSet<Arc<str>>,
        current: &mut native_template::Current<'a>,
    ) -> bool {
        let geometry = &backend.geometry;
        let mut last_mesh = None;
        let parameters_current = geometry.parameters.as_ref().is_some_and(|arena| {
            self.parameter_binding == arena.binding
                && self.parameter_buffer == arena.buffer
                && self.view_buffer == arena.view_buffer
                && self.index_buffer == arena.index_buffer
        });
        parameters_current
            && self.batches.iter().all(|batch| {
                let selected = last_mesh == Some(batch.mesh) || {
                    #[cfg(test)]
                    if !selected.is_direct() {
                        SURVIVAL_MESH_SEARCHES.with(|n| n.set(n.get() + 1));
                    }
                    selected.get(&batch.mesh).is_some()
                };
                if selected {
                    last_mesh = Some(batch.mesh);
                }
                batch.native.material_key.is_none()
                    && batch.lights.is_none()
                    && batch.shadows.is_none()
                    && selected
                    && pipelines.contains(&batch.pipeline_key)
                    && current.recording_current(backend, &batch.native)
            })
    }

    pub(super) fn record<'a>(&'a self, pass: &mut impl wgpu::util::RenderEncoder<'a>) -> u64 {
        pass.set_bind_group(0, Some(&self.parameter_binding), &[]);
        pass.set_vertex_buffer(1, self.index_buffer.slice(..));
        let mut last_pipeline = None;
        let mut last_material = None;
        let mut last_lights = None;
        let mut last_shadows = None;
        let mut last_vertices = None;
        let mut last_indices = None;
        for batch in &self.batches {
            if last_pipeline != Some(&batch.pipeline) {
                pass.set_pipeline(&batch.pipeline);
                #[cfg(test)]
                NATIVE_PIPELINE_BINDINGS.with(|n| n.set(n.get() + 1));
                last_pipeline = Some(&batch.pipeline);
                last_material = None;
                // A different pipeline may invalidate bind-group compatibility.
                // Reuse is only local encoder state, never resource admission.
                last_lights = None;
                last_shadows = None;
            }
            if let Some(shadows) = &batch.shadows {
                if last_shadows != Some(shadows) {
                    pass.set_bind_group(3, Some(shadows), &[]);
                    #[cfg(test)]
                    VIEW_BINDING_CALLS.with(|n| {
                        let mut v = n.get();
                        v[0] += 1;
                        n.set(v);
                    });
                    last_shadows = Some(shadows);
                }
            }
            if let Some(lights) = &batch.lights {
                if last_lights != Some(lights) {
                    pass.set_bind_group(2, Some(lights), &[]);
                    #[cfg(test)]
                    VIEW_BINDING_CALLS.with(|n| {
                        let mut v = n.get();
                        v[1] += 1;
                        n.set(v);
                    });
                    last_lights = Some(lights);
                }
            }
            if let Some(material) = &batch.material {
                if last_material != Some(material) {
                    pass.set_bind_group(1, Some(material), &[]);
                    last_material = Some(material);
                }
            }
            if last_vertices != Some(&batch.vertices) {
                pass.set_vertex_buffer(0, batch.vertices.slice(..));
                last_vertices = Some(&batch.vertices);
            }
            if last_indices != Some((&batch.indices, batch.index_format)) {
                pass.set_index_buffer(batch.indices.slice(..), batch.index_format);
                last_indices = Some((&batch.indices, batch.index_format));
            }
            pass.draw_indexed(
                batch.index_range.clone(),
                batch.base_vertex,
                batch.instance_range.clone(),
            );
            #[cfg(test)]
            NATIVE_DRAW_RECORDINGS.with(|n| n.set(n.get() + 1));
        }
        self.batches.len() as u64
    }

    pub(super) fn observe<'a>(
        &'a self,
        gpu: &mut GpuMemoryCensus<'a>,
        sample: &mut ResourceMemorySample,
        role: &str,
    ) {
        if gpu.is_backing_only() {
            self.backing.observe(self, gpu, role);
            return;
        }
        for buffer in [
            &self.parameter_buffer,
            &self.view_buffer,
            &self.index_buffer,
        ]
        .into_iter()
        .chain(&self.extra_buffers)
        {
            gpu.buffer(buffer, "renderer", &[role]);
        }
        for batch in &self.batches {
            #[cfg(test)]
            RECIPE_BACKING_BATCH_VISITS.with(|n| n.set(n.get() + 1));
            batch.native.observe_gpu(gpu, sample, role);
        }
        for texture in &self.textures {
            gpu.texture(texture, &[role]);
        }
        if !gpu.is_backing_only() {
            self.backing.observe_memory(sample, role);
            sample.cpu_vec(&self.batches, "renderer", &[role]);
            sample.cpu_vec(&self.extra_buffers, "renderer", &[role]);
            sample.cpu_vec(&self.textures, "renderer", &[role]);
            for batch in &self.batches {
                sample.cpu_arc(&batch.native, "renderer", &[role]);
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct Recorded {
    pub(super) command_count: usize,
    pub(super) recipe: Recipe,
    pub(super) bundle: wgpu::RenderBundle,
    pub(super) draws: u64,
    pub(super) ready: bool,
}
impl Recorded {
    pub(super) fn observe<'a>(
        &'a self,
        gpu: &mut GpuMemoryCensus<'a>,
        sample: &mut ResourceMemorySample,
        role: &str,
    ) {
        self.recipe.observe(gpu, sample, role);
    }
}
impl RetiredGpuResource for Recorded {
    fn observe<'a>(&'a self, gpu: &mut GpuMemoryCensus<'a>, sample: &mut ResourceMemorySample) {
        self.observe(gpu, sample, "retiring");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a real GPU"]
    fn current_capture_alternating_view_keys_keeps_complete_dependency_closure() {
        let plan = native_template::shared_native_material_plan(6);
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        backend
            .prepare_geometry_plan_for_extent(&plan, [64, 64], None)
            .unwrap();
        let start = plan
            .commands
            .iter()
            .position(|c| geometry_draw(c).is_some_and(|d| d.lighting.is_some()))
            .unwrap();
        let end = geometry_draw_end(&plan.commands, start);
        let original = geometry_draw(&plan.commands[start]).unwrap();
        let mut other_view = original.view.as_ref().clone();
        other_view.view_id = 991;
        let other_light = crate::render_geometry::ClusteredLights::build(&other_view, &[]).unwrap();
        assert_ne!(
            other_light.identity(),
            original.lighting.as_ref().unwrap().identity()
        );
        let buffer = backend
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("capture-alternate-lights"),
                contents: bytemuck::cast_slice(other_light.words()),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let binding = backend
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("capture-alternate-lights"),
                layout: &backend.geometry.light_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
        backend.geometry.light_buffers.insert(
            other_light.identity().into(),
            lights::LightBuffer { buffer, binding },
        );
        let shadow = &backend.geometry.shadow_bindings[&original.view.view_id];
        backend.geometry.shadow_bindings.insert(
            other_view.view_id,
            shadow.clone(),
        );
        let mut commands = plan.commands[start..end].to_vec();
        for command in &mut commands[2..4] {
            let mut draw = geometry_draw(command).unwrap().clone();
            draw.view = Arc::new(other_view.clone());
            draw.lighting = Some(other_light.clone());
            if let RhiCommand::Draw { payload, .. } = command {
                *payload = crate::rhi_command_plan::RhiDrawPayload::Geometry {
                    draw: Arc::new(draw),
                };
            } else {
                panic!("fixture expected a geometry draw");
            }
        }
        let draws: Vec<_> = commands.iter().filter_map(geometry_draw).collect();
        let oracle = Recipe::capture(&backend, &draws, 0);
        CAPTURE_BINDING_QUERIES.with(|n| n.set([0; 2]));
        let actual = Recipe::capture_commands(&backend, &commands, 0).unwrap();
        assert!(actual == oracle);
        assert_eq!(CAPTURE_BINDING_QUERIES.with(std::cell::Cell::get), [3, 3]);
        assert_eq!(
            actual.extra_buffers.len(),
            3,
            "A-B-A lost or duplicated a physical buffer"
        );
        assert!(actual.batches[0].lights != actual.batches[2].lights);
        assert!(actual.batches[0].lights == actual.batches[4].lights);
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn shared_native_recipe_capture_borrows_current_view_bindings_once() {
        let mut costs = Vec::new();
        for count in [4, 32, 256] {
            let plan = native_template::shared_native_material_plan(count);
            let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
            backend
                .prepare_geometry_plan_for_extent(&plan, [64, 64], None)
                .unwrap();
            let start = plan
                .commands
                .iter()
                .position(|c| geometry_draw(c).is_some_and(|d| d.lighting.is_some()))
                .unwrap();
            let end = geometry_draw_end(&plan.commands, start);
            let draws: Vec<_> = plan.commands[start..end]
                .iter()
                .filter_map(geometry_draw)
                .collect();
            assert!(draws
                .iter()
                .all(|draw| draw.lighting.is_some() && draw.shadows.is_some()));
            let oracle = Recipe::capture(&backend, &draws, 0);
            CAPTURE_BINDING_QUERIES.with(|n| n.set([0; 2]));
            let recipe = Recipe::capture_commands(&backend, &plan.commands[start..end], 0).unwrap();
            assert!(recipe == oracle);
            assert_eq!(recipe.batches.len(), count);
            assert_eq!(recipe.extra_buffers.len(), 2);
            costs.push((count, CAPTURE_BINDING_QUERIES.with(std::cell::Cell::get)));
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
        eprintln!("shared native actual recipe light/shadow queries {costs:?}");
        assert!(
            costs.iter().all(|(_, queries)| *queries == [1, 1]),
            "{costs:?}"
        );
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn immutable_recipe_backing_reuses_exact_dependency_locations_and_clone_resets_them() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        backend
            .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
            .unwrap();
        let start = plan
            .commands
            .iter()
            .position(|c| geometry_draw(c).is_some())
            .unwrap();
        let end = geometry_draw_end(&plan.commands, start);
        let base = Recipe::capture_commands(&backend, &plan.commands[start..end], 0).unwrap();
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let mut recipe = base.clone();
            let mut batch = base.batches[0].clone();
            let native = Arc::make_mut(&mut batch.native);
            native.environment = Some(backend.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("recipe-physical-environment"),
                size: 64,
                usage: wgpu::BufferUsages::UNIFORM,
                mapped_at_creation: false,
            }));
            let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("recipe-physical-material"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            native.textures.push((
                RenderResourceHandle {
                    kind: crate::render_resource::RenderResourceKind::Texture,
                    index: 404,
                    generation: 7,
                },
                texture,
            ));
            recipe.batches = vec![batch; count];
            let capture = |recipe: &Recipe| {
                let mut gpu = GpuMemoryCensus::backing_only();
                recipe.observe(
                    &mut gpu,
                    &mut ResourceMemorySample::new(0, 0, 0),
                    "retiring",
                );
                gpu.requested_bytes()
            };
            let expected = capture(&recipe);
            assert_eq!(
                expected,
                capture(&base) + 64 + 4,
                "physical material/environment aliases were lost or double counted"
            );
            RECIPE_BACKING_BATCH_VISITS.with(|n| n.set(0));
            for _ in 0..8 {
                assert_eq!(capture(&recipe), expected);
            }
            actual.push((count, RECIPE_BACKING_BATCH_VISITS.with(|n| n.get())));
            let mut detached = recipe.clone();
            let changed = Arc::make_mut(&mut detached.batches[count - 1].native);
            changed.indices = backend.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("recipe-detached-exact-backing"),
                size: 64,
                usage: wgpu::BufferUsages::INDEX,
                mapped_at_creation: false,
            });
            // Same typed mesh identity, distinct native allocation at the tail.
            assert_eq!(capture(&detached), expected + 64);
            assert_eq!(capture(&recipe), expected);
            let mut trace_gpu = GpuMemoryCensus::default();
            let mut trace = ResourceMemorySample::new(0, 0, 0);
            recipe.observe(&mut trace_gpu, &mut trace, "retiring");
            assert_eq!(trace_gpu.requested_bytes(), expected);
        }
        eprintln!("actual repeated backing batch visits: {actual:?}");
        assert!(
            actual.iter().all(|(_, n)| *n == 0),
            "immutable recipe was rewalked: {actual:?}"
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn retained_native_templates_are_consumed_without_re_resolving_objects() {
        for count in [4, 32, 256] {
            let graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            backend
                .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
                .unwrap();
            let start = plan
                .commands
                .iter()
                .position(|c| geometry_draw(c).is_some())
                .unwrap();
            let end = geometry_draw_end(&plan.commands, start);
            NATIVE_OBJECT_RESOLUTIONS.with(|n| n.set(0));
            let recipe = Recipe::capture_commands(&backend, &plan.commands[start..end], 0).unwrap();
            let actual = NATIVE_OBJECT_RESOLUTIONS.with(|n| n.get());
            assert_eq!(
                actual, 0,
                "scale {count}: stable native objects were resolved again after prepare"
            );
            assert_eq!(
                recipe
                    .batches
                    .iter()
                    .map(|b| b.instance_range.len())
                    .sum::<usize>(),
                count
            );
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn flow_stream_recipe_matches_flat_reference_for_ordered_batches_and_offsets() {
        use crate::render_geometry::{batch_test_frame, batch_test_graph};
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 32, 1);
        let plan =
            crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&batch_test_graph(&frame));
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        let commands: Vec<_> = plan
            .commands
            .iter()
            .filter(|c| geometry_draw(c).is_some())
            .cloned()
            .collect();
        let mut mixed = commands.clone();
        let mut legacy = commands[0].clone();
        if let RhiCommand::Draw { payload, .. } = &mut legacy {
            *payload = RhiDrawPayload::Geometry {
                draw: payload.geometry_draw().unwrap().clone(),
            };
        }
        mixed.insert(1, legacy);
        mixed.extend(commands.iter().rev().cloned());
        let draws: Vec<_> = mixed.iter().filter_map(geometry_draw).collect();
        let counts: Vec<_> = mixed
            .iter()
            .map(|c| c.geometry_instance_count() as u32)
            .collect();
        for offset in [0, 17, 101] {
            let expected = Recipe::capture_counts(&backend, &draws, &counts, offset);
            let actual = Recipe::capture_commands(&backend, &mixed, offset).unwrap();
            assert!(actual == expected, "offset={offset}");
            assert_eq!(actual.batches.first().unwrap().instance_range.start, offset);
            assert_eq!(
                actual.batches.last().unwrap().instance_range.end,
                offset + counts.iter().sum::<u32>()
            );
        }
        assert_eq!(
            Recipe::capture_commands(&backend, &[], 0).err().as_deref(),
            Some("geometry.invalid_draw_range")
        );
        let expected = Recipe::capture_counts(&backend, &draws, &counts, 17);
        mixed.push(
            plan.commands
                .iter()
                .find(|c| geometry_draw(c).is_none())
                .unwrap()
                .clone(),
        );
        assert_eq!(
            Recipe::capture_commands(&backend, &mixed, 0)
                .err()
                .as_deref(),
            Some("geometry.invalid_draw_range")
        );
        backend.geometry.parameters.as_mut().unwrap().index_buffer =
            backend.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("flow-replaced-index-backing"),
                size: 4096,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });
        let changed = Recipe::capture_commands(&backend, &commands, 17).unwrap();
        assert!(changed.index_buffer != expected.index_buffer);
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn flow_recipe_command_consumer_has_no_expanded_descriptor_arrays() {
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
            let commands: Vec<_> = plan
                .commands
                .iter()
                .filter(|c| geometry_draw(c).is_some())
                .cloned()
                .collect();
            let draws: Vec<_> = commands.iter().filter_map(geometry_draw).collect();
            let expected = Recipe::capture(&backend, &draws, 0);
            RECIPE_DESCRIPTOR_TEMP_ROWS.with(|n| n.set(0));
            let current = Recipe::capture_commands(&backend, &commands, 0).unwrap();
            actual.push((count, RECIPE_DESCRIPTOR_TEMP_ROWS.with(|n| n.get())));
            assert!(current == expected);
        }
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        eprintln!("actual temporary descriptor rows: {actual:?}");
        assert!(actual.iter().all(|(_, rows)| *rows == 0));
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn residual_recipe_clones_unique_native_dependencies_once_at_multiple_scales() {
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let handle = RenderResourceHandle {
            kind: crate::render_resource::RenderResourceKind::Texture,
            index: 404,
            generation: 7,
        };
        backend
            .register_rgba8_texture(handle, 1, 1, &[255; 4], "nearestClamp")
            .unwrap();
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let mut graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            let mut index = 0;
            let mut binding = None;
            for c in graph.passes.iter_mut().flat_map(|p| &mut p.commands) {
                if let crate::render_graph::RenderPassCommand::DrawGeometry { draw } = c {
                    let draw = Arc::make_mut(draw);
                    draw.cull = if index % 2 == 0 {
                        MaterialCull::Back
                    } else {
                        MaterialCull::Front
                    };
                    index += 1;
                    draw.material_binding = binding
                        .get_or_insert_with(|| {
                            let mut b = draw.material_binding.as_ref().clone();
                            b.slots
                                .push(crate::render_contract::NamedRenderBinding::new(
                                    "baseColorTexture",
                                    handle,
                                ));
                            Arc::new(b)
                        })
                        .clone();
                }
            }
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
            let draws = plan
                .commands
                .iter()
                .filter_map(geometry_draw)
                .collect::<Vec<_>>();
            RECIPE_TEXTURE_CLONES.with(|n| n.set(0));
            let recipe = Recipe::capture(&backend, &draws, 0);
            actual.push((count, RECIPE_TEXTURE_CLONES.with(|n| n.get())));
            assert_eq!(recipe.batches.len(), count);
            assert_eq!(recipe.textures.len(), 1);
            assert!(recipe.textures[0] == *backend.textures[&handle]._view.texture());
        }
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        eprintln!("recipe native texture clones {actual:?}");
        assert!(
            actual.iter().all(|(_, clones)| *clones == 1),
            "actual native texture clones {actual:?}"
        );
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn gpu_recording_survival_requires_current_native_backing_not_only_typed_handles() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        // This fixture qualifies allocation identities, not visible geometry.
        backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
        let draw = plan.commands.iter().find_map(geometry_draw).unwrap();
        let recipe = Recipe::capture(&backend, &[draw], 0);
        let meshes = [(draw.geometry_handle().raw(), draw.geometry.clone())].into();
        let pipelines =
            [draw.pipeline_key(&format!("{:?}", draw_color_format(draw, backend.format)))].into();
        assert!(recipe.survives_prepare(&backend, &meshes, &pipelines));
        let mut replaced = recipe.clone();
        Arc::make_mut(&mut replaced.batches[0].native).vertices =
            backend.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("same-typed-handle-different-native-backing"),
                size: 4,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });
        assert!(
            !replaced.survives_prepare(&backend, &meshes, &pipelines),
            "retention accepted a stale native buffer under the same typed handle"
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
}

#[cfg(test)]
thread_local! {static RECIPE_TEXTURE_CLONES: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};static RECIPE_BACKING_BATCH_VISITS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}

#[cfg(test)]
thread_local! {static RECIPE_DESCRIPTOR_TEMP_ROWS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}

#[cfg(test)]
thread_local! {static NATIVE_OBJECT_RESOLUTIONS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}

#[cfg(test)]
thread_local! {pub(super) static STABLE_DESCRIPTOR_CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };}

#[cfg(test)]
thread_local! {static VIEW_BINDING_CALLS: std::cell::Cell<[usize;2]> = const {std::cell::Cell::new([0;2])};}
#[cfg(test)]
thread_local! {pub(super) static SURVIVAL_MESH_SEARCHES: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
thread_local! {pub(super) static RECIPE_CAPTURE_ROWS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}

#[cfg(test)]
#[path = "encoding_binding_tests.rs"]
mod binding_tests;

#[cfg(test)]
thread_local! {static CAPTURE_BINDING_QUERIES:std::cell::Cell<[usize;2]>=const{std::cell::Cell::new([0;2])};}
