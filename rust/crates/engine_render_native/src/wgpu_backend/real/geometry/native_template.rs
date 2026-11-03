//! Native command facts resolved after current resource admission. The memo is
//! weak: only current plans and completion-owned recordings keep objects alive.
use super::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Weak;

// Only immutable native maps cross the bounded frame task boundary. The
// backend's RefCell/Cell caches and every GPU mutation stay with its owner.
#[derive(Clone, Copy)]
pub(super) struct Registry<'a> {
    pipelines: &'a BTreeMap<String, wgpu::RenderPipeline>,
    lit_bindings: &'a BTreeMap<lit::TextureKey, wgpu::BindGroup>,
    environments: &'a BTreeMap<String, ibl::EnvironmentBuffer>,
    textures: &'a BTreeMap<RenderResourceHandle, super::super::ResidentTexture>,
}
impl<'a> Registry<'a> {
    pub(super) fn new(backend: &'a RealWgpuBackend) -> Self {
        Self {
            pipelines: &backend.geometry.pipelines,
            lit_bindings: &backend.geometry.lit_bindings,
            environments: &backend.geometry.environment_buffers,
            textures: &backend.textures,
        }
    }
    pub(super) fn has_pipeline(&self, key: &str) -> bool {
        self.pipelines.contains_key(key)
    }
}

#[derive(Clone)]
pub(super) struct IntentRecorded {
    geometry: Weak<RenderBindingSet>,
    material: Weak<RenderBindingSet>,
    pipeline: Weak<str>,
}

#[derive(Clone)]
pub(super) struct Template {
    // CPU registry success belongs only to this backend-owned template. It
    // proves no GPU residency or encoded/submitted frame. OnceLock is bounded
    // by the existing template; mismatched later inputs simply record again.
    pub(super) intent: std::sync::OnceLock<IntentRecorded>,
    pub(super) skin: Option<Arc<skin::NativeSkin>>,
    pub(super) mesh: RenderResourceHandle,
    pub(super) pipeline_key: Arc<str>,
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) vertices: wgpu::Buffer,
    pub(super) indices: wgpu::Buffer,
    pub(super) index_format: wgpu::IndexFormat,
    pub(super) index_range: std::ops::Range<u32>,
    pub(super) base_vertex: i32,
    pub(super) dependencies: Arc<MaterialDependencies>,
}
#[derive(Clone, PartialEq, Eq)]
pub(super) struct MaterialDependencies {
    pub(super) material: Option<wgpu::BindGroup>,
    pub(super) material_key: Option<lit::TextureKey>,
    pub(super) textures: Vec<(RenderResourceHandle, wgpu::Texture)>,
    pub(super) environment: Option<wgpu::Buffer>,
}
impl std::ops::Deref for Template {
    type Target = MaterialDependencies;
    fn deref(&self) -> &Self::Target {
        &self.dependencies
    }
}
// Mutable scalar-oracle fixtures must detach shared immutable dependencies.
// Normal native templates are only consumed through Arc<Template>.
impl std::ops::DerefMut for Template {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.dependencies)
    }
}
// Equality describes encoder state. Physical dependency closure is compared
// by Recipe independently, including oracle and retained organization.
impl PartialEq for Template {
    fn eq(&self, other: &Self) -> bool {
        self.mesh == other.mesh
            && self.pipeline_key == other.pipeline_key
            && self.pipeline == other.pipeline
            && self.material == other.material
            && self.vertices == other.vertices
            && self.indices == other.indices
            && self.index_format == other.index_format
            && self.index_range == other.index_range
            && self.base_vertex == other.base_vertex
    }
}
impl Eq for Template {}
impl Template {
    pub(super) fn intent_recorded(&self, draw: &DrawGeometry, pipeline: &Arc<str>) -> bool {
        self.intent.get().is_some_and(|proof| {
            proof.geometry.as_ptr() == Arc::as_ptr(&draw.geometry_binding)
                && proof.material.as_ptr() == Arc::as_ptr(&draw.material_binding)
                && std::ptr::eq(proof.pipeline.as_ptr(), Arc::as_ptr(pipeline))
        })
    }
    pub(super) fn mark_intent(&self, draw: &DrawGeometry, pipeline: &Arc<str>) {
        let _ = self.intent.set(IntentRecorded {
            geometry: Arc::downgrade(&draw.geometry_binding),
            material: Arc::downgrade(&draw.material_binding),
            pipeline: Arc::downgrade(pipeline),
        });
    }
    pub(super) fn capture(
        backend: &RealWgpuBackend,
        draw: &DrawGeometry,
        facts: &preparation::Requirements,
    ) -> Self {
        #[cfg(test)]
        OBJECT_RESOLUTIONS.with(|n| n.set(n.get() + 1));
        let handle = draw.geometry_handle().raw();
        let mesh = &backend.geometry.meshes[&handle];
        let index_format = match draw.geometry.index_format() {
            GeometryIndexFormat::Uint16 => wgpu::IndexFormat::Uint16,
            GeometryIndexFormat::Uint32 => wgpu::IndexFormat::Uint32,
        };
        let first = draw.first_index + (mesh.indices.start / index_bytes(index_format)) as u32;
        let mut material_key = None;
        let mut material = None;
        let mut textures = Vec::new();
        let mut environment = None;
        if !draw.is_opaque_shadow_depth_writer() {
            if draw.standard_lit.is_some() {
                // Resource admission already interpreted this immutable material.
                // Consume its facts instead of scanning material bindings again
                // for every distinct geometry template sharing the material.
                let key = (
                    facts.textures().map(|texture| texture.map(|h| h.raw())),
                    draw.environment
                        .as_ref()
                        .map(|e| e.map.identity().to_owned()),
                );
                material = Some(backend.geometry.lit_bindings[&key].clone());
                for handle in key.0.into_iter().flatten() {
                    textures.push((handle, backend.textures[&handle]._view.texture().clone()));
                }
                if let Some(id) = &key.1 {
                    environment = Some(backend.geometry.environment_buffers[id].buffer.clone());
                }
                material_key = Some(key);
            } else if let Some(handle) = facts.textures()[0] {
                let texture = &backend.textures[&handle.raw()];
                material = Some(texture.bind_group.clone());
                textures.push((handle.raw(), texture._view.texture().clone()));
            }
        }
        let skin = backend.geometry.skins.resolve(draw);
        Self {
            intent: Default::default(),
            skin: skin.clone(),
            mesh: handle,
            pipeline_key: facts.pipeline().clone(),
            pipeline: backend.geometry.pipelines[facts.pipeline().as_ref()].clone(),
            vertices: skin
                .as_ref()
                .map_or_else(|| mesh.vertices.buffer().clone(), |s| s.current.clone()),
            indices: mesh.indices.buffer().clone(),
            index_format,
            index_range: first..first + draw.index_count,
            base_vertex: if skin.is_some() {
                0
            } else {
                (mesh.vertices.start / u64::from(draw.geometry.vertex_stride())) as i32
            },
            dependencies: Arc::new(MaterialDependencies {
                material,
                material_key,
                textures,
                environment,
            }),
        }
    }

    // Strict scalar oracle. Production shares call-local registry borrows,
    // without retaining any success beyond the immutable backend borrow.
    #[cfg(test)]
    fn current(&self, backend: &RealWgpuBackend, draw: &DrawGeometry) -> bool {
        let Some(mesh) = backend.geometry.meshes.get(&self.mesh) else {
            return false;
        };
        self.geometry_current(backend, draw, mesh) && self.material_current(backend)
    }
    #[cfg(test)]
    fn geometry_current(
        &self,
        backend: &RealWgpuBackend,
        draw: &DrawGeometry,
        mesh: &ResidentGeometry,
    ) -> bool {
        self.geometry_range_current(draw, mesh)
            && current_pipeline(Registry::new(backend), self.pipeline_key.as_ref())
                == Some(&self.pipeline)
    }
    fn geometry_range_current(&self, draw: &DrawGeometry, mesh: &ResidentGeometry) -> bool {
        (if let Some(skin) = &self.skin {
            skin.matches(draw)
                && skin.original_matches(mesh)
                && self.vertices == skin.current
                && self.base_vertex == 0
        } else {
            draw.deformation.is_none()
                && mesh.vertices.buffer() == &self.vertices
                && self.base_vertex
                    == (mesh.vertices.start / u64::from(draw.geometry.vertex_stride())) as i32
        }) && mesh.indices.buffer() == &self.indices
            && self.index_range.start
                == draw.first_index + (mesh.indices.start / index_bytes(self.index_format)) as u32
    }
    #[cfg(test)]
    fn material_current(&self, backend: &RealWgpuBackend) -> bool {
        self.material_current_in(Registry::new(backend))
    }
    fn material_current_in(&self, registry: Registry<'_>) -> bool {
        if self.textures.iter().any(|(handle, texture)| {
            registry
                .textures
                .get(handle)
                .is_none_or(|t| t._view.texture() != texture)
        }) {
            return false;
        }
        if let Some(key) = &self.material_key {
            #[cfg(test)]
            CURRENT_REGISTRY_QUERIES.with(|n| {
                let mut v = n.get();
                v[1] += 1;
                n.set(v);
            });
            if registry.lit_bindings.get(key) != self.material.as_ref() {
                return false;
            }
            if let Some(id) = &key.1 {
                return registry.environments.get(id).map(|e| &e.buffer)
                    == self.environment.as_ref();
            }
        } else if let Some((handle, _)) = self.textures.first() {
            return registry.textures.get(handle).map(|t| &t.bind_group) == self.material.as_ref();
        }
        true
    }
    pub(super) fn texture_objects(&self) -> impl Iterator<Item = &wgpu::Texture> {
        self.textures.iter().map(|(_, t)| t)
    }
    pub(super) fn environment_buffer(&self) -> Option<&wgpu::Buffer> {
        self.environment.as_ref()
    }
    pub(super) fn observe(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
        role: &str,
    ) {
        sample.cpu_arc(&self.dependencies, "renderer", &[role]);
        sample.cpu_vec(&self.textures, "renderer", &[role]);
        sample.cpu_arc(&self.pipeline_key, "renderer", &[role]);
        if let Some((_, Some(id))) = &self.material_key {
            sample.cpu_string(id, "renderer", &[role]);
        }
    }
    pub(super) fn observe_gpu<'a>(
        &'a self,
        gpu: &mut super::super::memory::GpuMemoryCensus<'a>,
        sample: &mut crate::resource_memory::ResourceMemorySample,
        role: &str,
    ) {
        if !gpu.visit_template(self) {
            return;
        }
        #[cfg(test)]
        BACKING_DEPENDENCY_VISITS.with(|n| n.set(n.get() + 1));
        gpu.buffer(&self.vertices, "renderer", &[role]);
        if let Some(skin) = &self.skin {
            skin.observe(gpu, role);
        }
        gpu.buffer(&self.indices, "renderer", &[role]);
        if let Some(buffer) = &self.environment {
            gpu.buffer(buffer, "renderer", &[role]);
        }
        for texture in self.texture_objects() {
            gpu.texture(texture, &[role]);
        }
        if !gpu.is_backing_only() {
            self.observe(sample, role);
        }
    }
}
fn current_pipeline<'a>(registry: Registry<'a>, key: &str) -> Option<&'a wgpu::RenderPipeline> {
    #[cfg(test)]
    CURRENT_REGISTRY_QUERIES.with(|n| {
        let mut v = n.get();
        v[2] += 1;
        n.set(v);
    });
    registry.pipelines.get(key)
}
fn index_bytes(format: wgpu::IndexFormat) -> u64 {
    match format {
        wgpu::IndexFormat::Uint16 => 2,
        wgpu::IndexFormat::Uint32 => 4,
    }
}

// Scratch borrows today's registry and owns successful exact witnesses only.
// A shared bind group alone cannot establish dependency equality.
#[derive(Default)]
pub(super) struct Current<'a> {
    meshes: HashMap<RenderResourceHandle, Option<&'a ResidentGeometry>>,
    registry: Option<residency::RegistryLookup<'a, RenderResourceHandle, ResidentGeometry>>,
    materials: HashMap<wgpu::BindGroup, Arc<MaterialDependencies>>,
    last_material: Option<Arc<MaterialDependencies>>,
    // Exact immutable string ownership prevents address recycling. These are
    // current registry borrows only and cannot survive this prepare call.
    pipelines: HashMap<usize, (Arc<str>, Option<&'a wgpu::RenderPipeline>)>,
    pipeline_values: HashMap<Arc<str>, Option<&'a wgpu::RenderPipeline>>,
    last_pipeline: Option<(Arc<str>, Option<&'a wgpu::RenderPipeline>)>,
}
impl<'a> Current<'a> {
    pub(super) fn for_plan(backend: &'a RealWgpuBackend, draws: usize) -> Self {
        let registry = residency::RegistryLookup::new(&backend.geometry.meshes, draws, |handle| {
            handle.index as usize
        });
        Self {
            registry: registry.is_direct().then_some(registry),
            ..Self::default()
        }
    }

    pub(super) fn check(
        &mut self,
        backend: &'a RealWgpuBackend,
        template: &Arc<Template>,
        draw: &DrawGeometry,
    ) -> bool {
        #[cfg(test)]
        CURRENT_CHECKS.with(|n| n.set(n.get() + 1));
        let Some(mesh) = self.mesh(backend, &template.mesh) else {
            return false;
        };
        template.geometry_range_current(draw, mesh) && self.dependencies_current(backend, template)
    }
    pub(super) fn recording_current(
        &mut self,
        backend: &'a RealWgpuBackend,
        template: &Arc<Template>,
    ) -> bool {
        let Some(mesh) = self.mesh(backend, &template.mesh) else {
            return false;
        };
        (template.skin.as_ref().map_or_else(
            || mesh.vertices.buffer() == &template.vertices,
            |s| s.original_matches(mesh),
        )) && mesh.indices.buffer() == &template.indices
            && self.dependencies_current(backend, template)
    }
    pub(super) fn mesh(
        &mut self,
        backend: &'a RealWgpuBackend,
        handle: &RenderResourceHandle,
    ) -> Option<&'a ResidentGeometry> {
        if let Some(registry) = &self.registry {
            #[cfg(test)]
            if registry.dense_len() == 0 {
                CURRENT_REGISTRY_QUERIES.with(|n| {
                    let mut v = n.get();
                    v[0] += 1;
                    n.set(v);
                });
            }
            registry.get(handle)
        } else {
            *self.meshes.entry(*handle).or_insert_with(|| {
                #[cfg(test)]
                CURRENT_REGISTRY_QUERIES.with(|n| {
                    let mut v = n.get();
                    v[0] += 1;
                    n.set(v);
                });
                backend.geometry.meshes.get(handle)
            })
        }
    }
    fn dependencies_current(
        &mut self,
        backend: &'a RealWgpuBackend,
        template: &Arc<Template>,
    ) -> bool {
        self.dependencies_current_in(Registry::new(backend), template)
    }
    pub(super) fn check_in(
        &mut self,
        registry: Registry<'a>,
        template: &Arc<Template>,
        draw: &DrawGeometry,
        resident: Option<&ResidentGeometry>,
    ) -> bool {
        #[cfg(test)]
        CURRENT_CHECKS.with(|n| n.set(n.get() + 1));
        template.mesh == draw.geometry_handle().raw()
            && resident.is_some_and(|mesh| template.geometry_range_current(draw, mesh))
            && self.dependencies_current_in(registry, template)
    }
    fn dependencies_current_in(
        &mut self,
        registry: Registry<'a>,
        template: &Arc<Template>,
    ) -> bool {
        let pipeline = if let Some((_, pipeline)) = self
            .last_pipeline
            .as_ref()
            .filter(|(key, _)| Arc::ptr_eq(key, &template.pipeline_key))
        {
            *pipeline
        } else {
            let key = Arc::as_ptr(&template.pipeline_key) as *const () as usize;
            #[cfg(test)]
            PIPELINE_CACHE_SEARCHES.with(|n| n.set(n.get() + 1));
            let (_, pipeline) = self.pipelines.entry(key).or_insert_with(|| {
                // Equal pipeline keys can have detached string allocations. The
                // address front avoids rehashing each repeated string; the value
                // map still resolves each distinct registry key exactly once.
                let pipeline = *self
                    .pipeline_values
                    .entry(template.pipeline_key.clone())
                    .or_insert_with(|| current_pipeline(registry, &template.pipeline_key));
                (template.pipeline_key.clone(), pipeline)
            });
            let pipeline = *pipeline;
            self.last_pipeline = Some((template.pipeline_key.clone(), pipeline));
            pipeline
        };
        if pipeline != Some(&template.pipeline) {
            return false;
        }
        if let Some(group) = &template.material {
            if self.last_material.as_ref().is_some_and(|checked| {
                if Arc::ptr_eq(checked, &template.dependencies) {
                    return true;
                }
                #[cfg(test)]
                MATERIAL_VALUE_COMPARISONS.with(|n| n.set(n.get() + 1));
                checked.as_ref() == template.dependencies.as_ref()
            }) {
                return true;
            }
            #[cfg(test)]
            MATERIAL_CACHE_SEARCHES.with(|n| n.set(n.get() + 1));
            if self.materials.get(group).is_some_and(|checked| {
                if Arc::ptr_eq(checked, &template.dependencies) {
                    return true;
                }
                #[cfg(test)]
                MATERIAL_VALUE_COMPARISONS.with(|n| n.set(n.get() + 1));
                checked.as_ref() == template.dependencies.as_ref()
            }) {
                self.last_material = Some(template.dependencies.clone());
                return true;
            }
        }
        if !template.material_current_in(registry) {
            return false;
        }
        if let Some(group) = &template.material {
            self.materials
                .insert(group.clone(), template.dependencies.clone());
            self.last_material = Some(template.dependencies.clone());
        }
        true
    }

    // Capture just read these exact native dependencies from today's backend.
    // Share their storage through this existing call-local successful map;
    // no new registry, native owner or cross-frame admission flag is added.
    fn share_captured(&mut self, template: &mut Template) {
        if let Some(group) = &template.material {
            if let Some(checked) = self
                .materials
                .get(group)
                .filter(|checked| checked.as_ref() == template.dependencies.as_ref())
            {
                template.dependencies = checked.clone();
            } else {
                self.materials
                    .insert(group.clone(), template.dependencies.clone());
            }
            self.last_material = Some(template.dependencies.clone());
        }
    }
}

type Key = (usize, usize, usize);
struct Entry {
    _facts: Weak<preparation::Requirements>,
    _environment: Option<Weak<crate::environment_lighting::EnvironmentLighting>>,
    template: Weak<Template>,
    prepare: u64,
}
#[derive(Clone, Copy)]
pub(super) struct Lookup<'a> {
    entries: &'a HashMap<Key, Entry>,
}
impl Lookup<'_> {
    pub(super) fn current_template<'a>(
        &self,
        registry: Registry<'a>,
        draw: &DrawGeometry,
        facts: &Arc<preparation::Requirements>,
        current: &mut Current<'a>,
        resident: Option<&ResidentGeometry>,
    ) -> Option<Arc<Template>> {
        let template = self
            .entries
            .get(&Templates::key(draw, facts))?
            .template
            .upgrade()?;
        current
            .check_in(registry, &template, draw, resident)
            .then_some(template)
    }
}
pub(super) struct Templates {
    entries: RefCell<HashMap<Key, Entry>>,
    limit: usize,
    next_prune: std::cell::Cell<usize>,
    prepare: std::cell::Cell<u64>,
    pub(super) memo_writes: std::cell::Cell<u64>,
}

// One selection's admission result. Record first occurrences while the actual
// immutable scan is in progress; installation resolves only missing rows. This
// scratch never certifies residency beyond that selection's backend borrow.
pub(super) struct SelectionTemplates {
    aligned: Vec<Option<Arc<Template>>>,
    retained: Vec<(usize, Arc<Template>)>,
    admitted: HashMap<*const Template, usize>,
    missing: Vec<usize>,
}
impl SelectionTemplates {
    pub(super) fn with_capacity(commands: usize) -> Self {
        Self {
            aligned: Vec::with_capacity(commands),
            retained: Vec::new(),
            admitted: HashMap::new(),
            missing: Vec::new(),
        }
    }
    pub(super) fn push(&mut self, template: Option<Arc<Template>>) {
        let index = self.aligned.len();
        if let Some(template) = &template {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                self.admitted.entry(Arc::as_ptr(template))
            {
                entry.insert(self.retained.len());
                self.retained.push((index, template.clone()));
            }
        } else {
            self.missing.push(index);
        }
        self.aligned.push(template);
    }
    pub(super) fn control(&mut self) {
        self.aligned.push(None);
    }
    pub(super) fn admitted(&self, template: &Arc<Template>) -> bool {
        self.admitted.contains_key(&Arc::as_ptr(template))
    }
    pub(super) fn complete(
        mut self,
        templates: &Templates,
        backend: &RealWgpuBackend,
        plan: &RhiCommandPlan,
        facts: &[Option<Arc<preparation::Requirements>>],
    ) -> (Vec<Option<Arc<Template>>>, Vec<Arc<Template>>) {
        let had_missing = !self.missing.is_empty();
        for index in self.missing {
            #[cfg(test)]
            SELECTION_TEMPLATE_REVISITS.with(|n| n.set(n.get() + 1));
            let draw = geometry_draw(&plan.commands[index]).unwrap();
            let template = templates.resolve(backend, draw, facts[index].as_ref().unwrap());
            match self.admitted.entry(Arc::as_ptr(&template)) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    // A previously missing earlier occurrence may resolve to
                    // an admitted later one. Preserve original census order.
                    let first = &mut self.retained[*entry.get()].0;
                    *first = (*first).min(index);
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(self.retained.len());
                    self.retained.push((index, template.clone()));
                }
            }
            self.aligned[index] = Some(template);
        }
        if had_missing {
            self.retained.sort_unstable_by_key(|(index, _)| *index);
        }
        (
            self.aligned,
            self.retained
                .into_iter()
                .map(|(_, template)| template)
                .collect(),
        )
    }
}
impl Default for Templates {
    fn default() -> Self {
        Self {
            entries: Default::default(),
            limit: 65_536,
            next_prune: std::cell::Cell::new(128),
            prepare: std::cell::Cell::new(0),
            memo_writes: Default::default(),
        }
    }
}
#[cfg(test)]
#[test]
fn dead_dynamic_native_template_metadata_is_reclaimed_before_capacity() {
    let memo = Templates::default();
    for i in 0..256 {
        memo.entries.borrow_mut().insert(
            (i, 0, 0),
            Entry {
                _facts: Weak::new(),
                _environment: None,
                template: Weak::new(),
                prepare: 0,
            },
        );
    }
    memo.prune();
    assert!(
        memo.entries.borrow().is_empty(),
        "dead template headers survived below the global capacity"
    );
}
impl Templates {
    pub(super) fn with_lookup<T>(&self, read: impl FnOnce(Lookup<'_>) -> T) -> T {
        let entries = self.entries.borrow();
        read(Lookup { entries: &entries })
    }
    pub(super) fn key(draw: &DrawGeometry, facts: &Arc<preparation::Requirements>) -> Key {
        (
            Arc::as_ptr(facts) as usize,
            draw.deformation
                .as_ref()
                .map_or(0, |s| Arc::as_ptr(s) as usize),
            draw.environment
                .as_ref()
                .map_or(0, |e| Arc::as_ptr(e) as usize),
        )
    }
    // Amortized at the existing preparation boundary, never a full-table walk
    // for every overflowed draw. A full live memo falls back without rejection.
    pub(super) fn prune(&self) {
        let mut entries = self.entries.borrow_mut();
        if entries.len() >= self.next_prune.get().min(self.limit) {
            entries.retain(|_, entry| entry.template.strong_count() != 0);
            self.next_prune.set(entries.len().saturating_add(128));
        }
    }
    pub(super) fn resolve(
        &self,
        backend: &RealWgpuBackend,
        draw: &DrawGeometry,
        facts: &Arc<preparation::Requirements>,
    ) -> Arc<Template> {
        self.resolve_with_current(backend, draw, facts, &mut Current::default(), None)
            .0
    }
    fn resolve_with_current<'a>(
        &self,
        backend: &'a RealWgpuBackend,
        draw: &DrawGeometry,
        facts: &Arc<preparation::Requirements>,
        current: &mut Current<'a>,
        prepare: Option<u64>,
    ) -> (Arc<Template>, bool, bool) {
        let key = Self::key(draw, facts);
        let mut entries = self.entries.borrow_mut();
        if let Some(entry) = entries.get_mut(&key) {
            if let Some(template) = entry.template.upgrade() {
                let first = prepare.is_none_or(|id| id != entry.prepare);
                // Repeated keys only reuse today's successful immutable backend
                // borrow. A new prepare always checks real native objects again.
                if !first || current.check(backend, &template, draw) {
                    if let Some(id) = prepare {
                        entry.prepare = id;
                    }
                    return (template, first, true);
                }
            }
        }
        let mut template = Template::capture(backend, draw, facts);
        current.share_captured(&mut template);
        let template = Arc::new(template);
        let remembered = entries.len() < self.limit || entries.contains_key(&key);
        if remembered {
            self.memo_writes.set(self.memo_writes.get() + 1);
            entries.insert(
                key,
                Entry {
                    _facts: Arc::downgrade(facts),
                    _environment: draw.environment.as_ref().map(Arc::downgrade),
                    template: Arc::downgrade(&template),
                    prepare: prepare.unwrap_or(0),
                },
            );
        }
        (template, true, remembered)
    }
    pub(super) fn prepare(
        &self,
        backend: &RealWgpuBackend,
        plan: &RhiCommandPlan,
        facts: &[Option<Arc<preparation::Requirements>>],
    ) -> (Vec<Option<Arc<Template>>>, Vec<Arc<Template>>) {
        let prepare = if self.prepare.get() == u64::MAX {
            for entry in self.entries.borrow_mut().values_mut() {
                entry.prepare = 0;
            }
            1
        } else {
            self.prepare.get() + 1
        };
        self.prepare.set(prepare);
        // Only overflowed entries need temporary ownership. The ordinary weak
        // memo already identifies each first visit within this prepare call.
        let mut overflow = HashMap::new();
        let mut retained = Vec::new();
        let mut current = Current::for_plan(backend, facts.iter().flatten().count());
        let aligned = plan
            .commands
            .iter()
            .zip(facts)
            .map(|(command, facts)| {
                let draw = geometry_draw(command)?;
                let facts = facts.as_ref()?;
                let key = Self::key(draw, facts);
                if let Some(template) = overflow.get(&key) {
                    return Some(Arc::clone(template));
                }
                let (template, first, remembered) =
                    self.resolve_with_current(backend, draw, facts, &mut current, Some(prepare));
                if first && retained.len() < 65_536 {
                    retained.push(template.clone());
                }
                if !remembered {
                    #[cfg(test)]
                    PREPARE_SCRATCH_INSERTIONS.with(|n| n.set(n.get() + 1));
                    overflow.insert(key, template.clone());
                }
                Some(template)
            })
            .collect();
        (aligned, retained)
    }
    pub(super) fn observe(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.partial(
            "cpu.owner.renderer",
            "nativeTemplateWeakHashBucketsAndPrepareScratchNotInstrumented",
        );
    }
}

#[cfg(test)]
thread_local! {pub(super) static SELECTION_TEMPLATE_REVISITS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
thread_local! {static MATERIAL_VALUE_COMPARISONS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a real GPU"]
    fn selection_completion_only_resolves_missing_rows_and_keeps_first_occurrence_order() {
        let mut plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &crate::rhi_command_plan::tests::retained_geometry_graph(4),
        );
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        backend
            .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
            .unwrap();
        let cached = backend.geometry.validated_plan.as_ref().unwrap();
        let rows: Vec<_> = plan
            .commands
            .iter()
            .zip(&cached.requirements)
            .zip(cached.native_templates.iter())
            .filter_map(|((command, facts), template)| {
                Some((
                    geometry_draw(command).map(|_| command.clone())?,
                    facts.clone()?,
                    template.clone()?,
                ))
            })
            .collect();
        let order = [3, 1, 3, 2, 1];
        plan.commands = order
            .iter()
            .map(|i| rows[*i].0.clone())
            .collect::<Vec<_>>()
            .into();
        let facts: Vec<_> = order.iter().map(|i| Some(rows[*i].1.clone())).collect();
        for misses in [vec![], vec![0], vec![0, 2, 4], vec![0, 1, 2, 3, 4]] {
            let mut selected = SelectionTemplates::with_capacity(order.len());
            for (index, row) in order.iter().enumerate() {
                selected.push((!misses.contains(&index)).then(|| rows[*row].2.clone()));
            }
            SELECTION_TEMPLATE_REVISITS.with(|n| n.set(0));
            let (aligned, retained) =
                selected.complete(&backend.geometry.native_templates, &backend, &plan, &facts);
            assert_eq!(SELECTION_TEMPLATE_REVISITS.with(|n| n.get()), misses.len());
            for (template, row) in aligned.iter().zip(order) {
                assert!(Arc::ptr_eq(template.as_ref().unwrap(), &rows[row].2));
            }
            let mut expected = Vec::new();
            for row in order {
                if !expected.iter().any(|t| Arc::ptr_eq(t, &rows[row].2)) {
                    expected.push(rows[row].2.clone());
                }
            }
            assert_eq!(retained.len(), expected.len());
            assert!(retained
                .iter()
                .zip(&expected)
                .all(|(a, b)| Arc::ptr_eq(a, b)));
        }
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn dense_native_mesh_checks_walk_current_registry_without_per_mesh_searches() {
        let mut observed = Vec::new();
        for count in [4, 32, 256] {
            let mut plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
                &crate::rhi_command_plan::tests::retained_geometry_graph(count),
            );
            let mut index = 0;
            for command in &mut plan.commands {
                if let RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } = command
                {
                    index += 1;
                    Arc::make_mut(draw).geometry_binding = Arc::new(
                        RenderBindingSet::single(
                            format!("dense-native-{index}"),
                            RenderBindingKind::Mesh,
                            RenderResourceHandle {
                                kind: crate::render_resource::RenderResourceKind::MeshBuffer,
                                index,
                                generation: 1,
                            },
                            "nearestClamp".into(),
                            false,
                            "dense-native-test".into(),
                        )
                        .unwrap(),
                    );
                }
            }
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            backend
                .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
                .unwrap();
            assert_eq!(backend.geometry.meshes.len(), count);
            backend
                .geometry
                .validated_plan
                .as_mut()
                .unwrap()
                .command_backing = Weak::new();
            CURRENT_REGISTRY_QUERIES.with(|n| n.set([0; 3]));
            OBJECT_RESOLUTIONS.with(|n| n.set(0));
            PREPARE_SCRATCH_INSERTIONS.with(|n| n.set(0));
            PIPELINE_CACHE_SEARCHES.with(|n| n.set(0));
            backend
                .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
                .unwrap();
            observed.push((count, CURRENT_REGISTRY_QUERIES.with(|n| n.get()[0])));
            assert_eq!(
                PREPARE_SCRATCH_INSERTIONS.with(|n| n.get()),
                0,
                "warm native preparation rebuilt a temporary unique-template map at {count}"
            );
            assert_eq!(OBJECT_RESOLUTIONS.with(std::cell::Cell::get), 0);
            assert!(
                PIPELINE_CACHE_SEARCHES.with(|n| n.get()) <= 2,
                "one repeated pipeline still searched the call-local hash table per template"
            );
            // Every check still reads this call's registry. Removal and a new
            // physical allocation with the same typed identity must be visible.
            let template = backend.geometry.current_templates.first().unwrap().clone();
            let draw = plan
                .commands
                .iter()
                .filter_map(geometry_draw)
                .find(|draw| draw.geometry_handle().raw() == template.mesh)
                .unwrap();
            assert!(
                Current::for_plan(&backend, 1).registry.is_none(),
                "sparse closure eagerly walked unrelated meshes"
            );
            assert_eq!(
                Current::for_plan(&backend, count)
                    .registry
                    .unwrap()
                    .dense_len(),
                count
            );
            let removed = backend.geometry.meshes.remove(&template.mesh).unwrap();
            assert!(!Current::for_plan(&backend, count).check(&backend, &template, draw));
            backend.geometry.meshes.insert(template.mesh, removed);
            assert!(Current::for_plan(&backend, count).check(&backend, &template, draw));
            let original = backend.geometry.meshes[&template.mesh].vertices.start;
            backend
                .geometry
                .meshes
                .get_mut(&template.mesh)
                .unwrap()
                .vertices
                .start += u64::from(draw.geometry.vertex_stride());
            assert!(
                !Current::for_plan(&backend, count).check(&backend, &template, draw),
                "current dense registry hid a changed physical range"
            );
            backend
                .geometry
                .meshes
                .get_mut(&template.mesh)
                .unwrap()
                .vertices
                .start = original;
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
        eprintln!("dense native current registry point searches {observed:?}");
        assert!(
            observed.iter().all(|(_, searches)| *searches <= 1),
            "{observed:?}"
        );
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn current_witness_checks_exact_dependencies_and_expires_with_registry_borrow() {
        let plan = shared_native_material_plan(4);
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        backend
            .prepare_geometry_plan_for_extent(&plan, [64, 64], None)
            .unwrap();
        let template = backend
            .geometry
            .current_templates
            .iter()
            .find(|t| t.material_key.is_some())
            .unwrap()
            .clone();
        // Add a real registered dependency to the detached test template so the
        // negative case keeps an existing typed handle while changing only the
        // physical texture. The strict scalar oracle must accept the baseline.
        let texture_handle = RenderResourceHandle {
            kind: crate::render_resource::RenderResourceKind::Texture,
            index: 918,
            generation: 7,
        };
        backend
            .register_rgba8_texture(texture_handle, 1, 1, &[255; 4], "nearestClamp")
            .unwrap();
        let mut with_texture = template.as_ref().clone();
        with_texture.textures.push((
            texture_handle,
            backend.textures[&texture_handle]._view.texture().clone(),
        ));
        let template = Arc::new(with_texture);
        let draw = plan
            .commands
            .iter()
            .filter_map(geometry_draw)
            .find(|d| !d.is_opaque_shadow_depth_writer())
            .unwrap();
        let detached_texture = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("current-witness-detached-texture"),
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
        let mut changed = template.as_ref().clone();
        // Keep the identical group and typed handle, replace only the real GPU texture.
        changed.textures.last_mut().unwrap().1 = detached_texture;
        let changed = Arc::new(changed);
        assert!(changed.material == template.material);
        let mut current = Current::default();
        assert!(!current.check(&backend, &changed, draw));
        assert!(
            current.materials.is_empty(),
            "failed check installed a witness"
        );
        assert!(current.check(&backend, &template, draw));
        assert!(
            !current.check(&backend, &changed, draw),
            "shared group hid different texture backing"
        );
        let mut environment = template.as_ref().clone();
        environment.environment = Some(backend.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("current-witness-detached-environment"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        }));
        // The scalar oracle defines admission even for detached test-only
        // dependencies. An otherwise equal group cannot bypass its decision.
        let environment = Arc::new(environment);
        assert_eq!(
            current.check(&backend, &environment, draw),
            environment.current(&backend, draw)
        );
        drop(current);
        let mut changed_pipeline = template.as_ref().clone();
        changed_pipeline.pipeline = backend
            .geometry
            .pipelines
            .values()
            .find(|pipeline| *pipeline != &template.pipeline)
            .unwrap()
            .clone();
        let changed_pipeline = Arc::new(changed_pipeline);
        let mut current = Current::default();
        assert!(current.check(&backend, &template, draw));
        assert!(
            !current.check(&backend, &changed_pipeline, draw),
            "shared pipeline key hid a different native object"
        );
        drop(current);
        let pipeline = backend
            .geometry
            .pipelines
            .remove(template.pipeline_key.as_ref())
            .unwrap();
        assert!(
            !Current::default().check(&backend, &template, draw),
            "a previous call hid a missing current pipeline"
        );
        backend
            .geometry
            .pipelines
            .insert(template.pipeline_key.to_string(), pipeline);
        let mesh = backend.geometry.meshes.get_mut(&template.mesh).unwrap();
        let offset = mesh.vertices.start;
        mesh.vertices.start += u64::from(draw.geometry.vertex_stride());
        assert!(!Current::default().check(&backend, &template, draw));
        backend
            .geometry
            .meshes
            .get_mut(&template.mesh)
            .unwrap()
            .vertices
            .start = offset;
        let removed = backend
            .geometry
            .lit_bindings
            .remove(template.material_key.as_ref().unwrap())
            .unwrap();
        assert!(
            !Current::default().check(&backend, &template, draw),
            "old call proved missing current group"
        );
        backend
            .geometry
            .lit_bindings
            .insert(template.material_key.clone().unwrap(), removed);
        assert!(Current::default().check(&backend, &template, draw));
        let mesh = backend.geometry.meshes.remove(&template.mesh).unwrap();
        assert!(!Current::default().check(&backend, &template, draw));
        backend.geometry.meshes.insert(template.mesh, mesh);
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn shared_native_current_registry_queries_scale_with_current_resources() {
        let mut costs = Vec::new();
        for count in [4, 32, 256] {
            let plan = shared_native_material_plan(count);
            let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
            backend
                .prepare_geometry_plan_for_extent(&plan, [64, 64], None)
                .unwrap();
            let prepared = backend.geometry.validated_plan.as_ref().unwrap();
            let unique: std::collections::HashSet<_> = prepared
                .native_templates
                .iter()
                .flatten()
                .map(Arc::as_ptr)
                .collect();
            assert_eq!(
                unique.len(),
                plan.commands
                    .iter()
                    .filter(|c| geometry_draw(c).is_some())
                    .count(),
                "fixture did not have independent native templates"
            );
            assert!(prepared
                .native_templates
                .iter()
                .flatten()
                .any(|n| n.material_key.is_some()));
            let pipeline_count = prepared
                .native_templates
                .iter()
                .flatten()
                .map(|template| template.pipeline_key.as_ref())
                .collect::<std::collections::HashSet<_>>()
                .len();
            backend
                .geometry
                .validated_plan
                .as_mut()
                .unwrap()
                .command_backing = Weak::new();
            CURRENT_REGISTRY_QUERIES.with(|n| n.set([0; 3]));
            MATERIAL_CACHE_SEARCHES.with(|n| n.set(0));
            MATERIAL_VALUE_COMPARISONS.with(|n| n.set(0));
            backend
                .prepare_geometry_plan_for_extent(&plan, [64, 64], None)
                .unwrap();
            assert!(MATERIAL_CACHE_SEARCHES.with(|n| n.get()) <= 4,
                "consecutive exact material dependencies repeated {} hash searches at scale {count}",
                MATERIAL_CACHE_SEARCHES.with(|n| n.get()));
            assert!(
                MATERIAL_VALUE_COMPARISONS.with(|n| n.get()) <= 4,
                "one immutable material reinterpreted {} dependency rows at scale {count}",
                MATERIAL_VALUE_COMPARISONS.with(|n| n.get())
            );
            costs.push((
                count,
                CURRENT_REGISTRY_QUERIES.with(std::cell::Cell::get),
                pipeline_count,
            ));
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
        eprintln!("shared native actual mesh/material registry queries {costs:?}");
        assert!(
            costs.iter().all(|(_, queries, pipelines)| {
                queries[0] == 1 && queries[1] == 1 && queries[2] == *pipelines && *pipelines == 4
            }),
            "{costs:?}"
        );
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn retained_native_mixed_sprite_plan_reuses_templates_without_geometry_only_proof() {
        use crate::render_graph::*;
        let mut graph = crate::render_geometry::retained_command_test_graph();
        graph.passes.push(RenderPass {
            pass_id: "native-template-sprite".into(),
            pass_name: "native-template-sprite".into(),
            pass_kind: RenderPassKind::DrawSpriteTextured,
            view_id: "legacy".into(),
            reads: vec![],
            writes: vec!["surface-main".into()],
            color_targets: vec!["surface-main".into()],
            depth_target: None,
            commands: vec![RenderPassCommand::DrawSpriteTextured {
                target: "surface-main".into(),
                sprite_ref: "native-template-test".into(),
                material_ref: None,
                sort_key: "0".into(),
                texture: None,
                binding: None,
                fallback_used: false,
                vertices: [[-0.9, -0.2], [-0.5, -0.2], [-0.5, 0.2]]
                    .into_iter()
                    .map(|p| RenderDrawVertex::new(p, [1.; 4], [0.; 2]))
                    .collect(),
            }]
            .into(),
            debug_source: None,
        });
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        assert!(!RealWgpuBackend::geometry_command_shape(&plan));
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        backend
            .prepare_geometry_plan_for_extent(&plan, [64, 64], None)
            .unwrap();
        assert!(
            backend.geometry.validated_plan.is_none(),
            "mixed input gained geometry-only proof"
        );
        let start = plan
            .commands
            .iter()
            .position(|c| geometry_draw(c).is_some())
            .unwrap();
        let end = geometry_draw_end(&plan.commands, start);
        let first =
            encoding::Recipe::capture_commands(&backend, &plan.commands[start..end], 0).unwrap();
        OBJECT_RESOLUTIONS.with(|n| n.set(0));
        let second =
            encoding::Recipe::capture_commands(&backend, &plan.commands[start..end], 0).unwrap();
        assert_eq!(
            OBJECT_RESOLUTIONS.with(|n| n.get()),
            0,
            "mixed consumer repeated stable native object preparation"
        );
        assert!(first == second);
        drop(first);
        drop(second);
        OBJECT_RESOLUTIONS.with(|n| n.set(0));
        backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        assert_eq!(
            OBJECT_RESOLUTIONS.with(|n| n.get()),
            0,
            "mixed production frames lost the prepared native resource workset"
        );
        assert!(backend.geometry.validated_plan.is_none());
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn retained_native_preparation_resolves_unique_templates_and_rechecks_current_objects() {
        for count in [4, 32, 256] {
            let mut graph = crate::render_geometry::retained_command_test_graph();
            for pass in &mut graph.passes {
                let old = std::mem::take(&mut pass.commands);
                for command in old {
                    if let crate::render_graph::RenderPassCommand::DrawGeometryBatch { batch } =
                        command
                    {
                        let table: Arc<Vec<_>> = Arc::new(
                            (0..count)
                                .map(|i| {
                                    let mut instance = batch.draw.instance.as_ref().clone();
                                    instance.instance_id = i as u64 + 1;
                                    instance.world_transform[12] = (i as f32 * 0.001).into();
                                    Arc::new(instance)
                                })
                                .collect(),
                        );
                        for i in 0..count {
                            let mut draw = batch.draw.as_ref().clone();
                            draw.instance = table[i].clone();
                            pass.commands.push(
                                crate::render_graph::RenderPassCommand::DrawGeometryBatch {
                                    batch: Arc::new(crate::render_geometry::GeometryBatch::new(
                                        Arc::new(draw),
                                        table.clone(),
                                        vec![crate::render_geometry::GeometryVisibleInstance {
                                            instance_index: i as u32,
                                            camera_depth: batch.draw.camera_depth,
                                        }],
                                    )),
                                },
                            );
                        }
                    } else {
                        pass.commands.push(command);
                    }
                }
            }
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
            OBJECT_RESOLUTIONS.with(|n| n.set(0));
            backend
                .prepare_geometry_plan_for_extent(&plan, [64, 64], None)
                .unwrap();
            let prepared = backend.geometry.validated_plan.as_ref().unwrap();
            let unique: std::collections::HashSet<_> = prepared
                .native_templates
                .iter()
                .flatten()
                .map(Arc::as_ptr)
                .collect();
            let built = OBJECT_RESOLUTIONS.with(|n| n.get());
            assert_eq!(
                built,
                unique.len(),
                "scale {count}: native objects reinterpreted per batch"
            );
            assert!(
                built < count,
                "fixture did not share stable production commands"
            );
            let start = plan
                .commands
                .iter()
                .position(|c| geometry_draw(c).is_some())
                .unwrap();
            let end = geometry_draw_end(&plan.commands, start);
            encoding::STABLE_DESCRIPTOR_CHECKS.with(|n| n.set(0));
            let _recipe =
                encoding::Recipe::capture_commands(&backend, &plan.commands[start..end], 0)
                    .unwrap();
            assert_eq!(encoding::STABLE_DESCRIPTOR_CHECKS.with(|n| n.get()), 0, "scale {count}: shared native command fell back to stable descriptor interpretation");
            drop(_recipe);
            let first = prepared
                .native_templates
                .iter()
                .flatten()
                .next()
                .unwrap()
                .clone();
            let weak = Arc::downgrade(&first);
            // Invalidate only the whole-plan proof. Stable per-template native
            // work must still survive the next successful preparation.
            backend
                .geometry
                .validated_plan
                .as_mut()
                .unwrap()
                .command_backing = Weak::new();
            OBJECT_RESOLUTIONS.with(|n| n.set(0));
            CURRENT_CHECKS.with(|n| n.set(0));
            backend
                .prepare_geometry_plan_for_extent(&plan, [64, 64], None)
                .unwrap();
            assert_eq!(OBJECT_RESOLUTIONS.with(|n| n.get()), 0, "scale {count}");
            assert_eq!(
                CURRENT_CHECKS.with(|n| n.get()),
                unique.len(),
                "scale {count}: current native checks repeated per batch"
            );
            eprintln!("native template scale={count}, first resolutions={built}, reprepare resolutions=0, current checks={}", unique.len());
            let draw = plan.commands.iter().find_map(geometry_draw).unwrap();
            assert!(first.current(&backend, draw));
            {
                let facts = &backend
                    .geometry
                    .validated_plan
                    .as_ref()
                    .unwrap()
                    .requirements;
                backend.geometry.native_templates.prepare.set(u64::MAX);
                CURRENT_CHECKS.with(|n| n.set(0));
                let (aligned, retained) = backend
                    .geometry
                    .native_templates
                    .prepare(&backend, &plan, facts);
                assert_eq!(backend.geometry.native_templates.prepare.get(), 1);
                assert_eq!(CURRENT_CHECKS.with(|n| n.get()), unique.len());
                assert_eq!(retained.len(), unique.len());
                assert!(aligned.iter().flatten().any(|t| Arc::ptr_eq(t, &first)));

                let overflow = Templates {
                    limit: 0,
                    ..Templates::default()
                };
                let (aligned, retained) = overflow.prepare(&backend, &plan, facts);
                assert!(overflow.entries.borrow().is_empty());
                assert_eq!(retained.len(), unique.len());
                let pointers: std::collections::HashSet<_> =
                    aligned.iter().flatten().map(Arc::as_ptr).collect();
                assert_eq!(
                    pointers.len(),
                    unique.len(),
                    "overflow lost same-call sharing"
                );
            }
            // A pooled allocation may reuse the same buffer but move its range.
            let mesh = backend.geometry.meshes.get_mut(&first.mesh).unwrap();
            let previous = mesh.vertices.start;
            mesh.vertices.start += u64::from(draw.geometry.vertex_stride());
            assert!(
                !first.current(&backend, draw),
                "same buffer hid a moved page range"
            );
            backend
                .geometry
                .meshes
                .get_mut(&first.mesh)
                .unwrap()
                .vertices
                .start = previous;
            drop(first);
            backend.geometry.validated_plan = None;
            backend.geometry.current_templates.clear();
            assert!(
                weak.upgrade().is_none(),
                "weak native memo pinned a dead plan's objects"
            );
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
    }
}

#[cfg(test)]
thread_local! {
    static PIPELINE_CACHE_SEARCHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static MATERIAL_CACHE_SEARCHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PREPARE_SCRATCH_INSERTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static CURRENT_REGISTRY_QUERIES: std::cell::Cell<[usize;3]> = const {std::cell::Cell::new([0;3])};
    pub(super) static OBJECT_RESOLUTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static CURRENT_CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(super) fn shared_native_material_plan(count: usize) -> RhiCommandPlan {
    let mut graph = crate::render_geometry::retained_command_test_graph();
    for pass in &mut graph.passes {
        for command in std::mem::take(&mut pass.commands) {
            let head = match command {
                crate::render_graph::RenderPassCommand::DrawGeometryBatch { batch } => {
                    batch.draw.clone()
                }
                crate::render_graph::RenderPassCommand::DrawGeometry { draw } => draw,
                other => {
                    pass.commands.push(other);
                    continue;
                }
            };
            for i in 0..count {
                let mut draw = head.as_ref().clone();
                draw.instance_id = i as u64 + 1;
                draw.cull = if i % 2 == 0 {
                    MaterialCull::Back
                } else {
                    MaterialCull::Front
                };
                pass.commands
                    .push(crate::render_graph::RenderPassCommand::DrawGeometry {
                        draw: Arc::new(draw),
                    });
            }
        }
    }
    let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    plan
}

#[cfg(test)]
thread_local! {pub(super) static BACKING_DEPENDENCY_VISITS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
mod backing_work_tests {
    use super::*;
    #[test]
    fn completed_recordings_cover_current_template_backing_without_rewalking_rows() {
        for count in [4, 32, 256] {
            let graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            let output = backend.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("recorded-backing-coverage"),
                size: wgpu::Extent3d {
                    width: 640,
                    height: 480,
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
            let report = backend.execute_plan_to_surface_view(&plan, &view);
            assert!(report.diagnostics.is_empty(), "{report:?}");
            assert!(!backend
                .geometry
                .validated_plan
                .as_ref()
                .unwrap()
                .bundles
                .is_empty());
            assert!(!backend.geometry.current_templates.is_empty());
            // Admission can initialize these lazy locations before the explicit
            // measurement. Reproduce the cold observation state without changing
            // any native dependency or recording.
            backend
                .geometry
                .current_templates
                .discard_observation_locations();
            backend
                .geometry
                .validated_plan
                .as_mut()
                .unwrap()
                .native_templates
                .discard_observation_locations();
            let mut oracle = super::super::super::memory::GpuMemoryCensus::default();
            let mut trace = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
            backend.geometry.observe_memory(&mut oracle, &mut trace);
            let expected = oracle.requested_bytes();
            assert!(!trace.allocations.is_empty());
            let mut physical = super::super::super::memory::GpuMemoryCensus::backing_only();
            let mut sample = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
            BACKING_DEPENDENCY_VISITS.with(|n| n.set(0));
            backend.geometry.observe_memory(&mut physical, &mut sample);
            assert_eq!(physical.requested_bytes(), expected);
            assert!(sample.allocations.is_empty());
            assert_eq!(
                BACKING_DEPENDENCY_VISITS.with(|n| n.get()),
                0,
                "completed native recordings still rescan covered template rows at scale {count}"
            );
            drop(physical);
            drop(oracle);
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
    }
    #[test]
    fn incomplete_recordings_and_detached_inputs_keep_exact_full_backing_census() {
        for state in [
            "unexpectedKey",
            "notReady",
            "missing",
            "currentReplaced",
            "alignedReplaced",
            "preparedOnly",
        ] {
            let graph = crate::rhi_command_plan::tests::retained_geometry_graph(256);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            let output = backend.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("partial-recording-coverage"),
                size: wgpu::Extent3d {
                    width: 640,
                    height: 480,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            if state == "preparedOnly" {
                backend
                    .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
                    .unwrap();
            } else {
                let report = backend
                    .execute_plan_to_surface_view(&plan, &output.create_view(&Default::default()));
                assert!(report.diagnostics.is_empty(), "{report:?}");
                assert!(
                    backend
                        .geometry
                        .validated_plan
                        .as_ref()
                        .unwrap()
                        .bundles
                        .len()
                        >= 2
                );
            }
            let capture = |backend: &RealWgpuBackend| {
                let mut gpu = super::super::super::memory::GpuMemoryCensus::default();
                backend.geometry.observe_memory(
                    &mut gpu,
                    &mut crate::resource_memory::ResourceMemorySample::new(0, 0, 0),
                );
                gpu.requested_bytes()
            };
            let baseline = capture(&backend);
            match state {
                "unexpectedKey" => {
                    // A provenance split can create a different real range key.
                    // Equal count and ready bits do not certify the original set.
                    let bundles = &mut backend.geometry.validated_plan.as_mut().unwrap().bundles;
                    let original_count = bundles.len();
                    let (_, recorded) = bundles.pop_first().unwrap();
                    assert!(bundles.insert(0x7fff_ffff, recorded).is_none());
                    assert_eq!(bundles.len(), original_count);
                    assert!(bundles.values().all(|recorded| recorded.ready));
                }
                "notReady" => {
                    backend
                        .geometry
                        .validated_plan
                        .as_mut()
                        .unwrap()
                        .bundles
                        .values_mut()
                        .next()
                        .unwrap()
                        .ready = false
                }
                "missing" => {
                    backend
                        .geometry
                        .validated_plan
                        .as_mut()
                        .unwrap()
                        .bundles
                        .pop_first();
                }
                "currentReplaced" | "alignedReplaced" => {
                    let mut changed = backend.geometry.current_templates[0].as_ref().clone();
                    assert!(changed.environment.is_none());
                    changed.environment =
                        Some(backend.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("detached-recording-input-buffer"),
                            size: 64,
                            usage: wgpu::BufferUsages::UNIFORM,
                            mapped_at_creation: false,
                        }));
                    let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("detached-recording-input-texture"),
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
                    changed.textures.push((
                        RenderResourceHandle {
                            kind: crate::render_resource::RenderResourceKind::Texture,
                            index: 991,
                            generation: 3,
                        },
                        texture,
                    ));
                    if state == "currentReplaced" {
                        backend.geometry.current_templates = vec![Arc::new(changed)].into();
                    } else {
                        backend
                            .geometry
                            .validated_plan
                            .as_mut()
                            .unwrap()
                            .native_templates = vec![Some(Arc::new(changed))].into();
                    }
                    assert_eq!(capture(&backend), baseline + 68, "{state}");
                }
                "preparedOnly" => {}
                _ => unreachable!(),
            }
            backend
                .geometry
                .current_templates
                .discard_observation_locations();
            backend
                .geometry
                .validated_plan
                .as_mut()
                .unwrap()
                .native_templates
                .discard_observation_locations();
            let expected = capture(&backend);
            let mut gpu = super::super::super::memory::GpuMemoryCensus::backing_only();
            let mut sample = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
            BACKING_DEPENDENCY_VISITS.with(|n| n.set(0));
            backend.geometry.observe_memory(&mut gpu, &mut sample);
            assert_eq!(gpu.requested_bytes(), expected, "{state}");
            assert!(
                BACKING_DEPENDENCY_VISITS.with(|n| n.get()) > 0,
                "{state}: full fallback was bypassed"
            );
            assert!(sample.allocations.is_empty());
            drop(gpu);
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
    }
    #[test]
    fn unchanged_template_collection_reuses_physical_dependency_locations() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        backend
            .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
            .unwrap();
        BACKING_DEPENDENCY_VISITS.with(|n| n.set(0));
        let baseline = backend.geometry_backing_bytes();
        let first = BACKING_DEPENDENCY_VISITS.with(|n| n.get());
        assert!(first > 0);
        assert_eq!(backend.geometry_backing_bytes(), baseline);
        assert_eq!(
            BACKING_DEPENDENCY_VISITS.with(|n| n.get()),
            first,
            "unchanged immutable template collection walked all dependencies again"
        );
        let mut previous = std::mem::take(&mut backend.geometry.current_templates);
        backend.geometry.current_templates = template_backing::TemplateCollection::reuse(
            previous.iter().cloned().collect(),
            &mut previous,
        );
        assert_eq!(backend.geometry_backing_bytes(), baseline);
        assert_eq!(BACKING_DEPENDENCY_VISITS.with(|n| n.get()), first);
        drop(previous);
        let template = backend.geometry.current_templates[0].clone();
        backend.geometry.current_templates = (0..256)
            .map(|_| Arc::new(template.as_ref().clone()))
            .collect::<Vec<_>>()
            .into();
        template_backing::RESOURCE_QUERIES.with(|n| n.set(0));
        assert_eq!(backend.geometry_backing_bytes(), baseline);
        assert!(
            template_backing::RESOURCE_QUERIES.with(|n| n.get()) <= 3,
            "identical adjacent native resources rehashed per template"
        );
        let mut changed = backend
            .geometry
            .current_templates
            .first()
            .unwrap()
            .as_ref()
            .clone();
        changed.environment = Some(backend.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("changed-collection-dependency"),
            size: 64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        }));
        backend.geometry.current_templates = vec![Arc::new(changed)].into();
        assert_eq!(backend.geometry_backing_bytes(), baseline + 64);
        assert_eq!(backend.geometry_backing_bytes(), baseline + 64);
        backend.geometry.current_templates.clear();
        assert_eq!(backend.geometry_backing_bytes(), baseline);
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
    #[test]
    fn gpu_backing_census_visits_shared_native_template_once_and_detached_change_is_not_hidden() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        backend
            .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
            .unwrap();
        let template = backend.geometry.current_templates.first().unwrap().clone();
        let extra = backend.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("detached-template-extra"),
            size: 64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let mut changed = template.as_ref().clone();
        assert!(changed.environment.is_none());
        changed.environment = Some(extra);
        let mut observed = Vec::new();
        for count in [4, 32, 256] {
            let aliases: Vec<_> = (0..count).map(|_| template.clone()).collect();
            let mut census = super::super::super::memory::GpuMemoryCensus::backing_only();
            let mut sample = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
            BACKING_DEPENDENCY_VISITS.with(|n| n.set(0));
            for t in &aliases {
                t.observe_gpu(&mut census, &mut sample, "resident");
            }
            let bytes = census.requested_bytes();
            let shared = BACKING_DEPENDENCY_VISITS.with(|n| n.get());
            changed.observe_gpu(&mut census, &mut sample, "retiring");
            assert_eq!(
                census.requested_bytes(),
                bytes + 64,
                "same-value/native dependency alias hid detached backing"
            );
            assert!(sample.allocations.is_empty());
            observed.push((count, shared, BACKING_DEPENDENCY_VISITS.with(|n| n.get())));
        }
        eprintln!("actual template dependency walks {observed:?}");
        assert!(
            observed
                .iter()
                .all(|(_, shared, total)| *shared == 1 && *total == 2),
            "repeated template walks {observed:?}"
        );
        drop(changed);
        drop(template);
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
}
