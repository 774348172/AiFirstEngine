use crate::ids::SourceEntityId;
use crate::render_state::{
    RenderPayloadKind, RenderProxyId, RenderProxyPayload, RenderSceneState, RenderViewId,
};
use crate::{components::Transform, render_state::RenderViewKind};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RendererFeatureCounters {
    pub view_count: usize,
    pub draw_item_count: usize,
    pub skipped_invisible_count: usize,
    pub warning_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RendererFeatureFrame {
    pub frame_index: u64,
    pub views: Vec<RendererFeatureView>,
    pub draw_items: Vec<RendererFeatureDrawItem>,
    pub diagnostics: Vec<RendererFeatureDiagnostic>,
    pub counters: RendererFeatureCounters,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererFeatureView {
    pub view_id: RenderViewId,
    pub view_kind: RenderViewKind,
    pub visible_proxy_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RendererFeatureDrawItem {
    pub proxy_id: RenderProxyId,
    pub source_entity_id: SourceEntityId,
    pub payload_kind: RenderPayloadKind,
    pub mesh_ref: Option<String>,
    pub sprite_ref: Option<String>,
    pub material_ref: Option<String>,
    pub color: [f32; 4],
    pub flip_x: bool,
    pub flip_y: bool,
    pub transform: Transform,
    pub visible: bool,
    pub layer: String,
    pub sorting_layer: i16,
    pub order_in_layer: i32,
    pub sort_z: f32,
    pub sort_key: DrawItemSortKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DrawItemSortKey {
    pub render_domain_order: u8,
    pub sorting_layer: i16,
    pub order_in_layer: i32,
    pub sort_z_quantized: i32,
    pub stable_proxy_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererFeatureDiagnostic {
    pub severity: RendererFeatureSeverity,
    pub code: &'static str,
    pub proxy_id: Option<RenderProxyId>,
    pub source_entity_id: Option<SourceEntityId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererFeatureSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Default)]
pub struct RendererFeatureBuilder;

impl RendererFeatureBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build(&self, frame_index: u64, scene: &RenderSceneState) -> RendererFeatureFrame {
        let mut views = Vec::new();
        for view in scene.views() {
            let visible_proxy_count = scene
                .build_frame_view_data(view.view_id)
                .map(|data| data.visible_proxy_ids.len())
                .unwrap_or_default();
            views.push(RendererFeatureView {
                view_id: view.view_id,
                view_kind: view.view_kind.clone(),
                visible_proxy_count,
            });
        }

        let mut draw_items = Vec::new();
        let mut diagnostics = Vec::new();
        let mut skipped_invisible_count = 0;
        for proxy in scene.proxies() {
            if !proxy.common.enabled || !proxy.common.visible {
                skipped_invisible_count += 1;
                diagnostics.push(RendererFeatureDiagnostic {
                    severity: RendererFeatureSeverity::Info,
                    code: "proxy_not_visible",
                    proxy_id: Some(proxy.common.proxy_id),
                    source_entity_id: Some(proxy.common.source_entity_id.clone()),
                });
                continue;
            }

            let draw_refs = draw_item_refs(&proxy.payload);
            draw_items.push(RendererFeatureDrawItem {
                proxy_id: proxy.common.proxy_id,
                source_entity_id: proxy.common.source_entity_id.clone(),
                payload_kind: proxy.payload.kind(),
                mesh_ref: draw_refs.mesh_ref,
                sprite_ref: draw_refs.sprite_ref,
                material_ref: draw_refs.material_ref,
                color: draw_refs.color,
                flip_x: draw_refs.flip_x,
                flip_y: draw_refs.flip_y,
                transform: proxy.common.transform.clone(),
                visible: proxy.common.visible,
                layer: proxy.common.layer.clone(),
                sorting_layer: draw_refs.sorting_layer,
                order_in_layer: draw_refs.order_in_layer,
                sort_z: draw_refs.sort_z,
                sort_key: draw_item_sort_key(
                    &proxy.payload,
                    proxy.common.proxy_id,
                    proxy.common.transform.local_position.z,
                ),
            });
        }
        draw_items.sort_by_key(|item| item.sort_key);

        let counters = RendererFeatureCounters {
            view_count: views.len(),
            draw_item_count: draw_items.len(),
            skipped_invisible_count,
            warning_count: diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == RendererFeatureSeverity::Warning)
                .count(),
        };

        RendererFeatureFrame {
            frame_index,
            views,
            draw_items,
            diagnostics,
            counters,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct DrawItemRefs {
    mesh_ref: Option<String>,
    sprite_ref: Option<String>,
    material_ref: Option<String>,
    color: [f32; 4],
    flip_x: bool,
    flip_y: bool,
    sorting_layer: i16,
    order_in_layer: i32,
    sort_z: f32,
}

fn draw_item_refs(payload: &RenderProxyPayload) -> DrawItemRefs {
    match payload {
        RenderProxyPayload::Mesh(payload) => DrawItemRefs {
            mesh_ref: payload.mesh_ref.clone(),
            sprite_ref: None,
            material_ref: payload.material_ref.clone(),
            color: [1.0, 1.0, 1.0, 1.0],
            flip_x: false,
            flip_y: false,
            sorting_layer: 0,
            order_in_layer: 0,
            sort_z: 0.0,
        },
        RenderProxyPayload::Sprite(payload) => DrawItemRefs {
            mesh_ref: None,
            sprite_ref: payload.sprite_ref.clone(),
            material_ref: payload.material_ref.clone(),
            color: payload.color,
            flip_x: payload.flip_x,
            flip_y: payload.flip_y,
            sorting_layer: payload.sorting_layer,
            order_in_layer: payload.order_in_layer,
            sort_z: payload.sort_z,
        },
        RenderProxyPayload::SkinnedMesh(payload) => DrawItemRefs {
            mesh_ref: payload.mesh_ref.clone(),
            sprite_ref: None,
            material_ref: payload.material_ref.clone(),
            color: [1.0, 1.0, 1.0, 1.0],
            flip_x: false,
            flip_y: false,
            sorting_layer: 0,
            order_in_layer: 0,
            sort_z: 0.0,
        },
        RenderProxyPayload::Light(_)
        | RenderProxyPayload::Camera(_)
        | RenderProxyPayload::Particle(_)
        | RenderProxyPayload::Instance(_) => DrawItemRefs {
            mesh_ref: None,
            sprite_ref: None,
            material_ref: None,
            color: [1.0, 1.0, 1.0, 1.0],
            flip_x: false,
            flip_y: false,
            sorting_layer: 0,
            order_in_layer: 0,
            sort_z: 0.0,
        },
    }
}

fn draw_item_sort_key(
    payload: &RenderProxyPayload,
    proxy_id: RenderProxyId,
    transform_z: f32,
) -> DrawItemSortKey {
    let (render_domain_order, sorting_layer, order_in_layer, sort_z) = match payload {
        RenderProxyPayload::Mesh(_) => (10, 0, 0, 0.0),
        RenderProxyPayload::Sprite(payload) => (
            20,
            payload.sorting_layer,
            payload.order_in_layer,
            if payload.sort_z == 0.0 {
                transform_z
            } else {
                payload.sort_z
            },
        ),
        RenderProxyPayload::SkinnedMesh(_) => (30, 0, 0, 0.0),
        RenderProxyPayload::Particle(_) => (40, 0, 0, 0.0),
        RenderProxyPayload::Light(_)
        | RenderProxyPayload::Camera(_)
        | RenderProxyPayload::Instance(_) => (80, 0, 0, 0.0),
    };

    DrawItemSortKey {
        render_domain_order,
        sorting_layer,
        order_in_layer,
        sort_z_quantized: quantize_sort_z(sort_z),
        stable_proxy_id: proxy_id.0,
    }
}

fn quantize_sort_z(sort_z: f32) -> i32 {
    (sort_z * 1000.0).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Renderable, Transform};
    use crate::ids::{RuntimeEntityId, SourceEntityId};
    use crate::math::Vec3;
    use crate::render_state::{
        RenderProxy, RenderProxyId, RenderProxyPayload, RenderTargetKind, RenderViewKind,
        RenderViewState, SpritePayload,
    };

    fn transform(x: f32) -> Transform {
        Transform {
            local_position: Vec3 { x, y: 0.0, z: 0.0 },
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        }
    }

    fn renderable(mesh: Option<&str>) -> Renderable {
        Renderable {
            mesh_ref: mesh.map(str::to_string),
            material_ref: Some("material-a".to_string()),
            visible: true,
            layer: "default".to_string(),
        }
    }

    fn scene_with_proxy(visible: bool) -> RenderSceneState {
        let mut scene = RenderSceneState::new();
        scene.register_view(RenderViewState::new(
            crate::render_state::RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::Window,
        ));
        let mut proxy = RenderProxy::new(
            RenderProxyId(0),
            RuntimeEntityId::new(1, 0),
            SourceEntityId::from("entity-a"),
            transform(1.0),
            renderable(Some("mesh-a")),
        );
        proxy.common.visible = visible;
        scene.insert_proxy(proxy);
        scene
    }

    fn sprite_proxy(
        proxy_id: u64,
        source: &str,
        sorting_layer: i16,
        order_in_layer: i32,
        sort_z: f32,
    ) -> RenderProxy {
        let mut proxy = RenderProxy::new(
            RenderProxyId(proxy_id),
            RuntimeEntityId::new(proxy_id as u32, 0),
            SourceEntityId::from(source),
            transform(sort_z),
            renderable(None),
        );
        proxy.payload = RenderProxyPayload::Sprite(SpritePayload {
            sprite_ref: Some(format!("sprite-{source}")),
            material_ref: Some("material-sprite".to_string()),
            color: [0.1, 0.2, 0.3, 0.4],
            flip_x: true,
            flip_y: false,
            sorting_layer,
            order_in_layer,
            sort_z,
        });
        proxy
    }

    #[test]
    fn builder_reads_render_scene_state_and_produces_draw_item() {
        let scene = scene_with_proxy(true);
        let frame = RendererFeatureBuilder::new().build(1, &scene);

        assert_eq!(frame.draw_items.len(), 1);
        assert_eq!(frame.draw_items[0].mesh_ref.as_deref(), Some("mesh-a"));
        assert_eq!(frame.counters.draw_item_count, 1);
    }

    #[test]
    fn builder_skips_invisible_proxy() {
        let scene = scene_with_proxy(false);
        let frame = RendererFeatureBuilder::new().build(1, &scene);

        assert!(frame.draw_items.is_empty());
        assert_eq!(frame.counters.skipped_invisible_count, 1);
        assert_eq!(frame.diagnostics[0].code, "proxy_not_visible");
    }

    #[test]
    fn builder_produces_per_view_output() {
        let scene = scene_with_proxy(true);
        let frame = RendererFeatureBuilder::new().build(1, &scene);

        assert_eq!(frame.views.len(), 1);
        assert_eq!(frame.views[0].visible_proxy_count, 1);
    }

    #[test]
    fn builder_output_is_stable_by_proxy_order() {
        let mut scene = scene_with_proxy(true);
        scene.insert_proxy(RenderProxy::new(
            RenderProxyId(0),
            RuntimeEntityId::new(2, 0),
            SourceEntityId::from("entity-b"),
            transform(2.0),
            renderable(Some("mesh-b")),
        ));

        let frame = RendererFeatureBuilder::new().build(1, &scene);

        assert_eq!(frame.draw_items.len(), 2);
        assert!(frame.draw_items[0].proxy_id < frame.draw_items[1].proxy_id);
    }

    #[test]
    fn sprite_draw_item_carries_sprite_fields() {
        let mut scene = RenderSceneState::new();
        scene.insert_proxy(sprite_proxy(10, "a", 2, 7, 1.25));

        let frame = RendererFeatureBuilder::new().build(1, &scene);
        let item = &frame.draw_items[0];

        assert_eq!(item.payload_kind, RenderPayloadKind::Sprite);
        assert_eq!(item.mesh_ref, None);
        assert_eq!(item.sprite_ref.as_deref(), Some("sprite-a"));
        assert_eq!(item.material_ref.as_deref(), Some("material-sprite"));
        assert_eq!(item.sorting_layer, 2);
        assert_eq!(item.order_in_layer, 7);
        assert_eq!(item.sort_z, 1.25);
        assert_eq!(item.sort_key.render_domain_order, 20);
    }

    #[test]
    fn mesh_draw_item_keeps_existing_mesh_fields() {
        let scene = scene_with_proxy(true);

        let frame = RendererFeatureBuilder::new().build(1, &scene);
        let item = &frame.draw_items[0];

        assert_eq!(item.payload_kind, RenderPayloadKind::Mesh);
        assert_eq!(item.mesh_ref.as_deref(), Some("mesh-a"));
        assert_eq!(item.sprite_ref, None);
        assert_eq!(item.material_ref.as_deref(), Some("material-a"));
        assert_eq!(item.sort_key.render_domain_order, 10);
    }

    #[test]
    fn sprite_draw_items_sort_by_layer_order_z_proxy() {
        let mut scene = RenderSceneState::new();
        scene.insert_proxy(sprite_proxy(30, "late-layer", 2, 0, 0.0));
        scene.insert_proxy(sprite_proxy(20, "middle-z", 1, 2, 2.0));
        scene.insert_proxy(sprite_proxy(10, "first", 1, 1, 9.0));
        scene.insert_proxy(sprite_proxy(40, "middle-z-first", 1, 2, 1.0));

        let frame = RendererFeatureBuilder::new().build(1, &scene);
        let sources = frame
            .draw_items
            .iter()
            .map(|item| item.source_entity_id.as_str().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            sources,
            vec!["first", "middle-z-first", "middle-z", "late-layer"]
        );
    }

    #[test]
    fn same_sprite_sort_bucket_falls_back_to_proxy_id() {
        let mut scene = RenderSceneState::new();
        scene.insert_proxy(sprite_proxy(20, "b", 1, 1, 1.0));
        scene.insert_proxy(sprite_proxy(10, "a", 1, 1, 1.0));

        let frame = RendererFeatureBuilder::new().build(1, &scene);

        assert_eq!(frame.draw_items[0].proxy_id, RenderProxyId(10));
        assert_eq!(frame.draw_items[1].proxy_id, RenderProxyId(20));
    }
}
