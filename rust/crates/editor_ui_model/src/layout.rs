use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelLayoutModel {
    pub layout_id: String,
    pub mode: PanelLayoutMode,
    pub regions: Vec<PanelRegion>,
}

impl PanelLayoutModel {
    pub fn fixed_mvp() -> Self {
        Self {
            layout_id: "native-editor-mvp.fixed".to_string(),
            mode: PanelLayoutMode::Fixed,
            regions: vec![
                PanelRegion::single("top", "toolbar"),
                PanelRegion::single("left", "hierarchy"),
                PanelRegion::single("center", "viewport"),
                PanelRegion::single("right", "inspector"),
                PanelRegion {
                    region_id: "bottom".to_string(),
                    panel_ids: vec![
                        "console".to_string(),
                        "runtime_trace".to_string(),
                        "ai_panel".to_string(),
                    ],
                    active_panel_id: Some("console".to_string()),
                },
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelLayoutMode {
    Fixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelRegion {
    pub region_id: String,
    pub panel_ids: Vec<String>,
    pub active_panel_id: Option<String>,
}

impl PanelRegion {
    pub fn single(region_id: impl Into<String>, panel_id: impl Into<String>) -> Self {
        let panel_id = panel_id.into();
        Self {
            region_id: region_id.into(),
            panel_ids: vec![panel_id.clone()],
            active_panel_id: Some(panel_id),
        }
    }
}
