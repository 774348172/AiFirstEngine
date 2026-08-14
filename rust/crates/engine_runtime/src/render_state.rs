use crate::components::{Renderable, SpriteRenderer2D, Transform};
use crate::ids::{RuntimeEntityId, SourceEntityId};
use crate::math::Vec3;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderProxyId(pub u64);

impl fmt::Display for RenderProxyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "proxy-{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderViewId(pub u64);

impl fmt::Display for RenderViewId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "view-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bounds {
    pub center: Vec3,
    pub extents: Vec3,
}

impl Bounds {
    pub fn from_transform(transform: &Transform) -> Self {
        Self {
            center: transform.local_position,
            extents: transform.local_scale,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderProxyCommon {
    pub proxy_id: RenderProxyId,
    pub runtime_entity_id: RuntimeEntityId,
    pub source_entity_id: SourceEntityId,
    pub enabled: bool,
    pub visible: bool,
    pub layer: String,
    pub flags: u32,
    pub transform: Transform,
    pub previous_transform: Transform,
    pub bounds: Bounds,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderProxyPayload {
    Mesh(MeshPayload),
    Sprite(SpritePayload),
    SkinnedMesh(SkinnedMeshPayload),
    Light(LightPayload),
    Camera(CameraPayload),
    Particle(ParticlePayload),
    Instance(InstancePayload),
}

impl RenderProxyPayload {
    pub fn kind(&self) -> RenderPayloadKind {
        match self {
            Self::Mesh(_) => RenderPayloadKind::Mesh,
            Self::Sprite(_) => RenderPayloadKind::Sprite,
            Self::SkinnedMesh(_) => RenderPayloadKind::SkinnedMesh,
            Self::Light(_) => RenderPayloadKind::Light,
            Self::Camera(_) => RenderPayloadKind::Camera,
            Self::Particle(_) => RenderPayloadKind::Particle,
            Self::Instance(_) => RenderPayloadKind::Instance,
        }
    }

    pub fn from_renderable(renderable: &Renderable) -> Self {
        Self::Mesh(MeshPayload {
            mesh_ref: renderable.mesh_ref.clone(),
            material_ref: renderable.material_ref.clone(),
        })
    }

    pub fn from_sprite_renderer2d(sprite: &SpriteRenderer2D) -> Self {
        Self::Sprite(SpritePayload::from_sprite_renderer2d(sprite))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderPayloadKind {
    Mesh,
    Sprite,
    SkinnedMesh,
    Light,
    Camera,
    Particle,
    Instance,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshPayload {
    pub mesh_ref: Option<String>,
    pub material_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpritePayload {
    pub sprite_ref: Option<String>,
    pub material_ref: Option<String>,
    pub color: [f32; 4],
    pub flip_x: bool,
    pub flip_y: bool,
    pub sorting_layer: i16,
    pub order_in_layer: i32,
    pub sort_z: f32,
}

impl SpritePayload {
    pub fn new(sprite_ref: impl Into<String>) -> Self {
        Self {
            sprite_ref: Some(sprite_ref.into()),
            ..Self::default()
        }
    }

    pub fn with_material(sprite_ref: impl Into<String>, material_ref: impl Into<String>) -> Self {
        Self {
            material_ref: Some(material_ref.into()),
            ..Self::new(sprite_ref)
        }
    }

    pub fn from_sprite_renderer2d(sprite: &SpriteRenderer2D) -> Self {
        Self {
            sprite_ref: sprite.sprite_ref.clone(),
            material_ref: sprite.material_ref.clone(),
            color: sprite.color,
            flip_x: sprite.flip_x,
            flip_y: sprite.flip_y,
            sorting_layer: sprite.sorting_layer,
            order_in_layer: sprite.order_in_layer,
            sort_z: sprite.sort_z,
        }
    }
}

impl Default for SpritePayload {
    fn default() -> Self {
        Self {
            sprite_ref: None,
            material_ref: None,
            color: [1.0, 1.0, 1.0, 1.0],
            flip_x: false,
            flip_y: false,
            sorting_layer: 0,
            order_in_layer: 0,
            sort_z: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkinnedMeshPayload {
    pub mesh_ref: Option<String>,
    pub material_ref: Option<String>,
    pub skeleton_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightPayload {
    pub intensity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraPayload {
    pub fov_y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParticlePayload {
    pub effect_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InstancePayload {
    pub instance_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderProxy {
    pub common: RenderProxyCommon,
    pub payload: RenderProxyPayload,
}

impl RenderProxy {
    pub fn new(
        proxy_id: RenderProxyId,
        runtime_entity_id: RuntimeEntityId,
        source_entity_id: SourceEntityId,
        transform: Transform,
        renderable: Renderable,
    ) -> Self {
        let bounds = Bounds::from_transform(&transform);
        Self {
            common: RenderProxyCommon {
                proxy_id,
                runtime_entity_id,
                source_entity_id,
                enabled: true,
                visible: renderable.visible,
                layer: renderable.layer.clone(),
                flags: 0,
                transform: transform.clone(),
                previous_transform: transform,
                bounds,
                version: 0,
            },
            payload: RenderProxyPayload::from_renderable(&renderable),
        }
    }

    pub fn from_descriptor(
        proxy_id: RenderProxyId,
        runtime_entity_id: RuntimeEntityId,
        source_entity_id: SourceEntityId,
        transform: Transform,
        descriptor: RenderProxyDescriptor,
    ) -> Self {
        let bounds = Bounds::from_transform(&transform);
        Self {
            common: RenderProxyCommon {
                proxy_id,
                runtime_entity_id,
                source_entity_id,
                enabled: true,
                visible: descriptor.visible,
                layer: descriptor.layer,
                flags: 0,
                transform: transform.clone(),
                previous_transform: transform,
                bounds,
                version: 0,
            },
            payload: descriptor.payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderProxyDescriptor {
    pub visible: bool,
    pub layer: String,
    pub payload: RenderProxyPayload,
}

impl RenderProxyDescriptor {
    pub fn from_renderable(renderable: Renderable) -> Self {
        Self {
            visible: renderable.visible,
            layer: renderable.layer.clone(),
            payload: RenderProxyPayload::from_renderable(&renderable),
        }
    }

    pub fn from_sprite_renderer2d(sprite: SpriteRenderer2D) -> Self {
        Self {
            visible: sprite.visible,
            layer: "sprite2d".to_string(),
            payload: RenderProxyPayload::from_sprite_renderer2d(&sprite),
        }
    }

    pub fn payload_kind(&self) -> RenderPayloadKind {
        self.payload.kind()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderViewKind {
    Game,
    SceneView,
    Preview,
    Shadow,
    Reflection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderTargetKind {
    Window,
    ViewportTexture,
    RenderTexture,
    ShadowMap,
    PreviewTexture,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderViewState {
    pub view_id: RenderViewId,
    pub source_entity_id: Option<SourceEntityId>,
    pub view_kind: RenderViewKind,
    pub viewport: Viewport,
    pub target: RenderTargetKind,
    pub view_matrix: [f32; 16],
    pub projection_matrix: [f32; 16],
    pub clear_color: [f32; 4],
    pub layer_mask: Option<String>,
    pub version: u64,
}

impl RenderViewState {
    pub fn new(view_id: RenderViewId, view_kind: RenderViewKind, target: RenderTargetKind) -> Self {
        Self {
            view_id,
            source_entity_id: None,
            view_kind,
            viewport: Viewport {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            target,
            view_matrix: identity_matrix(),
            projection_matrix: identity_matrix(),
            clear_color: [0.0, 0.0, 0.0, 1.0],
            layer_mask: None,
            version: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderFrameViewData {
    pub view_id: RenderViewId,
    pub visible_proxy_ids: Vec<RenderProxyId>,
    pub culling_result_summary: String,
    pub render_phase_summary: String,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RenderSceneState {
    proxies: BTreeMap<RenderProxyId, RenderProxy>,
    entity_to_proxy: BTreeMap<RuntimeEntityId, RenderProxyId>,
    source_to_proxy: BTreeMap<SourceEntityId, RenderProxyId>,
    view_registry: BTreeMap<RenderViewId, RenderViewState>,
    next_proxy_id: u64,
}

impl RenderSceneState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_view(&mut self, view: RenderViewState) {
        self.view_registry.insert(view.view_id, view);
    }

    pub fn view(&self, view_id: RenderViewId) -> Option<&RenderViewState> {
        self.view_registry.get(&view_id)
    }

    pub fn views(&self) -> impl Iterator<Item = &RenderViewState> {
        self.view_registry.values()
    }

    pub fn proxy(&self, proxy_id: RenderProxyId) -> Option<&RenderProxy> {
        self.proxies.get(&proxy_id)
    }

    pub fn proxies(&self) -> impl Iterator<Item = &RenderProxy> {
        self.proxies.values()
    }

    pub fn proxy_mut(&mut self, proxy_id: RenderProxyId) -> Option<&mut RenderProxy> {
        self.proxies.get_mut(&proxy_id)
    }

    pub fn proxy_for_source(&self, source_id: &SourceEntityId) -> Option<RenderProxyId> {
        self.source_to_proxy.get(source_id).copied()
    }

    pub fn proxy_for_runtime(&self, runtime_id: RuntimeEntityId) -> Option<RenderProxyId> {
        self.entity_to_proxy.get(&runtime_id).copied()
    }

    pub fn insert_proxy(&mut self, mut proxy: RenderProxy) -> RenderProxyId {
        let proxy_id = if proxy.common.proxy_id.0 == 0 {
            self.allocate_proxy_id()
        } else {
            proxy.common.proxy_id
        };
        proxy.common.proxy_id = proxy_id;
        self.entity_to_proxy
            .insert(proxy.common.runtime_entity_id, proxy_id);
        self.source_to_proxy
            .insert(proxy.common.source_entity_id.clone(), proxy_id);
        self.proxies.insert(proxy_id, proxy);
        proxy_id
    }

    pub fn remove_proxy(&mut self, proxy_id: RenderProxyId) -> Option<RenderProxy> {
        let removed = self.proxies.remove(&proxy_id)?;
        self.entity_to_proxy
            .remove(&removed.common.runtime_entity_id);
        self.source_to_proxy
            .remove(&removed.common.source_entity_id);
        Some(removed)
    }

    pub fn proxies_len(&self) -> usize {
        self.proxies.len()
    }

    pub fn build_frame_view_data(&self, view_id: RenderViewId) -> Option<RenderFrameViewData> {
        self.view_registry.get(&view_id)?;
        Some(RenderFrameViewData {
            view_id,
            visible_proxy_ids: self.proxies.keys().copied().collect(),
            culling_result_summary: "not_implemented".to_string(),
            render_phase_summary: "not_implemented".to_string(),
            diagnostics: Vec::new(),
        })
    }

    fn allocate_proxy_id(&mut self) -> RenderProxyId {
        self.next_proxy_id += 1;
        RenderProxyId(self.next_proxy_id)
    }
}

fn identity_matrix() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec3;

    fn transform(x: f32) -> Transform {
        Transform {
            local_position: Vec3 { x, y: 0.0, z: 0.0 },
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        }
    }

    fn renderable(mesh: &str) -> Renderable {
        Renderable {
            mesh_ref: Some(mesh.to_string()),
            material_ref: None,
            visible: true,
            layer: "default".to_string(),
        }
    }

    #[test]
    fn create_render_scene_state_with_two_views() {
        let mut scene = RenderSceneState::new();
        scene.register_view(RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::Window,
        ));
        scene.register_view(RenderViewState::new(
            RenderViewId(2),
            RenderViewKind::SceneView,
            RenderTargetKind::ViewportTexture,
        ));

        assert_eq!(scene.views().count(), 2);
        assert_eq!(
            scene.view(RenderViewId(1)).unwrap().view_kind,
            RenderViewKind::Game
        );
        assert_eq!(
            scene.view(RenderViewId(2)).unwrap().view_kind,
            RenderViewKind::SceneView
        );
    }

    #[test]
    fn add_proxy_and_find_by_source_entity() {
        let mut scene = RenderSceneState::new();
        let source_id = SourceEntityId::from("entity-a");
        let runtime_id = RuntimeEntityId::new(7, 0);
        let proxy = RenderProxy::new(
            RenderProxyId(0),
            runtime_id,
            source_id.clone(),
            transform(1.0),
            renderable("mesh-a"),
        );

        let proxy_id = scene.insert_proxy(proxy);

        assert_eq!(scene.proxy_for_source(&source_id), Some(proxy_id));
        assert_eq!(scene.proxy_for_runtime(runtime_id), Some(proxy_id));
        assert_eq!(
            scene.proxy(proxy_id).unwrap().payload.kind(),
            RenderPayloadKind::Mesh
        );
    }

    #[test]
    fn sprite_payload_defaults_are_stable() {
        let payload = SpritePayload::default();

        assert_eq!(payload.sprite_ref, None);
        assert_eq!(payload.material_ref, None);
        assert_eq!(payload.color, [1.0, 1.0, 1.0, 1.0]);
        assert!(!payload.flip_x);
        assert!(!payload.flip_y);
        assert_eq!(payload.sorting_layer, 0);
        assert_eq!(payload.order_in_layer, 0);
        assert_eq!(payload.sort_z, 0.0);
    }

    #[test]
    fn sprite_payload_keeps_authoring_fields() {
        let payload = SpritePayload {
            sprite_ref: Some("sprite-a".to_string()),
            material_ref: Some("material-a".to_string()),
            color: [0.2, 0.4, 0.6, 0.8],
            flip_x: true,
            flip_y: true,
            sorting_layer: 2,
            order_in_layer: 10,
            sort_z: -3.5,
        };

        assert_eq!(payload.sprite_ref.as_deref(), Some("sprite-a"));
        assert_eq!(payload.material_ref.as_deref(), Some("material-a"));
        assert_eq!(payload.color, [0.2, 0.4, 0.6, 0.8]);
        assert!(payload.flip_x);
        assert!(payload.flip_y);
        assert_eq!(payload.sorting_layer, 2);
        assert_eq!(payload.order_in_layer, 10);
        assert_eq!(payload.sort_z, -3.5);
    }
}
