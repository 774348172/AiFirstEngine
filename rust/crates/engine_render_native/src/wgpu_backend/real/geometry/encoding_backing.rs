//! Physical dependency locations of one immutable recording. No GPU handles,
//! residency proof, cross-recipe cache or lifetime owner is stored here.
use super::*;
use std::sync::OnceLock;
#[cfg(not(target_arch = "wasm32"))]
use wgpu::naga::FastHashSet;
#[cfg(target_arch = "wasm32")]
type FastHashSet<T> = std::collections::HashSet<T>;

#[derive(Default)]
pub(super) struct RecipeBacking(OnceLock<Locations>);
impl Clone for RecipeBacking {
    fn clone(&self) -> Self {
        // A detached recipe may change its batch/native dependencies before
        // publication. Its old allocation's locations are never inherited.
        Self::default()
    }
}

enum BufferLocation {
    Parameters,
    View,
    Index,
    Extra(usize),
    Vertices(usize),
    Indices(usize),
    Environment(usize),
}
enum TextureLocation {
    Extra(usize),
    Native(usize, usize),
}
struct Locations {
    buffers: Vec<BufferLocation>,
    textures: Vec<TextureLocation>,
}
impl BufferLocation {
    fn get<'a>(&self, recipe: &'a Recipe) -> &'a wgpu::Buffer {
        match *self {
            Self::Parameters => &recipe.parameter_buffer,
            Self::View => &recipe.view_buffer,
            Self::Index => &recipe.index_buffer,
            Self::Extra(i) => &recipe.extra_buffers[i],
            Self::Vertices(i) => &recipe.batches[i].vertices,
            Self::Indices(i) => &recipe.batches[i].indices,
            Self::Environment(i) => recipe.batches[i].environment.as_ref().unwrap(),
        }
    }
}
impl TextureLocation {
    fn get<'a>(&self, recipe: &'a Recipe) -> &'a wgpu::Texture {
        match *self {
            Self::Extra(i) => &recipe.textures[i],
            Self::Native(batch, i) => &recipe.batches[batch].native.textures[i].1,
        }
    }
}
impl Locations {
    fn capture(recipe: &Recipe) -> Self {
        let mut buffers = Vec::new();
        let mut textures = Vec::new();
        let mut seen_buffers = FastHashSet::default();
        let mut seen_textures = FastHashSet::default();
        let mut buffer = |location: BufferLocation| {
            if seen_buffers.insert(location.get(recipe)) {
                buffers.push(location);
            }
        };
        let mut texture = |location: TextureLocation| {
            if seen_textures.insert(location.get(recipe)) {
                textures.push(location);
            }
        };
        buffer(BufferLocation::Parameters);
        buffer(BufferLocation::View);
        buffer(BufferLocation::Index);
        for i in 0..recipe.extra_buffers.len() {
            buffer(BufferLocation::Extra(i));
        }
        for (i, batch) in recipe.batches.iter().enumerate() {
            #[cfg(test)]
            RECIPE_BACKING_BATCH_VISITS.with(|n| n.set(n.get() + 1));
            buffer(BufferLocation::Vertices(i));
            buffer(BufferLocation::Indices(i));
            if batch.environment.is_some() {
                buffer(BufferLocation::Environment(i));
            }
            for t in 0..batch.native.textures.len() {
                texture(TextureLocation::Native(i, t));
            }
        }
        for i in 0..recipe.textures.len() {
            texture(TextureLocation::Extra(i));
        }
        Self { buffers, textures }
    }
}
impl RecipeBacking {
    pub(super) fn observe<'a>(
        &self,
        recipe: &'a Recipe,
        gpu: &mut GpuMemoryCensus<'a>,
        role: &str,
    ) {
        let locations = self.0.get_or_init(|| Locations::capture(recipe));
        for location in &locations.buffers {
            gpu.buffer(location.get(recipe), "renderer", &[role]);
        }
        for location in &locations.textures {
            gpu.texture(location.get(recipe), &[role]);
        }
    }
    pub(super) fn observe_memory(&self, sample: &mut ResourceMemorySample, role: &str) {
        if let Some(locations) = self.0.get() {
            sample.cpu_vec(&locations.buffers, "renderer", &[role]);
            sample.cpu_vec(&locations.textures, "renderer", &[role]);
        }
    }
}
