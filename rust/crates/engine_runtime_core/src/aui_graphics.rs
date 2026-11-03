//! Production path tessellation and bounded retained geometry. Uses the existing
//! custom-mesh UI projection; callers never receive GPU or renderer internals.
use lyon_tessellation::geometry_builder::{
    FillGeometryBuilder, GeometryBuilder, GeometryBuilderError, StrokeGeometryBuilder,
};
use lyon_tessellation::math::point;
use lyon_tessellation::path::iterator::PathIterator;
use lyon_tessellation::path::Path;
use lyon_tessellation::{
    FillOptions, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator, StrokeVertex,
    VertexId,
};
use project_runtime_sdk::aui_draw2d::{
    Draw2DItem, FillRule, Graphics2D, LineCap, LineJoin, Path2D, PathCommand,
};
use project_runtime_sdk::aui_extension::{
    UiExtensionMeshVertex, UiExtensionPaintOutput, MAX_UI_EXTENSION_INDICES,
    MAX_UI_EXTENSION_VERTICES,
};
use project_runtime_sdk::aui_presentation::{curve_tolerance, DEFAULT_CURVE_ERROR_PIXELS};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

pub const MAX_GRAPHICS_CACHE_ENTRIES: usize = 64;
pub const MAX_GRAPHICS_CACHE_BYTES: usize = 8 * 1024 * 1024;
const MAX_FLATTENED_EVENTS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsError {
    pub code: &'static str,
    pub item: Option<usize>,
    pub message: String,
}
impl GraphicsError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            item: None,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for GraphicsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} item {:?}: {}", self.code, self.item, self.message)
    }
}
impl std::error::Error for GraphicsError {}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphicsGeometry {
    pub mesh: crate::aui_retained::AuiMeshPayload,
    pub tolerance: f32,
}

struct BoundedGeometry {
    mesh: UiExtensionPaintOutput,
    color: [f32; 4],
    failed: bool,
}
impl BoundedGeometry {
    fn new(color: [f32; 4]) -> Self {
        Self {
            mesh: UiExtensionPaintOutput {
                vertices: Vec::new(),
                indices: Vec::new(),
                asset_id: None,
                primitives: Vec::new(),
            },
            color,
            failed: false,
        }
    }
    fn vertex(&mut self, position: [f32; 2]) -> Result<VertexId, GeometryBuilderError> {
        if self.mesh.vertices.len() >= MAX_UI_EXTENSION_VERTICES
            || position.iter().any(|v| !v.is_finite())
        {
            self.failed = true;
            return Err(GeometryBuilderError::TooManyVertices);
        }
        let id = VertexId(self.mesh.vertices.len() as u32);
        self.mesh.vertices.push(UiExtensionMeshVertex {
            position,
            uv: [0.0; 2],
            color: self.color,
        });
        Ok(id)
    }
}
impl GeometryBuilder for BoundedGeometry {
    fn add_triangle(&mut self, a: VertexId, b: VertexId, c: VertexId) {
        if self.mesh.indices.len() + 3 > MAX_UI_EXTENSION_INDICES {
            self.failed = true;
            return;
        }
        self.mesh.indices.extend([a.0, b.0, c.0]);
    }
    fn abort_geometry(&mut self) {
        self.failed = true;
    }
}
impl FillGeometryBuilder for BoundedGeometry {
    fn add_fill_vertex(
        &mut self,
        vertex: FillVertex<'_>,
    ) -> Result<VertexId, GeometryBuilderError> {
        let p = vertex.position();
        self.vertex([p.x, p.y])
    }
}
impl StrokeGeometryBuilder for BoundedGeometry {
    fn add_stroke_vertex(
        &mut self,
        vertex: StrokeVertex<'_, '_>,
    ) -> Result<VertexId, GeometryBuilderError> {
        let p = vertex.position();
        self.vertex([p.x, p.y])
    }
}

fn path_events(path: &Path2D, tolerance: f32) -> Result<Path, GraphicsError> {
    let mut builder = Path::builder();
    let mut active = false;
    let mut event_count = 0usize;
    for command in path.commands() {
        event_count += 1;
        match *command {
            PathCommand::Move { to } => {
                if active {
                    builder.end(false);
                }
                builder.begin(point(to[0], to[1]));
                active = true;
            }
            PathCommand::Line { to } => {
                builder.line_to(point(to[0], to[1]));
            }
            PathCommand::Quadratic { control, to } => {
                builder.quadratic_bezier_to(point(control[0], control[1]), point(to[0], to[1]));
            }
            PathCommand::Cubic {
                control1,
                control2,
                to,
            } => {
                builder.cubic_bezier_to(
                    point(control1[0], control1[1]),
                    point(control2[0], control2[1]),
                    point(to[0], to[1]),
                );
            }
            PathCommand::Arc {
                center,
                radii,
                start,
                sweep,
            } => {
                // Angle step bounds ellipse error by the maximum axis; use f64 to avoid
                // cancellation for small errors and large radii. Never silently lower quality.
                let radius = f64::from(radii[0].max(radii[1]));
                let step = if radius == 0.0 {
                    std::f64::consts::TAU
                } else {
                    (4.0 * (f64::from(tolerance) / (2.0 * radius))
                        .min(1.0)
                        .sqrt()
                        .asin())
                    .min(std::f64::consts::FRAC_PI_2)
                };
                let segments = (f64::from(sweep.abs()) / step).ceil().max(1.0);
                if !segments.is_finite() || segments > MAX_FLATTENED_EVENTS as f64 {
                    return Err(GraphicsError::new(
                        "aui_graphics.capacity",
                        "arc precision exceeds event budget",
                    ));
                }
                let count = segments as usize;
                event_count += count + 1;
                if event_count > MAX_FLATTENED_EVENTS {
                    return Err(GraphicsError::new(
                        "aui_graphics.capacity",
                        "flattened path budget exceeded",
                    ));
                }
                for i in 0..=count {
                    let angle = if i == count && sweep.abs() == std::f32::consts::TAU {
                        f64::from(start)
                    } else {
                        f64::from(start) + f64::from(sweep) * i as f64 / count as f64
                    };
                    let x = f64::from(center[0]) + f64::from(radii[0]) * angle.cos();
                    let y = f64::from(center[1]) + f64::from(radii[1]) * angle.sin();
                    builder.line_to(point(x as f32, y as f32));
                }
            }
            PathCommand::Close => {
                builder.end(true);
                active = false;
            }
        }
    }
    if active {
        builder.end(false);
    }
    // Pre-flatten curves with a hard budget, then feed the same bounded line path to lyon.
    let mut flat = Path::builder();
    for (index, event) in builder.build().iter().flattened(tolerance).enumerate() {
        if index >= MAX_FLATTENED_EVENTS {
            return Err(GraphicsError::new(
                "aui_graphics.capacity",
                "curve precision exceeds event budget",
            ));
        }
        match event {
            lyon_tessellation::path::Event::Begin { at } => {
                flat.begin(at);
            }
            lyon_tessellation::path::Event::Line { to, .. } => {
                flat.line_to(to);
            }
            lyon_tessellation::path::Event::End { close, .. } => {
                flat.end(close);
            }
            _ => unreachable!("flattened path contains only linear events"),
        }
    }
    Ok(flat.build())
}

/// One logical pixel coverage ramp outside triangulated boundaries. Alpha is carried
/// per vertex by the existing UI pass. Interior edges are removed, including holes.
fn add_coverage_fringe(mesh: &mut UiExtensionPaintOutput, width: f32) -> Result<(), GraphicsError> {
    let mut edges = BTreeMap::<(u32, u32), (usize, u32, u32)>::new();
    // Weld positions: stroke tessellation may emit distinct IDs at a shared seam.
    let mut positions = BTreeMap::<(u32, u32), u32>::new();
    let mut welded = Vec::new();
    for (index, vertex) in mesh.vertices.iter().enumerate() {
        let key = (
            if vertex.position[0] == 0.0 {
                0
            } else {
                vertex.position[0].to_bits()
            },
            if vertex.position[1] == 0.0 {
                0
            } else {
                vertex.position[1].to_bits()
            },
        );
        welded.push(*positions.entry(key).or_insert(index as u32));
    }
    for tri in mesh.indices.chunks_exact(3) {
        let mut ids = [
            welded[tri[0] as usize],
            welded[tri[1] as usize],
            welded[tri[2] as usize],
        ];
        let p = ids.map(|id| mesh.vertices[id as usize].position);
        let cross =
            (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
        if cross == 0.0 {
            continue;
        }
        if cross < 0.0 {
            ids.swap(1, 2);
        }
        for (a, b) in [(ids[0], ids[1]), (ids[1], ids[2]), (ids[2], ids[0])] {
            let edge = edges.entry((a.min(b), a.max(b))).or_insert((0, a, b));
            edge.0 += 1;
        }
    }
    let boundary: Vec<_> = edges
        .values()
        .filter(|edge| edge.0 == 1)
        .map(|edge| (edge.1, edge.2))
        .collect();
    let mut normals = BTreeMap::<u32, Vec<[f32; 2]>>::new();
    for &(a, b) in &boundary {
        let pa = mesh.vertices[a as usize].position;
        let pb = mesh.vertices[b as usize].position;
        let dx = pb[0] - pa[0];
        let dy = pb[1] - pa[1];
        let length = dx.hypot(dy);
        if length > 0.0 {
            let n = [dy / length, -dx / length];
            normals.entry(a).or_default().push(n);
            normals.entry(b).or_default().push(n);
        }
    }
    if mesh.vertices.len() + normals.len() > MAX_UI_EXTENSION_VERTICES
        || mesh.indices.len() + boundary.len() * 6 > MAX_UI_EXTENSION_INDICES
    {
        return Err(GraphicsError::new(
            "aui_graphics.capacity",
            "coverage geometry exceeds mesh budget",
        ));
    }
    let mut outer = BTreeMap::new();
    for (id, ns) in normals {
        let sum = ns.iter().fold([0.0; 2], |s, n| [s[0] + n[0], s[1] + n[1]]);
        let len = sum[0].hypot(sum[1]);
        let dir = if len > 1e-6 {
            [sum[0] / len, sum[1] / len]
        } else {
            ns[0]
        };
        let dot = (dir[0] * ns[0][0] + dir[1] * ns[0][1]).abs().max(0.25);
        let mut vertex = mesh.vertices[id as usize];
        vertex.position[0] += dir[0] * width / dot;
        vertex.position[1] += dir[1] * width / dot;
        vertex.color[3] = 0.0;
        if !vertex.position.iter().all(|v| v.is_finite()) {
            return Err(GraphicsError::new(
                "aui_graphics.nonfinite",
                "coverage vertex overflow",
            ));
        }
        outer.insert(id, mesh.vertices.len() as u32);
        mesh.vertices.push(vertex);
    }
    for (a, b) in boundary {
        if let (Some(&oa), Some(&ob)) = (outer.get(&a), outer.get(&b)) {
            mesh.indices.extend([a, b, ob, a, ob, oa]);
        }
    }
    Ok(())
}

pub fn tessellate_graphics(
    graphics: &Graphics2D,
    pixel_scale: f32,
) -> Result<GraphicsGeometry, GraphicsError> {
    graphics
        .validate()
        .map_err(|e| GraphicsError::new("aui_graphics.invalid", e.to_string()))?;
    let tolerance = curve_tolerance(f64::from(pixel_scale), 1.0, DEFAULT_CURVE_ERROR_PIXELS)
        .map_err(|e| GraphicsError::new("aui_graphics.precision", e.to_string()))?;
    let mut output = BoundedGeometry::new([1.0; 4]).mesh;
    for (item_index, item) in graphics.items().iter().enumerate() {
        let result = (|| {
            let (source, color) = match item {
                Draw2DItem::Fill { path, color, .. } => (path, *color),
                Draw2DItem::Stroke { path, style } => (path, style.color),
            };
            let path = path_events(source, tolerance)?;
            let mut builder = BoundedGeometry::new(color);
            match item {
                Draw2DItem::Fill { rule, .. } => {
                    let options = FillOptions::default()
                        .with_tolerance(tolerance)
                        .with_fill_rule(match rule {
                            FillRule::NonZero => lyon_tessellation::FillRule::NonZero,
                            FillRule::EvenOdd => lyon_tessellation::FillRule::EvenOdd,
                        });
                    FillTessellator::new()
                        .tessellate_path(&path, &options, &mut builder)
                        .map_err(|e| {
                            GraphicsError::new("aui_graphics.tessellation", e.to_string())
                        })?;
                }
                Draw2DItem::Stroke { style, .. } if style.width > 0.0 => {
                    let cap = match style.cap {
                        LineCap::Butt => lyon_tessellation::LineCap::Butt,
                        LineCap::Round => lyon_tessellation::LineCap::Round,
                        LineCap::Square => lyon_tessellation::LineCap::Square,
                    };
                    let join = match style.join {
                        LineJoin::Bevel => lyon_tessellation::LineJoin::Bevel,
                        LineJoin::Round => lyon_tessellation::LineJoin::Round,
                        LineJoin::Miter => lyon_tessellation::LineJoin::Miter,
                    };
                    let options = StrokeOptions::default()
                        .with_tolerance(tolerance)
                        .with_line_width(style.width)
                        .with_line_cap(cap)
                        .with_line_join(join)
                        .with_miter_limit(style.miter_limit);
                    StrokeTessellator::new()
                        .tessellate_path(&path, &options, &mut builder)
                        .map_err(|e| {
                            GraphicsError::new("aui_graphics.tessellation", e.to_string())
                        })?;
                }
                _ => {}
            }
            if builder.failed {
                return Err(GraphicsError::new(
                    "aui_graphics.capacity",
                    "tessellation exceeds mesh capacity",
                ));
            }
            add_coverage_fringe(&mut builder.mesh, 1.0 / pixel_scale)?;
            if output.vertices.len() + builder.mesh.vertices.len() > MAX_UI_EXTENSION_VERTICES
                || output.indices.len() + builder.mesh.indices.len() > MAX_UI_EXTENSION_INDICES
            {
                return Err(GraphicsError::new(
                    "aui_graphics.capacity",
                    "aggregate graphics exceeds mesh capacity",
                ));
            }
            let base = output.vertices.len() as u32;
            output.vertices.extend(builder.mesh.vertices);
            output
                .indices
                .extend(builder.mesh.indices.iter().map(|index| index + base));
            Ok(())
        })();
        if let Err(mut error) = result {
            error.item = Some(item_index);
            return Err(error);
        }
    }
    Ok(GraphicsGeometry {
        mesh: crate::aui_retained::AuiMeshPayload::graphics(output),
        tolerance,
    })
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GraphicsCache {
    entries: VecDeque<(Graphics2D, u32, Arc<GraphicsGeometry>)>,
    bytes: usize,
    pub builds: u64,
    pub hits: u64,
}
impl GraphicsCache {
    /// Scale buckets round upward: reuse cannot lower requested pixel precision.
    pub fn geometry(
        &mut self,
        graphics: &Graphics2D,
        pixel_scale: f32,
    ) -> Result<Arc<GraphicsGeometry>, GraphicsError> {
        if !pixel_scale.is_finite() || pixel_scale <= 0.0 {
            return Err(GraphicsError::new(
                "aui_graphics.precision",
                "pixel scale must be positive and finite",
            ));
        }
        let bucket = pixel_scale
            .log2()
            .mul_add(4.0, 0.0)
            .ceil()
            .clamp(-120.0, 120.0) as i32;
        let scale = 2.0f32.powf(bucket as f32 / 4.0);
        if scale < pixel_scale {
            return Err(GraphicsError::new(
                "aui_graphics.precision",
                "pixel scale exceeds supported precision range",
            ));
        }
        let key = scale.to_bits();
        if let Some(entry) = self
            .entries
            .iter()
            .find(|(value, precision, _)| value == graphics && *precision == key)
        {
            self.hits += 1;
            return Ok(entry.2.clone());
        }
        let geometry = Arc::new(tessellate_graphics(graphics, scale)?);
        let bytes = geometry.mesh.vertices.len()
            * (std::mem::size_of::<UiExtensionMeshVertex>()
                + std::mem::size_of::<crate::render_graph::RenderDrawVertex>())
            + geometry.mesh.indices.len() * 8
            + serde_json::to_vec(graphics)
                .map_err(|e| GraphicsError::new("aui_graphics.invalid", e.to_string()))?
                .len();
        while !self.entries.is_empty()
            && (self.entries.len() >= MAX_GRAPHICS_CACHE_ENTRIES
                || self.bytes + bytes > MAX_GRAPHICS_CACHE_BYTES)
        {
            if let Some((description, _, old)) = self.entries.pop_front() {
                self.bytes -= old.mesh.vertices.len()
                    * (std::mem::size_of::<UiExtensionMeshVertex>()
                        + std::mem::size_of::<crate::render_graph::RenderDrawVertex>())
                    + old.mesh.indices.len() * 8
                    + serde_json::to_vec(&description)
                        .expect("validated graphics serialize")
                        .len();
            }
        }
        self.bytes += bytes;
        self.entries
            .push_back((graphics.clone(), key, geometry.clone()));
        self.builds += 1;
        Ok(geometry)
    }
    pub fn resident_bytes(&self) -> usize {
        self.bytes
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use project_runtime_sdk::aui_draw2d::{Draw2D, StrokeStyle};
    fn fill(path: Path2D) -> Graphics2D {
        let mut draw = Draw2D::new();
        draw.fill(path, [0.25, 0.5, 1.0, 0.6], FillRule::EvenOdd)
            .unwrap();
        draw.finish()
    }
    #[test]
    fn aui_graphics_empty_degenerate_and_zero_strokes_have_no_fallback() {
        for path in [Path2D::default(), Path2D::line([1., 2.], [1., 2.]).unwrap()] {
            assert!(tessellate_graphics(&fill(path), 1.0)
                .unwrap()
                .mesh
                .indices
                .is_empty());
        }
        let mut draw = Draw2D::new();
        draw.stroke(
            Path2D::line([0., 0.], [30., 0.]).unwrap(),
            StrokeStyle {
                width: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(tessellate_graphics(&draw.finish(), 1.0)
            .unwrap()
            .mesh
            .indices
            .is_empty());
    }
    #[test]
    fn aui_graphics_caps_joins_curves_produce_finite_bounded_geometry() {
        for cap in [LineCap::Butt, LineCap::Round, LineCap::Square] {
            for join in [LineJoin::Miter, LineJoin::Bevel, LineJoin::Round] {
                let mut draw = Draw2D::new();
                draw.stroke(
                    Path2D::polygon(&[[0., 0.], [30., 0.], [30., 30.]], false).unwrap(),
                    StrokeStyle {
                        width: 6.,
                        cap,
                        join,
                        ..Default::default()
                    },
                )
                .unwrap();
                let mesh = tessellate_graphics(&draw.finish(), 1.5).unwrap().mesh;
                assert!(!mesh.indices.is_empty());
                assert!(mesh
                    .vertices
                    .iter()
                    .all(|v| v.position.iter().all(|p| p.is_finite())));
                assert!(mesh.vertices.iter().any(|v| v.color[3] == 0.0));
                assert!(mesh.vertices.iter().any(|v| v.color[3] == 1.0));
            }
        }
        let circle = fill(Path2D::circle([50., 50.], 40.).unwrap());
        let a = tessellate_graphics(&circle, 1.).unwrap();
        let b = tessellate_graphics(&circle, 2.).unwrap();
        assert!(b.mesh.vertices.len() > a.mesh.vertices.len());
        assert_eq!(b.tolerance, 0.125);
    }
    #[test]
    fn aui_graphics_capacity_failure_is_atomic_and_not_cached() {
        let mut draw = Draw2D::new();
        for i in 0..256 {
            draw.fill(
                Path2D::circle([i as f32, 0.], 100.).unwrap(),
                [1.; 4],
                FillRule::NonZero,
            )
            .unwrap();
        }
        let mut cache = GraphicsCache::default();
        let error = cache.geometry(&draw.finish(), 2.).unwrap_err();
        assert_eq!(error.code, "aui_graphics.capacity");
        assert!(error.item.is_some());
        assert_eq!((cache.builds, cache.resident_bytes()), (0, 0));
        assert!(cache.geometry(&Graphics2D::default(), f32::NAN).is_err());
    }
    #[test]
    fn aui_graphics_extreme_finite_coordinates_fail_without_panicking() {
        let mut draw = Draw2D::new();
        draw.stroke(
            Path2D::line([-f32::MAX, 0.], [f32::MAX, 0.]).unwrap(),
            StrokeStyle::default(),
        )
        .unwrap();
        assert!(tessellate_graphics(&draw.finish(), 1.).is_err());
    }
    #[test]
    fn aui_graphics_retention_precision_replacement_and_release_are_bounded() {
        let graphics = fill(Path2D::circle([0., 0.], 40.).unwrap());
        let mut cache = GraphicsCache::default();
        let first = cache.geometry(&graphics, 1.).unwrap();
        assert!(Arc::ptr_eq(&first, &cache.geometry(&graphics, 1.).unwrap()));
        assert!(!Arc::ptr_eq(
            &first,
            &cache.geometry(&graphics, 1.5).unwrap()
        ));
        assert!(Arc::ptr_eq(&first, &cache.geometry(&graphics, 1.).unwrap()));
        for i in 0..80 {
            cache
                .geometry(
                    &fill(Path2D::rectangle([i as f32, 0., 10., 10.]).unwrap()),
                    1.,
                )
                .unwrap();
        }
        assert_eq!(cache.entries.len(), MAX_GRAPHICS_CACHE_ENTRIES);
        assert!(cache.resident_bytes() <= MAX_GRAPHICS_CACHE_BYTES);
        cache.clear();
        assert_eq!(cache.resident_bytes(), 0);
        assert!(!Arc::ptr_eq(
            &first,
            &cache.geometry(&graphics, 1.).unwrap()
        ));
    }
}
