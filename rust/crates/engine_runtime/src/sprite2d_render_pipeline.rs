use crate::components::Transform;
use crate::ids::SourceEntityId;
use crate::render_asset_production::{RenderBindingKind, RenderBindingSet};
use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
use crate::render_state::{RenderPayloadKind, RenderProxyId, RenderViewState};
use crate::renderer_feature_builder::{DrawItemSortKey, RendererFeatureFrame};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprite2DRenderCounters {
    pub input_sprite_count: usize,
    pub draw_plan_count: usize,
    pub skipped_count: usize,
    pub fallback_binding_count: usize,
    pub diagnostic_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sprite2DRenderFrame {
    pub frame_index: u64,
    pub view_id: String,
    pub layer_mask: Option<String>,
    pub draw_plans: Vec<Sprite2DDrawPlan>,
    pub diagnostics: Vec<Sprite2DRenderDiagnostic>,
    pub counters: Sprite2DRenderCounters,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sprite2DDrawPlan {
    pub proxy_id: RenderProxyId,
    pub source_entity_id: SourceEntityId,
    pub sprite_ref: String,
    pub material_ref: Option<String>,
    pub transform: Transform,
    pub color: [f32; 4],
    pub flip_x: bool,
    pub flip_y: bool,
    pub layer: String,
    pub sort_key: DrawItemSortKey,
    pub binding: Option<RenderBindingSet>,
    pub fallback_used: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprite2DRenderDiagnostic {
    pub severity: Sprite2DRenderSeverity,
    pub code: &'static str,
    pub proxy_id: Option<RenderProxyId>,
    pub source_entity_id: Option<SourceEntityId>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sprite2DRenderSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sprite2DTextureBindingContext {
    bindings_by_sprite_ref: BTreeMap<String, RenderBindingSet>,
}

impl Sprite2DTextureBindingContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_texture_handle(
        &mut self,
        sprite_ref: impl Into<String>,
        handle: RenderResourceHandle,
        sampler: impl Into<String>,
    ) {
        let sprite_ref = sprite_ref.into();
        self.bindings_by_sprite_ref.insert(
            sprite_ref.clone(),
            RenderBindingSet {
                binding_id: format!("binding:sprite2d:{sprite_ref}"),
                binding_kind: RenderBindingKind::Texture,
                resources: vec![handle],
                material_handle: None,
                sampler: sampler.into(),
                fallback_used: false,
                debug_label: "Sprite2DTexture runtime binding".to_string(),
            },
        );
    }

    pub fn insert_binding(&mut self, sprite_ref: impl Into<String>, binding: RenderBindingSet) {
        self.bindings_by_sprite_ref
            .insert(sprite_ref.into(), binding);
    }

    pub fn binding_for(&self, sprite_ref: &str) -> Option<RenderBindingSet> {
        self.bindings_by_sprite_ref
            .get(sprite_ref)
            .filter(|binding| {
                binding.binding_kind == RenderBindingKind::Texture
                    && binding
                        .resources
                        .iter()
                        .any(|resource| resource.kind == RenderResourceKind::Texture)
            })
            .cloned()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Sprite2DRenderPipeline;

impl Sprite2DRenderPipeline {
    pub fn new() -> Self {
        Self
    }

    pub fn build(
        &self,
        frame_index: u64,
        feature_frame: &RendererFeatureFrame,
        view: Option<&RenderViewState>,
    ) -> Sprite2DRenderFrame {
        self.build_with_texture_bindings(frame_index, feature_frame, view, None)
    }

    pub fn build_with_texture_bindings(
        &self,
        frame_index: u64,
        feature_frame: &RendererFeatureFrame,
        view: Option<&RenderViewState>,
        texture_bindings: Option<&Sprite2DTextureBindingContext>,
    ) -> Sprite2DRenderFrame {
        let view_id = view
            .map(|view| view.view_id.to_string())
            .unwrap_or_else(|| "view-runtime-default".to_string());
        let layer_mask = view.and_then(|view| view.layer_mask.clone());
        let mut draw_plans = Vec::new();
        let mut diagnostics = Vec::new();
        let mut input_sprite_count = 0;
        let mut skipped_count = 0;
        let mut fallback_binding_count = 0;

        for draw_item in &feature_frame.draw_items {
            if draw_item.payload_kind != RenderPayloadKind::Sprite {
                continue;
            }
            input_sprite_count += 1;

            if let Some(layer_mask) = &layer_mask {
                if draw_item.layer != *layer_mask {
                    skipped_count += 1;
                    diagnostics.push(Sprite2DRenderDiagnostic {
                        severity: Sprite2DRenderSeverity::Info,
                        code: "sprite_layer_mismatch",
                        proxy_id: Some(draw_item.proxy_id),
                        source_entity_id: Some(draw_item.source_entity_id.clone()),
                        message: format!(
                            "Sprite proxy '{}' layer '{}' does not match view layer_mask '{}'.",
                            draw_item.proxy_id, draw_item.layer, layer_mask
                        ),
                    });
                    continue;
                }
            }

            let Some(sprite_ref) = draw_item
                .sprite_ref
                .as_ref()
                .filter(|value| !value.is_empty())
            else {
                skipped_count += 1;
                diagnostics.push(Sprite2DRenderDiagnostic {
                    severity: Sprite2DRenderSeverity::Warning,
                    code: "sprite_missing_ref",
                    proxy_id: Some(draw_item.proxy_id),
                    source_entity_id: Some(draw_item.source_entity_id.clone()),
                    message: format!(
                        "Sprite proxy '{}' has no sprite_ref and cannot create a draw plan.",
                        draw_item.proxy_id
                    ),
                });
                continue;
            };

            let binding = texture_bindings
                .and_then(|bindings| bindings.binding_for(sprite_ref))
                .unwrap_or_else(|| {
                    fallback_binding_count += 1;
                    let fallback = fallback_sprite_binding(sprite_ref, draw_item.proxy_id);
                    diagnostics.push(Sprite2DRenderDiagnostic {
                        severity: Sprite2DRenderSeverity::Warning,
                        code: "sprite_binding_fallback",
                        proxy_id: Some(draw_item.proxy_id),
                        source_entity_id: Some(draw_item.source_entity_id.clone()),
                        message: format!(
                            "Sprite '{}' uses fallback Sprite2DTexture binding until runtime asset records are connected.",
                            sprite_ref
                        ),
                    });
                    fallback
                });
            if !binding.fallback_used {
                diagnostics.push(Sprite2DRenderDiagnostic {
                    severity: Sprite2DRenderSeverity::Info,
                    code: "sprite_texture_binding_ready",
                    proxy_id: Some(draw_item.proxy_id),
                    source_entity_id: Some(draw_item.source_entity_id.clone()),
                    message: format!(
                        "Sprite '{}' uses a prepared runtime texture binding.",
                        sprite_ref
                    ),
                });
            }
            diagnostics.push(Sprite2DRenderDiagnostic {
                severity: Sprite2DRenderSeverity::Info,
                code: "sprite_ready",
                proxy_id: Some(draw_item.proxy_id),
                source_entity_id: Some(draw_item.source_entity_id.clone()),
                message: format!("Sprite '{}' produced a Sprite2D draw plan.", sprite_ref),
            });

            draw_plans.push(Sprite2DDrawPlan {
                proxy_id: draw_item.proxy_id,
                source_entity_id: draw_item.source_entity_id.clone(),
                sprite_ref: sprite_ref.clone(),
                material_ref: draw_item.material_ref.clone(),
                transform: draw_item.transform.clone(),
                color: draw_item.color,
                flip_x: draw_item.flip_x,
                flip_y: draw_item.flip_y,
                layer: draw_item.layer.clone(),
                sort_key: draw_item.sort_key,
                fallback_used: binding.fallback_used,
                binding: Some(binding),
            });
        }

        draw_plans.sort_by_key(|plan| plan.sort_key);

        Sprite2DRenderFrame {
            frame_index,
            view_id,
            layer_mask,
            counters: Sprite2DRenderCounters {
                input_sprite_count,
                draw_plan_count: draw_plans.len(),
                skipped_count,
                fallback_binding_count,
                diagnostic_count: diagnostics.len(),
            },
            draw_plans,
            diagnostics,
        }
    }
}

fn fallback_sprite_binding(sprite_ref: &str, proxy_id: RenderProxyId) -> RenderBindingSet {
    RenderBindingSet {
        binding_id: format!("binding:sprite2d:{sprite_ref}"),
        binding_kind: RenderBindingKind::Texture,
        resources: vec![RenderResourceHandle {
            kind: RenderResourceKind::Texture,
            index: proxy_id.0,
            generation: 0,
        }],
        material_handle: None,
        sampler: "linearClamp".to_string(),
        fallback_used: true,
        debug_label: "Sprite2DTexture fallback binding".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Renderable, Transform};
    use crate::ids::RuntimeEntityId;
    use crate::math::Vec3;
    use crate::render_state::{
        RenderProxy, RenderProxyPayload, RenderSceneState, RenderTargetKind, RenderViewId,
        RenderViewKind, RenderViewState, SpritePayload,
    };
    use crate::renderer_feature_builder::RendererFeatureBuilder;

    fn transform(z: f32) -> Transform {
        Transform {
            local_position: Vec3 { x: 0.0, y: 0.0, z },
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        }
    }

    fn sprite_proxy(
        proxy_id: u64,
        source: &str,
        sprite_ref: Option<&str>,
        layer: &str,
        sorting_layer: i16,
        order_in_layer: i32,
        sort_z: f32,
    ) -> RenderProxy {
        let mut proxy = RenderProxy::new(
            RenderProxyId(proxy_id),
            RuntimeEntityId::new(proxy_id as u32, 0),
            SourceEntityId::from(source),
            transform(sort_z),
            Renderable {
                mesh_ref: None,
                material_ref: Some("material-sprite".to_string()),
                visible: true,
                layer: layer.to_string(),
            },
        );
        proxy.common.layer = layer.to_string();
        proxy.payload = RenderProxyPayload::Sprite(SpritePayload {
            sprite_ref: sprite_ref.map(str::to_string),
            material_ref: Some("material-sprite".to_string()),
            color: [1.0, 1.0, 1.0, 1.0],
            flip_x: false,
            flip_y: false,
            sorting_layer,
            order_in_layer,
            sort_z,
        });
        proxy
    }

    fn feature_frame(scene: &RenderSceneState) -> RendererFeatureFrame {
        RendererFeatureBuilder::new().build(1, scene)
    }

    #[test]
    fn sprite2d_render_pipeline_creates_draw_plan_for_visible_sprite() {
        let mut scene = RenderSceneState::new();
        scene.insert_proxy(sprite_proxy(
            1,
            "sprite-a",
            Some("ship"),
            "sprite2d",
            0,
            0,
            0.0,
        ));
        let frame = feature_frame(&scene);

        let output = Sprite2DRenderPipeline::new().build(1, &frame, None);

        assert_eq!(output.draw_plans.len(), 1);
        assert_eq!(output.draw_plans[0].sprite_ref, "ship");
        assert!(output.draw_plans[0].binding.is_some());
        assert!(output.draw_plans[0].fallback_used);
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "sprite_ready"));
    }

    #[test]
    fn sprite2d_render_pipeline_reports_missing_sprite_ref() {
        let mut scene = RenderSceneState::new();
        scene.insert_proxy(sprite_proxy(1, "sprite-a", None, "sprite2d", 0, 0, 0.0));
        let frame = feature_frame(&scene);

        let output = Sprite2DRenderPipeline::new().build(1, &frame, None);

        assert!(output.draw_plans.is_empty());
        assert_eq!(output.counters.skipped_count, 1);
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "sprite_missing_ref"));
    }

    #[test]
    fn sprite2d_render_pipeline_reports_layer_mismatch() {
        let mut scene = RenderSceneState::new();
        scene.insert_proxy(sprite_proxy(
            1,
            "sprite-a",
            Some("ship"),
            "background",
            0,
            0,
            0.0,
        ));
        let mut view = RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::Window,
        );
        view.layer_mask = Some("sprite2d".to_string());
        let frame = feature_frame(&scene);

        let output = Sprite2DRenderPipeline::new().build(1, &frame, Some(&view));

        assert!(output.draw_plans.is_empty());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "sprite_layer_mismatch"));
    }

    #[test]
    fn sprite2d_render_pipeline_sorts_by_existing_sort_key() {
        let mut scene = RenderSceneState::new();
        scene.insert_proxy(sprite_proxy(
            30,
            "late-layer",
            Some("late"),
            "sprite2d",
            2,
            0,
            0.0,
        ));
        scene.insert_proxy(sprite_proxy(
            20,
            "middle-z",
            Some("middle"),
            "sprite2d",
            1,
            2,
            2.0,
        ));
        scene.insert_proxy(sprite_proxy(
            10,
            "first",
            Some("first"),
            "sprite2d",
            1,
            1,
            9.0,
        ));
        scene.insert_proxy(sprite_proxy(
            40,
            "middle-z-first",
            Some("middle-first"),
            "sprite2d",
            1,
            2,
            1.0,
        ));
        let frame = feature_frame(&scene);

        let output = Sprite2DRenderPipeline::new().build(1, &frame, None);
        let refs = output
            .draw_plans
            .iter()
            .map(|plan| plan.sprite_ref.as_str())
            .collect::<Vec<_>>();

        assert_eq!(refs, vec!["first", "middle-first", "middle", "late"]);
    }

    #[test]
    fn sprite2d_render_pipeline_binding_uses_sprite2d_texture_semantics() {
        let mut scene = RenderSceneState::new();
        scene.insert_proxy(sprite_proxy(
            7,
            "sprite-a",
            Some("ship"),
            "sprite2d",
            0,
            0,
            0.0,
        ));
        let frame = feature_frame(&scene);

        let output = Sprite2DRenderPipeline::new().build(1, &frame, None);
        let binding = output.draw_plans[0].binding.as_ref().expect("binding");

        assert_eq!(binding.binding_kind, RenderBindingKind::Texture);
        assert_eq!(binding.binding_id, "binding:sprite2d:ship");
        assert!(binding.fallback_used);
        assert_eq!(binding.resources[0].kind, RenderResourceKind::Texture);
    }

    #[test]
    fn sprite2d_render_pipeline_uses_prepared_texture_binding_when_available() {
        let mut scene = RenderSceneState::new();
        scene.insert_proxy(sprite_proxy(
            7,
            "sprite-a",
            Some("ship"),
            "sprite2d",
            0,
            0,
            0.0,
        ));
        let frame = feature_frame(&scene);
        let mut bindings = Sprite2DTextureBindingContext::new();
        bindings.insert_texture_handle(
            "ship",
            RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 42,
                generation: 3,
            },
            "nearestClamp",
        );

        let output = Sprite2DRenderPipeline::new().build_with_texture_bindings(
            1,
            &frame,
            None,
            Some(&bindings),
        );
        let plan = &output.draw_plans[0];
        let binding = plan.binding.as_ref().expect("binding");

        assert!(!plan.fallback_used);
        assert_eq!(output.counters.fallback_binding_count, 0);
        assert_eq!(binding.resources[0].index, 42);
        assert_eq!(binding.resources[0].generation, 3);
        assert_eq!(binding.sampler, "nearestClamp");
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "sprite_texture_binding_ready"));
    }
}
