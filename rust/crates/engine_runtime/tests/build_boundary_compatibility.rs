//! The facade must export the original types, not wire-compatible copies.
use engine_runtime::{canonical_digest, font_bundle, render_resource, runtime_texture};

#[test]
fn rendering_facade_exports_the_actual_core_and_native_types() {
    fn world(value: engine_runtime::world::World) -> engine_runtime_core::world::World {
        value
    }
    fn graph(
        value: engine_runtime::render_graph::RenderGraph,
    ) -> engine_runtime_core::render_graph::RenderGraph {
        value
    }
    fn backend(
        value: engine_runtime::wgpu_backend::WgpuBackend,
    ) -> engine_render_native::wgpu_backend::WgpuBackend {
        value
    }
    fn domain(
        value: engine_runtime::domain::render::wgpu_backend::WgpuBackend,
    ) -> engine_runtime::wgpu_backend::WgpuBackend {
        value
    }
    let _ = world(engine_runtime::world::World::new());
    let _ = graph(engine_runtime::render_graph::RenderGraph::new(
        "identity", 1,
    ));
    let _ = backend(domain(
        engine_runtime::wgpu_backend::WgpuBackend::new_unavailable(),
    ));
    #[cfg(feature = "real-wgpu")]
    {
        fn native(
            value: engine_runtime::wgpu_backend::real::RealWgpuBackend,
        ) -> engine_render_native::wgpu_backend::real::RealWgpuBackend {
            value
        }
        let _: fn(
            engine_runtime::wgpu_backend::real::RealWgpuBackend,
        ) -> engine_render_native::wgpu_backend::real::RealWgpuBackend = native;
    }
}

#[test]
fn moved_owners_keep_type_identity_and_legacy_paths() {
    fn resource_identity(
        manager: render_resource::RenderResourceManager,
    ) -> engine_render_resources::render_resource::RenderResourceManager {
        manager
    }
    fn font_identity(
        registry: font_bundle::RuntimeFontBundleRegistry,
    ) -> engine_text::font_bundle::RuntimeFontBundleRegistry {
        registry
    }
    let _: engine_render_resources::render_resource::RenderResourceManager =
        resource_identity(render_resource::RenderResourceManager::new());
    let _ = font_identity(font_bundle::RuntimeFontBundleRegistry::default());
    let sampler = runtime_texture::TextureSampler::parse("linearClamp").unwrap();
    let moved: engine_render_resources::texture_sampler::TextureSampler = sampler;
    assert_eq!(moved.key(), sampler.key());
    let levels: Vec<engine_render_resources::texture_mips::TextureMipLevel> =
        runtime_texture::rgba8_mip_layout(4, 4, 3).unwrap();
    assert_eq!(levels.last().unwrap().byte_len, 4);
    let legacy_hash: fn(&[u8]) -> String = canonical_digest::sha256_prefixed;
    assert_eq!(
        legacy_hash(b"same implementation"),
        engine_content::canonical_digest::sha256_prefixed(b"same implementation")
    );
}
