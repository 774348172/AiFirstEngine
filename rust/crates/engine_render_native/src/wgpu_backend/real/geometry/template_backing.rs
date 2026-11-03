//! Physical dependency locations bound to one immutable template collection.
//! Locations borrow the collection at observation; they own no native handles.
use super::{native_template::Template, *};
use std::sync::OnceLock;
#[cfg(test)]
thread_local! {pub(super) static RESOURCE_QUERIES: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}

pub(super) trait TemplateEntry {
    fn template(&self) -> Option<&Arc<Template>>;
}
impl TemplateEntry for Arc<Template> {
    fn template(&self) -> Option<&Arc<Template>> {
        Some(self)
    }
}
impl TemplateEntry for Option<Arc<Template>> {
    fn template(&self) -> Option<&Arc<Template>> {
        self.as_ref()
    }
}

pub(super) struct TemplateCollection<T> {
    entries: Vec<T>,
    backing: OnceLock<Locations>,
    recording_input: bool,
}
impl<T> Default for TemplateCollection<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            backing: OnceLock::new(),
            recording_input: false,
        }
    }
}
impl<T> From<Vec<T>> for TemplateCollection<T> {
    fn from(entries: Vec<T>) -> Self {
        Self {
            entries,
            backing: OnceLock::new(),
            recording_input: false,
        }
    }
}
impl<T> std::ops::Deref for TemplateCollection<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}
impl<'a, T> IntoIterator for &'a TemplateCollection<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}
impl<T> TemplateCollection<T> {
    // Published only by the successful geometry-plan owner, together with its
    // aligned collection. A detached/replaced collection is never certified.
    pub(super) fn mark_recording_input(&mut self) {
        self.recording_input = true;
    }
    pub(super) fn is_recording_input(&self) -> bool {
        self.recording_input
    }
    #[cfg(test)]
    pub(super) fn discard_observation_locations(&mut self) {
        self.backing = OnceLock::new();
    }
    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.backing = OnceLock::new();
        self.recording_input = false;
    }
}
impl<T: TemplateEntry> TemplateCollection<T> {
    pub(super) fn reuse(entries: Vec<T>, previous: &mut Self) -> Self {
        let same = entries.len() == previous.len()
            && entries.iter().zip(&previous.entries).all(|(a, b)| {
                match (a.template(), b.template()) {
                    (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                    (None, None) => true,
                    _ => false,
                }
            });
        Self {
            entries,
            backing: if same {
                std::mem::take(&mut previous.backing)
            } else {
                OnceLock::new()
            },
            recording_input: false,
        }
    }
    pub(super) fn observe_gpu<'a>(
        &'a self,
        gpu: &mut super::super::memory::GpuMemoryCensus<'a>,
        sample: &mut crate::resource_memory::ResourceMemorySample,
        role: &str,
    ) {
        if !gpu.is_backing_only() {
            for template in self.entries.iter().filter_map(TemplateEntry::template) {
                template.observe_gpu(gpu, sample, role);
            }
            if let Some(locations) = self.backing.get() {
                sample.cpu_vec(&locations.buffers, "renderer", &[role]);
                sample.cpu_vec(&locations.textures, "renderer", &[role]);
            }
            return;
        }
        let locations = self
            .backing
            .get_or_init(|| Locations::capture(&self.entries));
        for (index, field) in &locations.buffers {
            let template = self.entries[*index].template().unwrap();
            gpu.buffer(field.get(template), "renderer", &[role]);
        }
        for (index, texture) in &locations.textures {
            gpu.texture(
                &self.entries[*index].template().unwrap().textures[*texture].1,
                &[role],
            );
        }
    }
}
#[derive(Clone, Copy)]
enum BufferField {
    Vertices,
    Indices,
    Environment,
}
impl BufferField {
    fn get<'a>(&self, template: &'a Template) -> &'a wgpu::Buffer {
        match self {
            Self::Vertices => &template.vertices,
            Self::Indices => &template.indices,
            Self::Environment => template.environment.as_ref().unwrap(),
        }
    }
}
struct Locations {
    buffers: Vec<(usize, BufferField)>,
    textures: Vec<(usize, usize)>,
}
impl Locations {
    // These borrowed native handles hash by immutable object identity; their
    // interior mapping/content state never changes equality during this borrow.
    #[allow(clippy::mutable_key_type)]
    fn capture<T: TemplateEntry>(entries: &[T]) -> Self {
        let mut buffers = Vec::new();
        let mut textures = Vec::new();
        let mut seen_buffers = super::super::memory::FastHashSet::default();
        let mut seen_textures = super::super::memory::FastHashSet::default();
        let mut last_buffers = [None; 3];
        let mut last_texture = None;
        for (index, entry) in entries.iter().enumerate() {
            let Some(template) = entry.template() else {
                continue;
            };
            #[cfg(test)]
            super::native_template::BACKING_DEPENDENCY_VISITS.with(|n| n.set(n.get() + 1));
            for field in [
                BufferField::Vertices,
                BufferField::Indices,
                BufferField::Environment,
            ] {
                if matches!(field, BufferField::Environment) && template.environment.is_none() {
                    continue;
                }
                let slot = match field {
                    BufferField::Vertices => 0,
                    BufferField::Indices => 1,
                    BufferField::Environment => 2,
                };
                let object = field.get(template);
                if last_buffers[slot] == Some(object) {
                    continue;
                }
                last_buffers[slot] = Some(object);
                #[cfg(test)]
                RESOURCE_QUERIES.with(|n| n.set(n.get() + 1));
                if seen_buffers.insert(object) {
                    buffers.push((index, field));
                }
            }
            for (texture, (_, object)) in template.textures.iter().enumerate() {
                if last_texture == Some(object) {
                    continue;
                }
                last_texture = Some(object);
                #[cfg(test)]
                RESOURCE_QUERIES.with(|n| n.set(n.get() + 1));
                if seen_textures.insert(object) {
                    textures.push((index, texture));
                }
            }
        }
        Self { buffers, textures }
    }
}
