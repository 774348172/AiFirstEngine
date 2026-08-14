use crate::ids::SourceEntityId;
use crate::render_state::RenderProxyId;
use crate::renderer_feature_builder::{
    DrawItemSortKey, RendererFeatureDrawItem, RendererFeatureFrame,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MinimalRenderer;

impl MinimalRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, feature_frame: &RendererFeatureFrame) -> MinimalRendererFrame {
        let mut draw_records = Vec::new();
        let mut warnings = Vec::new();

        for draw_item in &feature_frame.draw_items {
            if draw_item.mesh_ref.is_none() && draw_item.sprite_ref.is_none() {
                warnings.push(MinimalRendererWarning {
                    code: "missing_draw_source",
                    proxy_id: draw_item.proxy_id,
                    source_entity_id: draw_item.source_entity_id.clone(),
                });
            }
            draw_records.push(MinimalRendererDrawRecord::from_draw_item(draw_item));
        }

        MinimalRendererFrame {
            frame_index: feature_frame.frame_index,
            view_count: feature_frame.views.len(),
            draw_record_count: draw_records.len(),
            draw_records,
            warnings,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalRendererFrame {
    pub frame_index: u64,
    pub view_count: usize,
    pub draw_record_count: usize,
    pub draw_records: Vec<MinimalRendererDrawRecord>,
    pub warnings: Vec<MinimalRendererWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalRendererDrawRecord {
    pub proxy_id: RenderProxyId,
    pub source_entity_id: SourceEntityId,
    pub mesh_ref: Option<String>,
    pub sprite_ref: Option<String>,
    pub material_ref: Option<String>,
    pub layer: String,
    pub sort_key: DrawItemSortKey,
}

impl MinimalRendererDrawRecord {
    fn from_draw_item(draw_item: &RendererFeatureDrawItem) -> Self {
        Self {
            proxy_id: draw_item.proxy_id,
            source_entity_id: draw_item.source_entity_id.clone(),
            mesh_ref: draw_item.mesh_ref.clone(),
            sprite_ref: draw_item.sprite_ref.clone(),
            material_ref: draw_item.material_ref.clone(),
            layer: draw_item.layer.clone(),
            sort_key: draw_item.sort_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalRendererWarning {
    pub code: &'static str,
    pub proxy_id: RenderProxyId,
    pub source_entity_id: SourceEntityId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Transform;
    use crate::ids::SourceEntityId;
    use crate::math::Vec3;
    use crate::render_state::{RenderPayloadKind, RenderProxyId};
    use crate::renderer_feature_builder::{
        DrawItemSortKey, RendererFeatureCounters, RendererFeatureFrame,
    };

    fn draw_item(
        mesh_ref: Option<&str>,
    ) -> crate::renderer_feature_builder::RendererFeatureDrawItem {
        crate::renderer_feature_builder::RendererFeatureDrawItem {
            proxy_id: RenderProxyId(1),
            source_entity_id: SourceEntityId::from("entity-a"),
            payload_kind: RenderPayloadKind::Mesh,
            mesh_ref: mesh_ref.map(str::to_string),
            sprite_ref: None,
            material_ref: Some("material-a".to_string()),
            color: [1.0, 1.0, 1.0, 1.0],
            flip_x: false,
            flip_y: false,
            transform: Transform {
                local_position: Vec3::ZERO,
                local_rotation: Vec3::ZERO,
                local_scale: Vec3::ONE,
            },
            visible: true,
            layer: "default".to_string(),
            sorting_layer: 0,
            order_in_layer: 0,
            sort_z: 0.0,
            sort_key: DrawItemSortKey {
                render_domain_order: 10,
                sorting_layer: 0,
                order_in_layer: 0,
                sort_z_quantized: 0,
                stable_proxy_id: 1,
            },
        }
    }

    fn sprite_draw_item() -> crate::renderer_feature_builder::RendererFeatureDrawItem {
        crate::renderer_feature_builder::RendererFeatureDrawItem {
            proxy_id: RenderProxyId(2),
            source_entity_id: SourceEntityId::from("entity-sprite"),
            payload_kind: RenderPayloadKind::Sprite,
            mesh_ref: None,
            sprite_ref: Some("sprite-a".to_string()),
            material_ref: Some("material-sprite".to_string()),
            color: [1.0, 1.0, 1.0, 1.0],
            flip_x: false,
            flip_y: false,
            transform: Transform {
                local_position: Vec3::ZERO,
                local_rotation: Vec3::ZERO,
                local_scale: Vec3::ONE,
            },
            visible: true,
            layer: "default".to_string(),
            sorting_layer: 1,
            order_in_layer: 2,
            sort_z: 3.0,
            sort_key: DrawItemSortKey {
                render_domain_order: 20,
                sorting_layer: 1,
                order_in_layer: 2,
                sort_z_quantized: 3000,
                stable_proxy_id: 2,
            },
        }
    }

    fn feature_frame(mesh_ref: Option<&str>) -> RendererFeatureFrame {
        RendererFeatureFrame {
            frame_index: 1,
            views: Vec::new(),
            draw_items: vec![draw_item(mesh_ref)],
            diagnostics: Vec::new(),
            counters: RendererFeatureCounters {
                view_count: 0,
                draw_item_count: 1,
                skipped_invisible_count: 0,
                warning_count: 0,
            },
        }
    }

    #[test]
    fn minimal_renderer_consumes_feature_frame() {
        let frame = MinimalRenderer::new().render(&feature_frame(Some("mesh-a")));

        assert_eq!(frame.draw_record_count, 1);
        assert_eq!(frame.draw_records[0].mesh_ref.as_deref(), Some("mesh-a"));
    }

    #[test]
    fn minimal_renderer_reports_missing_draw_source() {
        let frame = MinimalRenderer::new().render(&feature_frame(None));

        assert_eq!(frame.warnings.len(), 1);
        assert_eq!(frame.warnings[0].code, "missing_draw_source");
    }

    #[test]
    fn minimal_renderer_report_includes_sprite_draw_item() {
        let feature_frame = RendererFeatureFrame {
            frame_index: 1,
            views: Vec::new(),
            draw_items: vec![sprite_draw_item()],
            diagnostics: Vec::new(),
            counters: RendererFeatureCounters {
                view_count: 0,
                draw_item_count: 1,
                skipped_invisible_count: 0,
                warning_count: 0,
            },
        };

        let frame = MinimalRenderer::new().render(&feature_frame);

        assert!(frame.warnings.is_empty());
        assert_eq!(
            frame.draw_records[0].sprite_ref.as_deref(),
            Some("sprite-a")
        );
        assert_eq!(frame.draw_records[0].sort_key.render_domain_order, 20);
    }
}
