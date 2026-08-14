use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_GAME_VIEW_TARGET_WIDTH: u32 = 1280;
pub const DEFAULT_GAME_VIEW_TARGET_HEIGHT: u32 = 720;
pub const MAX_GAME_VIEW_TARGET_DIMENSION: u32 = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewExtent {
    pub width: u32,
    pub height: u32,
}

impl GameViewExtent {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    fn validate_target(self) -> Result<Self, GameViewPresentationError> {
        if self.width == 0 || self.height == 0 {
            return Err(GameViewPresentationError::new(
                "game_view.presentation.target_extent_invalid",
            ));
        }
        if self.width > MAX_GAME_VIEW_TARGET_DIMENSION
            || self.height > MAX_GAME_VIEW_TARGET_DIMENSION
        {
            return Err(GameViewPresentationError::new(
                "game_view.presentation.target_capability_exceeded",
            ));
        }
        Ok(self)
    }

    fn validate_canvas(self) -> Result<Self, GameViewPresentationError> {
        if self.width == 0 || self.height == 0 {
            return Err(GameViewPresentationError::new(
                "game_view.presentation.canvas_extent_invalid",
            ));
        }
        Ok(self)
    }
}

impl Default for GameViewExtent {
    fn default() -> Self {
        Self::new(
            DEFAULT_GAME_VIEW_TARGET_WIDTH,
            DEFAULT_GAME_VIEW_TARGET_HEIGHT,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl GameViewRect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn from_extent(extent: GameViewExtent) -> Self {
        Self::new(0.0, 0.0, extent.width as f32, extent.height as f32)
    }

    pub fn contains(self, point: GameViewPoint) -> bool {
        point.x >= self.x
            && point.y >= self.y
            && point.x < self.x + self.width
            && point.y < self.y + self.height
    }

    fn validate_display(self) -> Result<Self, GameViewPresentationError> {
        if !self.x.is_finite()
            || !self.y.is_finite()
            || !self.width.is_finite()
            || !self.height.is_finite()
            || self.width <= 0.0
            || self.height <= 0.0
        {
            return Err(GameViewPresentationError::new(
                "game_view.presentation.display_rect_invalid",
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewPoint {
    pub x: f32,
    pub y: f32,
}

impl GameViewPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn validate(self) -> Result<Self, GameViewPresentationError> {
        if !self.x.is_finite() || !self.y.is_finite() {
            return Err(GameViewPresentationError::new(
                "game_view.presentation.inverse_non_finite",
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameViewScalePolicy {
    #[default]
    Contain,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewTargetSpec {
    pub extent: GameViewExtent,
    pub scale_policy: GameViewScalePolicy,
}

impl GameViewTargetSpec {
    pub const fn new(width: u32, height: u32, scale_policy: GameViewScalePolicy) -> Self {
        Self {
            extent: GameViewExtent::new(width, height),
            scale_policy,
        }
    }

    pub const fn portrait_1080x1920() -> Self {
        Self::new(1080, 1920, GameViewScalePolicy::Contain)
    }

    pub const fn portrait_720x1280() -> Self {
        Self::new(720, 1280, GameViewScalePolicy::Contain)
    }

    pub fn validate(self) -> Result<Self, GameViewPresentationError> {
        self.extent.validate_target()?;
        Ok(self)
    }
}

impl Default for GameViewTargetSpec {
    fn default() -> Self {
        Self::new(
            DEFAULT_GAME_VIEW_TARGET_WIDTH,
            DEFAULT_GAME_VIEW_TARGET_HEIGHT,
            GameViewScalePolicy::Stretch,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasReferenceFact {
    pub canvas_id: String,
    pub reference_extent: GameViewExtent,
}

impl CanvasReferenceFact {
    pub fn new(canvas_id: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            canvas_id: canvas_id.into(),
            reference_extent: GameViewExtent::new(width, height),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewPresentationSpec {
    pub session_id: String,
    pub target_id: String,
    pub target_extent: GameViewExtent,
    pub display_rect: GameViewRect,
    pub scale_policy: GameViewScalePolicy,
    pub surface_generation: u64,
    pub presentation_revision: u64,
    pub canvas_references: Vec<CanvasReferenceFact>,
}

impl GameViewPresentationSpec {
    pub fn legacy(
        session_id: impl Into<String>,
        target_id: impl Into<String>,
        display_rect: GameViewRect,
        canvas_references: Vec<CanvasReferenceFact>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            target_id: target_id.into(),
            target_extent: GameViewExtent::default(),
            display_rect,
            scale_policy: GameViewScalePolicy::Stretch,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewPresentationIdentity {
    pub session_id: String,
    pub target_id: String,
    pub surface_generation: u64,
    pub presentation_revision: u64,
    pub compact_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameViewPresentationError {
    pub code: &'static str,
    pub canvas_id: Option<String>,
}

impl GameViewPresentationError {
    fn new(code: &'static str) -> Self {
        Self {
            code,
            canvas_id: None,
        }
    }

    fn for_canvas(code: &'static str, canvas_id: &str) -> Self {
        Self {
            code,
            canvas_id: Some(canvas_id.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AxisAlignedTransform {
    source_rect: GameViewRect,
    destination_rect: GameViewRect,
}

impl AxisAlignedTransform {
    fn resolve(
        source_rect: GameViewRect,
        destination_slot: GameViewRect,
        policy: GameViewScalePolicy,
    ) -> Self {
        let destination_rect = match policy {
            GameViewScalePolicy::Stretch => destination_slot,
            GameViewScalePolicy::Contain => {
                let scale = (destination_slot.width / source_rect.width)
                    .min(destination_slot.height / source_rect.height);
                let width = source_rect.width * scale;
                let height = source_rect.height * scale;
                GameViewRect::new(
                    destination_slot.x + (destination_slot.width - width) * 0.5,
                    destination_slot.y + (destination_slot.height - height) * 0.5,
                    width,
                    height,
                )
            }
        };
        Self {
            source_rect,
            destination_rect,
        }
    }

    fn forward(self, point: GameViewPoint) -> Result<GameViewPoint, GameViewPresentationError> {
        let point = point.validate()?;
        if !self.source_rect.contains(point) {
            return Err(GameViewPresentationError::new(
                "game_view.presentation.point_outside_content",
            ));
        }
        Ok(GameViewPoint::new(
            self.destination_rect.x
                + (point.x - self.source_rect.x) / self.source_rect.width
                    * self.destination_rect.width,
            self.destination_rect.y
                + (point.y - self.source_rect.y) / self.source_rect.height
                    * self.destination_rect.height,
        ))
    }

    fn inverse(self, point: GameViewPoint) -> Result<GameViewPoint, GameViewPresentationError> {
        let point = point.validate()?;
        if !self.destination_rect.contains(point) {
            return Err(GameViewPresentationError::new(
                "game_view.presentation.point_outside_content",
            ));
        }
        Ok(GameViewPoint::new(
            self.source_rect.x
                + (point.x - self.destination_rect.x) / self.destination_rect.width
                    * self.source_rect.width,
            self.source_rect.y
                + (point.y - self.destination_rect.y) / self.destination_rect.height
                    * self.source_rect.height,
        ))
    }

    fn forward_rect(self, rect: GameViewRect) -> Result<GameViewRect, GameViewPresentationError> {
        if !rect.x.is_finite()
            || !rect.y.is_finite()
            || !rect.width.is_finite()
            || !rect.height.is_finite()
            || rect.width < 0.0
            || rect.height < 0.0
            || rect.x < self.source_rect.x
            || rect.y < self.source_rect.y
            || rect.x + rect.width > self.source_rect.x + self.source_rect.width
            || rect.y + rect.height > self.source_rect.y + self.source_rect.height
        {
            return Err(GameViewPresentationError::new(
                "game_view.presentation.point_outside_content",
            ));
        }
        Ok(GameViewRect::new(
            self.destination_rect.x
                + (rect.x - self.source_rect.x) / self.source_rect.width
                    * self.destination_rect.width,
            self.destination_rect.y
                + (rect.y - self.source_rect.y) / self.source_rect.height
                    * self.destination_rect.height,
            rect.width / self.source_rect.width * self.destination_rect.width,
            rect.height / self.source_rect.height * self.destination_rect.height,
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedGameViewPresentation {
    pub identity: GameViewPresentationIdentity,
    pub target_extent: GameViewExtent,
    pub display_slot_rect: GameViewRect,
    pub display_content_rect: GameViewRect,
    pub scale_policy: GameViewScalePolicy,
    target_to_display: AxisAlignedTransform,
    canvas_to_target: BTreeMap<String, (GameViewExtent, AxisAlignedTransform)>,
}

impl ResolvedGameViewPresentation {
    pub fn canvas_reference_extent(&self, canvas_id: &str) -> Option<GameViewExtent> {
        self.canvas_to_target
            .get(canvas_id)
            .map(|(extent, _)| *extent)
    }

    pub fn canvas_target_content_rect(&self, canvas_id: &str) -> Option<GameViewRect> {
        self.canvas_to_target
            .get(canvas_id)
            .map(|(_, transform)| transform.destination_rect)
    }

    pub fn reference_to_target(
        &self,
        canvas_id: &str,
        point: GameViewPoint,
    ) -> Result<GameViewPoint, GameViewPresentationError> {
        self.canvas_transform(canvas_id)?.forward(point)
    }

    pub fn reference_rect_to_target(
        &self,
        canvas_id: &str,
        rect: GameViewRect,
    ) -> Result<GameViewRect, GameViewPresentationError> {
        self.canvas_transform(canvas_id)?.forward_rect(rect)
    }

    pub fn target_to_reference(
        &self,
        canvas_id: &str,
        point: GameViewPoint,
    ) -> Result<GameViewPoint, GameViewPresentationError> {
        self.canvas_transform(canvas_id)?.inverse(point)
    }

    pub fn target_to_display(
        &self,
        point: GameViewPoint,
    ) -> Result<GameViewPoint, GameViewPresentationError> {
        self.target_to_display.forward(point)
    }

    pub fn display_to_target(
        &self,
        point: GameViewPoint,
    ) -> Result<GameViewPoint, GameViewPresentationError> {
        self.target_to_display.inverse(point)
    }

    pub fn reference_to_display(
        &self,
        canvas_id: &str,
        point: GameViewPoint,
    ) -> Result<GameViewPoint, GameViewPresentationError> {
        self.target_to_display(self.reference_to_target(canvas_id, point)?)
    }

    pub fn display_to_reference(
        &self,
        canvas_id: &str,
        point: GameViewPoint,
    ) -> Result<GameViewPoint, GameViewPresentationError> {
        self.target_to_reference(canvas_id, self.display_to_target(point)?)
    }

    fn canvas_transform(
        &self,
        canvas_id: &str,
    ) -> Result<AxisAlignedTransform, GameViewPresentationError> {
        self.canvas_to_target
            .get(canvas_id)
            .map(|(_, transform)| *transform)
            .ok_or_else(|| {
                GameViewPresentationError::for_canvas(
                    "game_view.presentation.canvas_missing",
                    canvas_id,
                )
            })
    }
}

pub struct GameViewPresentationModule;

impl GameViewPresentationModule {
    pub fn resolve(
        spec: GameViewPresentationSpec,
    ) -> Result<ResolvedGameViewPresentation, GameViewPresentationError> {
        let target_extent = spec.target_extent.validate_target()?;
        let display_slot_rect = spec.display_rect.validate_display()?;
        let target_rect = GameViewRect::from_extent(target_extent);
        let target_to_display =
            AxisAlignedTransform::resolve(target_rect, display_slot_rect, spec.scale_policy);
        let mut canvas_to_target = BTreeMap::new();
        for fact in &spec.canvas_references {
            if fact.canvas_id.trim().is_empty() {
                return Err(GameViewPresentationError::new(
                    "game_view.presentation.canvas_missing",
                ));
            }
            let extent = fact.reference_extent.validate_canvas()?;
            if let Some((existing, _)) = canvas_to_target.get(&fact.canvas_id) {
                if *existing != extent {
                    return Err(GameViewPresentationError::for_canvas(
                        "game_view.presentation.canvas_extent_conflict",
                        &fact.canvas_id,
                    ));
                }
                continue;
            }
            let transform = AxisAlignedTransform::resolve(
                GameViewRect::from_extent(extent),
                target_rect,
                spec.scale_policy,
            );
            canvas_to_target.insert(fact.canvas_id.clone(), (extent, transform));
        }
        let compact_digest = presentation_digest(&spec, &canvas_to_target);
        Ok(ResolvedGameViewPresentation {
            identity: GameViewPresentationIdentity {
                session_id: spec.session_id,
                target_id: spec.target_id,
                surface_generation: spec.surface_generation,
                presentation_revision: spec.presentation_revision,
                compact_digest,
            },
            target_extent,
            display_slot_rect,
            display_content_rect: target_to_display.destination_rect,
            scale_policy: spec.scale_policy,
            target_to_display,
            canvas_to_target,
        })
    }
}

fn presentation_digest(
    spec: &GameViewPresentationSpec,
    canvases: &BTreeMap<String, (GameViewExtent, AxisAlignedTransform)>,
) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    feed(spec.session_id.as_bytes());
    feed(spec.target_id.as_bytes());
    feed(&spec.target_extent.width.to_le_bytes());
    feed(&spec.target_extent.height.to_le_bytes());
    feed(&spec.display_rect.x.to_bits().to_le_bytes());
    feed(&spec.display_rect.y.to_bits().to_le_bytes());
    feed(&spec.display_rect.width.to_bits().to_le_bytes());
    feed(&spec.display_rect.height.to_bits().to_le_bytes());
    feed(&spec.surface_generation.to_le_bytes());
    feed(&spec.presentation_revision.to_le_bytes());
    feed(&[match spec.scale_policy {
        GameViewScalePolicy::Contain => 0,
        GameViewScalePolicy::Stretch => 1,
    }]);
    for (canvas_id, (extent, _)) in canvases {
        feed(canvas_id.as_bytes());
        feed(&extent.width.to_le_bytes());
        feed(&extent.height.to_le_bytes());
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(
        target: GameViewExtent,
        display: GameViewRect,
        policy: GameViewScalePolicy,
        canvases: Vec<CanvasReferenceFact>,
    ) -> ResolvedGameViewPresentation {
        GameViewPresentationModule::resolve(GameViewPresentationSpec {
            session_id: "session".to_string(),
            target_id: "target".to_string(),
            target_extent: target,
            display_rect: display,
            scale_policy: policy,
            surface_generation: 3,
            presentation_revision: 7,
            canvas_references: canvases,
        })
        .unwrap()
    }

    fn assert_point(actual: GameViewPoint, expected: GameViewPoint) {
        assert!((actual.x - expected.x).abs() < 0.001, "x={}", actual.x);
        assert!((actual.y - expected.y).abs() < 0.001, "y={}", actual.y);
    }

    #[test]
    fn game_view_presentation_identity_and_portrait_round_trip() {
        let presentation = resolve(
            GameViewExtent::new(720, 1280),
            GameViewRect::new(10.0, 20.0, 360.0, 640.0),
            GameViewScalePolicy::Contain,
            vec![CanvasReferenceFact::new("main", 1080, 1920)],
        );
        let reference = GameViewPoint::new(451.0, 1779.0);
        let target = presentation.reference_to_target("main", reference).unwrap();
        assert_point(target, GameViewPoint::new(300.666_66, 1186.0));
        let display = presentation.target_to_display(target).unwrap();
        assert_point(display, GameViewPoint::new(160.333_33, 613.0));
        assert_point(
            presentation.display_to_reference("main", display).unwrap(),
            reference,
        );
        assert_eq!(presentation.identity.presentation_revision, 7);
        assert!(presentation.identity.compact_digest.starts_with("fnv1a64:"));
    }

    #[test]
    fn game_view_presentation_contain_rejects_target_and_display_gutters() {
        let presentation = resolve(
            GameViewExtent::new(1280, 720),
            GameViewRect::new(0.0, 0.0, 1000.0, 1000.0),
            GameViewScalePolicy::Contain,
            vec![CanvasReferenceFact::new("portrait", 1080, 1920)],
        );
        assert_eq!(
            presentation.canvas_target_content_rect("portrait").unwrap(),
            GameViewRect::new(437.5, 0.0, 405.0, 720.0)
        );
        assert_eq!(
            presentation
                .target_to_reference("portrait", GameViewPoint::new(100.0, 100.0))
                .unwrap_err()
                .code,
            "game_view.presentation.point_outside_content"
        );
        assert_eq!(
            presentation
                .display_to_target(GameViewPoint::new(10.0, 10.0))
                .unwrap_err()
                .code,
            "game_view.presentation.point_outside_content"
        );
    }

    #[test]
    fn game_view_presentation_stretch_is_non_uniform_and_invertible() {
        let presentation = resolve(
            GameViewExtent::new(1280, 720),
            GameViewRect::new(0.0, 0.0, 640.0, 720.0),
            GameViewScalePolicy::Stretch,
            vec![CanvasReferenceFact::new("main", 1080, 1920)],
        );
        let point = GameViewPoint::new(540.0, 960.0);
        assert_point(
            presentation.reference_to_target("main", point).unwrap(),
            GameViewPoint::new(640.0, 360.0),
        );
        assert_point(
            presentation.reference_to_display("main", point).unwrap(),
            GameViewPoint::new(320.0, 360.0),
        );
    }

    #[test]
    fn game_view_presentation_uses_left_top_inclusive_right_bottom_exclusive() {
        let presentation = resolve(
            GameViewExtent::new(1080, 1920),
            GameViewRect::new(0.0, 0.0, 1080.0, 1920.0),
            GameViewScalePolicy::Contain,
            vec![CanvasReferenceFact::new("main", 1080, 1920)],
        );
        assert!(presentation
            .reference_to_target("main", GameViewPoint::new(0.0, 0.0))
            .is_ok());
        assert!(presentation
            .reference_to_target("main", GameViewPoint::new(1080.0, 10.0))
            .is_err());
        assert!(presentation
            .reference_to_target("main", GameViewPoint::new(10.0, 1920.0))
            .is_err());
    }

    #[test]
    fn game_view_presentation_maps_distinct_canvases_independently() {
        let presentation = resolve(
            GameViewExtent::new(720, 1280),
            GameViewRect::new(0.0, 0.0, 720.0, 1280.0),
            GameViewScalePolicy::Contain,
            vec![
                CanvasReferenceFact::new("portrait", 1080, 1920),
                CanvasReferenceFact::new("square", 1000, 1000),
            ],
        );
        assert_point(
            presentation
                .reference_to_target("portrait", GameViewPoint::new(540.0, 960.0))
                .unwrap(),
            GameViewPoint::new(360.0, 640.0),
        );
        assert_point(
            presentation
                .reference_to_target("square", GameViewPoint::new(500.0, 500.0))
                .unwrap(),
            GameViewPoint::new(360.0, 640.0),
        );
        assert_eq!(
            presentation
                .target_to_reference("missing", GameViewPoint::new(1.0, 1.0))
                .unwrap_err()
                .code,
            "game_view.presentation.canvas_missing"
        );
    }

    #[test]
    fn game_view_presentation_rejects_invalid_and_conflicting_facts() {
        let base = GameViewPresentationSpec {
            session_id: "session".to_string(),
            target_id: "target".to_string(),
            target_extent: GameViewExtent::new(0, 720),
            display_rect: GameViewRect::new(0.0, 0.0, 100.0, 100.0),
            scale_policy: GameViewScalePolicy::Contain,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: vec![],
        };
        assert_eq!(
            GameViewPresentationModule::resolve(base.clone())
                .unwrap_err()
                .code,
            "game_view.presentation.target_extent_invalid"
        );
        let mut conflict = base;
        conflict.target_extent = GameViewExtent::new(720, 1280);
        conflict.canvas_references = vec![
            CanvasReferenceFact::new("main", 1080, 1920),
            CanvasReferenceFact::new("main", 720, 1280),
        ];
        assert_eq!(
            GameViewPresentationModule::resolve(conflict)
                .unwrap_err()
                .code,
            "game_view.presentation.canvas_extent_conflict"
        );
    }
}
