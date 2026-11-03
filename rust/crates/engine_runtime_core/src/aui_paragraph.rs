//! Cached paragraph shaping and basic multiline layout for C2 AUI.
use crate::aui::{
    AuiComputedRect, AuiDocument, AuiFontRasterPolicy, AuiFontStyle, AuiFontStyleKind, AuiNode,
};
use crate::aui_layout_runtime::{
    AuiAvailableSpace, AuiIntrinsicMeasure, AuiMeasureInput, AuiMeasureOutput,
};
use crate::aui_layout_schema::{
    AuiRichTextSpan, LineHeight, TextAlign, TextDirection, TextLayoutSpec, TextOverflow, TextWrap,
    VerticalAlign,
};
use crate::canonical_digest::sha256_prefixed;
use crate::font_bundle::{
    FontBundleStyle, RuntimeFontBundleRegistry, RuntimeLoadedFontBundle,
    RuntimePackageSourceFontFace,
};
use crate::text_shaping::{shape_stack, GlyphRun};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use unicode_bidi::{BidiInfo, Level, ParagraphInfo};
use unicode_linebreak::{linebreaks, BreakOpportunity};

pub const DEFAULT_PARAGRAPH_CACHE_BYTES: usize = 32 * 1024 * 1024;
pub const DEFAULT_PARAGRAPH_CACHE_ENTRIES: usize = 4096;
pub const MAX_PARAGRAPH_SOURCE_BYTES: usize = 64 * 1024;
pub const MAX_PARAGRAPH_LINES: usize = 4096;

#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphGlyph {
    pub font_bundle_id: String,
    pub face_id: String,
    pub glyph_id: u32,
    pub cluster_start: u32,
    pub cluster_end: u32,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    pub has_outline: bool,
    pub ink_bounds: Option<[f32; 4]>,
    pub fallback_used: bool,
    pub font_size: f32,
    pub raster_policy: AuiFontRasterPolicy,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParagraphVisualRun {
    pub source_start: u32,
    pub source_end: u32,
    pub embedding_level: u8,
    pub rtl: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphLine {
    pub source_start: u32,
    pub source_end: u32,
    pub glyph_start: usize,
    pub glyph_end: usize,
    pub visual_run_order: Vec<ParagraphVisualRun>,
    pub baseline: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphLayout {
    pub identity: String,
    pub text_digest: String,
    pub font_identity: String,
    pub constraint_width: f32,
    pub width: f32,
    pub height: f32,
    pub first_baseline: Option<f32>,
    pub lines: Vec<ParagraphLine>,
    pub glyphs: Vec<ParagraphGlyph>,
    pub truncated: bool,
    pub overflow: TextOverflow,
}

impl ParagraphLayout {
    fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.identity.len()
            + self.text_digest.len()
            + self.font_identity.len()
            + self.glyphs.len() * std::mem::size_of::<ParagraphGlyph>()
            + self
                .glyphs
                .iter()
                .map(|glyph| {
                    glyph.face_id.len()
                        + glyph.font_bundle_id.len()
                        + glyph.color.as_ref().map_or(0, String::len)
                })
                .sum::<usize>()
            + self.lines.len() * std::mem::size_of::<ParagraphLine>()
    }

    pub fn hit_test(&self, x: f32, y: f32) -> ParagraphHit {
        let Some((line_index, line)) =
            self.lines
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| {
                    distance_to_line(left, y).total_cmp(&distance_to_line(right, y))
                })
        else {
            return ParagraphHit::default();
        };
        let groups = line_cluster_geometries(self, line);
        if groups.is_empty() {
            return ParagraphHit {
                byte_offset: line.source_start,
                affinity: ParagraphHitAffinity::Downstream,
                line_index,
            };
        }
        let group = groups
            .iter()
            .min_by(|left, right| {
                distance_to_interval(left.x0, left.x1, x)
                    .total_cmp(&distance_to_interval(right.x0, right.x1, x))
            })
            .expect("non-empty cluster geometry");
        let before_visual_midpoint = x <= (group.x0 + group.x1) * 0.5;
        let byte_offset = match (group.rtl, before_visual_midpoint) {
            (false, true) | (true, false) => group.start,
            (false, false) | (true, true) => group.end,
        };
        ParagraphHit {
            byte_offset,
            affinity: if before_visual_midpoint {
                ParagraphHitAffinity::Upstream
            } else {
                ParagraphHitAffinity::Downstream
            },
            line_index,
        }
    }

    pub fn caret_rect(&self, byte_offset: u32) -> Option<AuiComputedRect> {
        for line in &self.lines {
            let groups = line_cluster_geometries(self, line);
            for group in groups {
                let x = if byte_offset == group.start {
                    if group.rtl {
                        group.x1
                    } else {
                        group.x0
                    }
                } else if byte_offset == group.end {
                    if group.rtl {
                        group.x0
                    } else {
                        group.x1
                    }
                } else {
                    continue;
                };
                return Some(AuiComputedRect {
                    x,
                    y: line.baseline - line.height,
                    width: 1.0,
                    height: line.height,
                });
            }
            if byte_offset == line.source_start || byte_offset == line.source_end {
                return Some(AuiComputedRect {
                    x: 0.0,
                    y: line.baseline - line.height,
                    width: 1.0,
                    height: line.height,
                });
            }
        }
        None
    }

    pub fn selection_rects(&self, start: u32, end: u32) -> Vec<AuiComputedRect> {
        let (start, end) = (start.min(end), start.max(end));
        self.lines
            .iter()
            .filter_map(|line| {
                let selected = line_cluster_geometries(self, line)
                    .into_iter()
                    .filter(|cluster| cluster.end > start && cluster.start < end)
                    .collect::<Vec<_>>();
                if selected.is_empty() {
                    return None;
                }
                let x0 = selected
                    .iter()
                    .map(|cluster| cluster.x0)
                    .fold(f32::INFINITY, f32::min);
                let x1 = selected
                    .iter()
                    .map(|cluster| cluster.x1)
                    .fold(f32::NEG_INFINITY, f32::max);
                Some(AuiComputedRect {
                    x: x0,
                    y: line.baseline - line.height,
                    width: (x1 - x0).max(0.0),
                    height: line.height,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParagraphHitAffinity {
    Upstream,
    #[default]
    Downstream,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParagraphHit {
    pub byte_offset: u32,
    pub affinity: ParagraphHitAffinity,
    pub line_index: usize,
}

#[derive(Debug, Clone, Copy)]
struct ClusterGeometry {
    start: u32,
    end: u32,
    x0: f32,
    x1: f32,
    rtl: bool,
}

fn line_cluster_geometries(layout: &ParagraphLayout, line: &ParagraphLine) -> Vec<ClusterGeometry> {
    let mut groups = Vec::<ClusterGeometry>::new();
    for glyph in &layout.glyphs[line.glyph_start..line.glyph_end] {
        let rtl = line.visual_run_order.iter().any(|run| {
            run.rtl
                && glyph.cluster_start >= run.source_start
                && glyph.cluster_end <= run.source_end
        });
        let x0 = glyph.x.min(glyph.x + glyph.advance);
        let x1 = glyph.x.max(glyph.x + glyph.advance);
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.start == glyph.cluster_start && group.end == glyph.cluster_end)
        {
            group.x0 = group.x0.min(x0);
            group.x1 = group.x1.max(x1);
        } else {
            groups.push(ClusterGeometry {
                start: glyph.cluster_start,
                end: glyph.cluster_end,
                x0,
                x1,
                rtl,
            });
        }
    }
    groups
}

fn distance_to_line(line: &ParagraphLine, y: f32) -> f32 {
    distance_to_interval(line.baseline - line.height, line.baseline, y)
}

fn distance_to_interval(start: f32, end: f32, value: f32) -> f32 {
    if value < start {
        start - value
    } else if value > end {
        value - end
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuiParagraphCacheStats {
    pub hits: usize,
    pub misses: usize,
    pub reshapes: usize,
    pub evictions: usize,
    pub oversized_not_cached: usize,
}

struct CacheEntry {
    layout: Arc<ParagraphLayout>,
    bytes: usize,
    last_use: u64,
}

pub struct AuiParagraphCache {
    entries: HashMap<String, CacheEntry>,
    byte_budget: usize,
    entry_budget: usize,
    resident_bytes: usize,
    clock: u64,
    stats: AuiParagraphCacheStats,
}

impl Default for AuiParagraphCache {
    fn default() -> Self {
        Self::new(
            DEFAULT_PARAGRAPH_CACHE_BYTES,
            DEFAULT_PARAGRAPH_CACHE_ENTRIES,
        )
    }
}

impl AuiParagraphCache {
    pub fn new(byte_budget: usize, entry_budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            byte_budget,
            entry_budget,
            resident_bytes: 0,
            clock: 0,
            stats: AuiParagraphCacheStats::default(),
        }
    }

    pub fn stats(&self) -> AuiParagraphCacheStats {
        self.stats
    }

    pub fn resident_bytes(&self) -> usize {
        self.resident_bytes
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn get(&mut self, key: &str) -> Option<Arc<ParagraphLayout>> {
        self.clock = self.clock.saturating_add(1);
        let entry = self.entries.get_mut(key)?;
        entry.last_use = self.clock;
        self.stats.hits = self.stats.hits.saturating_add(1);
        Some(Arc::clone(&entry.layout))
    }

    fn insert(&mut self, key: String, layout: Arc<ParagraphLayout>) {
        let bytes = layout.estimated_bytes();
        if self.byte_budget == 0 || self.entry_budget == 0 || bytes > self.byte_budget {
            self.stats.oversized_not_cached = self.stats.oversized_not_cached.saturating_add(1);
            return;
        }
        while self.entries.len() >= self.entry_budget
            || self.resident_bytes.saturating_add(bytes) > self.byte_budget
        {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_use)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.resident_bytes = self.resident_bytes.saturating_sub(entry.bytes);
                self.stats.evictions = self.stats.evictions.saturating_add(1);
            }
        }
        self.clock = self.clock.saturating_add(1);
        self.resident_bytes = self.resident_bytes.saturating_add(bytes);
        self.entries.insert(
            key,
            CacheEntry {
                layout,
                bytes,
                last_use: self.clock,
            },
        );
    }
}

pub struct ParagraphRequest<'a> {
    pub text: &'a str,
    pub font_size: f32,
    pub font: Option<&'a AuiFontStyle>,
    pub spans: &'a [AuiRichTextSpan],
    pub text_layout: &'a TextLayoutSpec,
    pub constraint_width: f32,
    pub box_height: Option<f32>,
    pub font_bundles: &'a RuntimeFontBundleRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParagraphError {
    SourceTooLarge,
    FontBundleMissing,
    FontSourceMissing,
    ShapeFailed,
}

#[derive(Clone)]
struct TempGlyph {
    font_bundle_id: String,
    face_id: String,
    glyph_id: u32,
    cluster_start: usize,
    cluster_end: usize,
    offset_x: f32,
    offset_y: f32,
    advance: f32,
    has_outline: bool,
    ink_bounds: Option<[f32; 4]>,
    fallback_used: bool,
    ascender: f32,
    descender: f32,
    line_gap: f32,
    font_size: f32,
    raster_policy: AuiFontRasterPolicy,
    color: Option<String>,
}

#[derive(Clone)]
struct Cluster {
    start: usize,
    end: usize,
    advance: f32,
    glyphs: Vec<TempGlyph>,
}

pub fn layout_paragraph(
    cache: &mut AuiParagraphCache,
    request: ParagraphRequest<'_>,
) -> Result<Arc<ParagraphLayout>, ParagraphError> {
    if request.text.len() > MAX_PARAGRAPH_SOURCE_BYTES {
        return Err(ParagraphError::SourceTooLarge);
    }
    let bundle = select_bundle(request.font_bundles, request.font)?;
    let sources = sorted_sources(bundle, request.font);
    if sources.is_empty() {
        return Err(ParagraphError::FontSourceMissing);
    }
    let registry_identity = request
        .font_bundles
        .bundles_by_id
        .iter()
        .map(|(id, bundle)| format!("{id}:{}", bundle.metadata.bundle_digest))
        .collect::<Vec<_>>()
        .join(",");
    let font_identity = format!(
        "{}:{}:{}:{}",
        bundle.metadata.font_bundle_id,
        bundle.metadata.bundle_digest,
        sources
            .iter()
            .map(|source| source.source_digest.as_str())
            .collect::<Vec<_>>()
            .join(","),
        registry_identity,
    );
    let key = paragraph_key(&request, &font_identity);
    if let Some(layout) = cache.get(&key) {
        return Ok(layout);
    }
    cache.stats.misses = cache.stats.misses.saturating_add(1);
    cache.stats.reshapes = cache.stats.reshapes.saturating_add(1);
    let layout = Arc::new(build_layout(request, &sources, font_identity, key.clone())?);
    cache.insert(key, Arc::clone(&layout));
    Ok(layout)
}

fn paragraph_key(request: &ParagraphRequest<'_>, font_identity: &str) -> String {
    sha256_prefixed(
        format!(
            "{}|{}|{}|{:08x}|{:08x}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}|{}",
            sha256_prefixed(request.text.as_bytes()),
            font_identity,
            serde_json::to_string(request.spans).unwrap_or_default(),
            request.font_size.to_bits(),
            request.constraint_width.to_bits(),
            request.text_layout.wrap,
            request.text_layout.align,
            request.text_layout.vertical_align,
            request.text_layout.line_height,
            request.text_layout.max_lines,
            request.text_layout.overflow,
            request.text_layout.direction,
            request.font,
            request.text_layout.language.as_deref().unwrap_or(""),
            request.box_height.map(f32::to_bits).unwrap_or_default(),
        )
        .as_bytes(),
    )
}

fn select_bundle<'a>(
    registry: &'a RuntimeFontBundleRegistry,
    font: Option<&AuiFontStyle>,
) -> Result<&'a RuntimeLoadedFontBundle, ParagraphError> {
    font.and_then(|font| font.font_bundle_id.as_deref())
        .and_then(|id| registry.bundles_by_id.get(id))
        .or_else(|| registry.default_bundle())
        .ok_or(ParagraphError::FontBundleMissing)
}

fn sorted_sources<'a>(
    bundle: &'a RuntimeLoadedFontBundle,
    font: Option<&AuiFontStyle>,
) -> Vec<&'a RuntimePackageSourceFontFace> {
    let family = font.and_then(|font| font.font_family_id.as_deref());
    let weight = font.map(|font| font.weight).unwrap_or(400);
    let style = match font.map(|font| font.style).unwrap_or_default() {
        AuiFontStyleKind::Normal => FontBundleStyle::Normal,
        AuiFontStyleKind::Italic => FontBundleStyle::Italic,
        AuiFontStyleKind::Oblique => FontBundleStyle::Oblique,
    };
    let mut sources = bundle.font_face_sources.iter().collect::<Vec<_>>();
    sources.sort_by_key(|source| {
        bundle
            .metadata
            .glyphs
            .iter()
            .filter(|glyph| glyph.font_face_id == source.font_face_id)
            .map(|glyph| {
                (
                    family.is_some_and(|value| value != glyph.font_family_id),
                    glyph.style != style,
                    glyph.weight.abs_diff(weight),
                )
            })
            .min()
            .unwrap_or((true, true, u16::MAX))
    });
    sources
}

fn build_layout(
    request: ParagraphRequest<'_>,
    sources: &[&RuntimePackageSourceFontFace],
    font_identity: String,
    identity: String,
) -> Result<ParagraphLayout, ParagraphError> {
    let font_size = request.font_size.max(1.0);
    let mut clusters = Vec::new();
    let mut mandatory_offsets = Vec::new();
    let mut segment_start = 0;
    for segment in request.text.split('\n') {
        let segment_end = segment_start + segment.len();
        if !segment.is_empty() {
            let mut boundaries = vec![segment_start, segment_end];
            for span in request.spans {
                for boundary in [span.start as usize, span.end as usize] {
                    if boundary > segment_start && boundary < segment_end {
                        boundaries.push(boundary);
                    }
                }
            }
            boundaries.sort_unstable();
            boundaries.dedup();
            for pair in boundaries.windows(2) {
                let (start, end) = (pair[0], pair[1]);
                if start == end {
                    continue;
                }
                let span = request
                    .spans
                    .iter()
                    .find(|span| start >= span.start as usize && start < span.end as usize);
                let style_font = span.and_then(|span| span.font.as_ref()).or(request.font);
                let style_size = span
                    .and_then(|span| span.font_size)
                    .unwrap_or(font_size)
                    .max(1.0);
                let bundle = select_bundle(request.font_bundles, style_font)?;
                let style_sources = sorted_sources(bundle, style_font);
                let runs = shape_stack(&style_sources, &request.text[start..end])
                    .ok_or(ParagraphError::ShapeFailed)?;
                clusters.extend(clusters_from_runs(
                    &runs,
                    start,
                    style_size,
                    &bundle.metadata.font_bundle_id,
                    span.and_then(|span| span.color.clone()),
                    style_font
                        .map(|font| font.raster_policy)
                        .unwrap_or_default(),
                ));
            }
        }
        mandatory_offsets.push(segment_end);
        segment_start = segment_end.saturating_add(1);
    }

    let allowed = linebreaks(request.text)
        .filter_map(|(offset, opportunity)| {
            (opportunity == BreakOpportunity::Allowed).then_some(offset)
        })
        .collect::<HashSet<_>>();
    let mut ranges = Vec::new();
    let mut cluster_start = 0;
    let mut source_start = 0;
    for mandatory in mandatory_offsets {
        let cluster_end = clusters.partition_point(|cluster| cluster.end <= mandatory);
        append_segment_lines(
            &clusters,
            cluster_start,
            cluster_end,
            source_start,
            mandatory,
            request.constraint_width,
            request.text_layout.wrap,
            &allowed,
            &mut ranges,
        );
        cluster_start = cluster_end;
        source_start = mandatory.saturating_add(1);
    }
    if ranges.is_empty() {
        ranges.push((0, 0, 0, 0));
    }

    let max_lines = request
        .text_layout
        .max_lines
        .map(|value| value as usize)
        .unwrap_or(MAX_PARAGRAPH_LINES)
        .min(MAX_PARAGRAPH_LINES);
    let truncated = ranges.len() > max_lines;
    ranges.truncate(max_lines);

    let natural = line_metrics(&clusters, font_size);
    let line_height = match request.text_layout.line_height {
        LineHeight::Multiple { value } => natural.2 * value,
        LineHeight::Fixed { value } => value,
    }
    .max(0.0);
    let intrinsic_height = line_height * ranges.len() as f32;
    let box_height = request.box_height.unwrap_or(intrinsic_height);
    let vertical_offset = match request.text_layout.vertical_align {
        VerticalAlign::Start => 0.0,
        VerticalAlign::Center => (box_height - intrinsic_height).max(0.0) * 0.5,
        VerticalAlign::End => (box_height - intrinsic_height).max(0.0),
    };
    let bidi = BidiInfo::new(
        request.text,
        match request.text_layout.direction {
            TextDirection::Auto => None,
            TextDirection::Ltr => Some(Level::ltr()),
            TextDirection::Rtl => Some(Level::rtl()),
        },
    );
    let mut glyphs = Vec::new();
    let mut lines = Vec::new();
    let mut max_width = 0.0_f32;
    for (line_index, &(start, end, source_start, source_end)) in ranges.iter().enumerate() {
        let width = clusters[start..end]
            .iter()
            .map(|cluster| cluster.advance)
            .sum::<f32>();
        let justify_gap = if request.text_layout.align == TextAlign::Justify
            && line_index + 1 < ranges.len()
            && request.constraint_width.is_finite()
            && width < request.constraint_width
        {
            let whitespace_count = clusters[start..end]
                .iter()
                .filter(|cluster| {
                    request
                        .text
                        .get(cluster.start..cluster.end)
                        .is_some_and(|text| text.chars().all(char::is_whitespace))
                })
                .count();
            if whitespace_count > 0 {
                (request.constraint_width - width) / whitespace_count as f32
            } else {
                0.0
            }
        } else {
            0.0
        };
        let display_width = if justify_gap > 0.0 {
            request.constraint_width
        } else {
            width
        };
        max_width = max_width.max(display_width);
        let paragraph = paragraph_for_line(&bidi, source_start, source_end);
        let base_rtl = paragraph.is_some_and(|paragraph| paragraph.level.is_rtl());
        let x_offset = match request.text_layout.align {
            TextAlign::Start if base_rtl => (request.constraint_width - width).max(0.0),
            TextAlign::End if base_rtl => 0.0,
            TextAlign::Start | TextAlign::Justify => 0.0,
            TextAlign::Center => (request.constraint_width - width).max(0.0) * 0.5,
            TextAlign::End => (request.constraint_width - width).max(0.0),
        };
        let baseline = vertical_offset
            + line_index as f32 * line_height
            + natural.0
            + (line_height - natural.2).max(0.0) * 0.5;
        let glyph_start = glyphs.len();
        let mut cursor = x_offset;
        let (visual_indices, visual_run_order) = visual_cluster_indices(
            &bidi,
            paragraph,
            source_start,
            source_end,
            &clusters,
            start,
            end,
        );
        for cluster_index in visual_indices {
            let cluster = &clusters[cluster_index];
            for glyph in &cluster.glyphs {
                glyphs.push(ParagraphGlyph {
                    font_bundle_id: glyph.font_bundle_id.clone(),
                    face_id: glyph.face_id.clone(),
                    glyph_id: glyph.glyph_id,
                    cluster_start: glyph.cluster_start as u32,
                    cluster_end: glyph.cluster_end as u32,
                    x: cursor + glyph.offset_x,
                    y: baseline - glyph.offset_y,
                    advance: glyph.advance,
                    has_outline: glyph.has_outline,
                    ink_bounds: glyph.ink_bounds,
                    fallback_used: glyph.fallback_used,
                    font_size: glyph.font_size,
                    color: glyph.color.clone(),
                    raster_policy: glyph.raster_policy,
                });
                cursor += glyph.advance;
            }
            if justify_gap > 0.0
                && request
                    .text
                    .get(cluster.start..cluster.end)
                    .is_some_and(|text| text.chars().all(char::is_whitespace))
            {
                cursor += justify_gap;
            }
        }
        lines.push(ParagraphLine {
            source_start: source_start as u32,
            source_end: source_end as u32,
            glyph_start,
            glyph_end: glyphs.len(),
            visual_run_order,
            baseline,
            width: display_width,
            height: line_height,
        });
    }
    if truncated && request.text_layout.overflow == TextOverflow::Ellipsis {
        let ellipsis_bundle_id = select_bundle(request.font_bundles, request.font)?
            .metadata
            .font_bundle_id
            .clone();
        append_ellipsis(
            &mut glyphs,
            &mut lines,
            sources,
            font_size,
            request.constraint_width,
            &ellipsis_bundle_id,
        )?;
    }
    Ok(ParagraphLayout {
        identity,
        text_digest: sha256_prefixed(request.text.as_bytes()),
        font_identity,
        constraint_width: request.constraint_width,
        width: if request.constraint_width.is_finite() {
            max_width.min(request.constraint_width.max(0.0))
        } else {
            max_width
        },
        height: intrinsic_height,
        first_baseline: lines.first().map(|line| line.baseline),
        lines,
        glyphs,
        truncated,
        overflow: request.text_layout.overflow,
    })
}

fn clusters_from_runs(
    runs: &[GlyphRun],
    source_offset: usize,
    font_size: f32,
    font_bundle_id: &str,
    color: Option<String>,
    raster_policy: AuiFontRasterPolicy,
) -> Vec<Cluster> {
    let mut clusters = Vec::<Cluster>::new();
    for run in runs {
        let scale = font_size / f32::from(run.units_per_em.max(1));
        for item in &run.items {
            let start = source_offset + item.cluster_start as usize;
            let end = source_offset + item.cluster_end as usize;
            let glyph = TempGlyph {
                font_bundle_id: font_bundle_id.to_owned(),
                face_id: run.face_id.clone(),
                glyph_id: item.glyph_id,
                cluster_start: start,
                cluster_end: end,
                offset_x: item.offset_x as f32 * scale,
                offset_y: item.offset_y as f32 * scale,
                advance: item.advance_x as f32 * scale,
                has_outline: item.has_outline,
                ink_bounds: item
                    .ink_bounds
                    .map(|bounds| bounds.map(|value| value as f32 * scale)),
                fallback_used: run.fallback_used,
                ascender: f32::from(run.ascender) * scale,
                descender: f32::from(run.descender) * scale,
                line_gap: f32::from(run.line_gap) * scale,
                font_size,
                color: color.clone(),
                raster_policy,
            };
            if let Some(cluster) = clusters
                .iter_mut()
                .find(|cluster| cluster.start == start && cluster.end == end)
            {
                cluster.advance += glyph.advance;
                cluster.glyphs.push(glyph);
            } else {
                clusters.push(Cluster {
                    start,
                    end,
                    advance: glyph.advance,
                    glyphs: vec![glyph],
                });
            }
        }
    }
    clusters.sort_by_key(|cluster| cluster.start);
    clusters
}

fn paragraph_for_line<'a>(
    bidi: &'a BidiInfo<'_>,
    source_start: usize,
    source_end: usize,
) -> Option<&'a ParagraphInfo> {
    bidi.paragraphs.iter().find(|paragraph| {
        source_start >= paragraph.range.start && source_end <= paragraph.range.end
    })
}

#[allow(clippy::too_many_arguments)]
fn visual_cluster_indices(
    bidi: &BidiInfo<'_>,
    paragraph: Option<&ParagraphInfo>,
    source_start: usize,
    source_end: usize,
    clusters: &[Cluster],
    cluster_start: usize,
    cluster_end: usize,
) -> (Vec<usize>, Vec<ParagraphVisualRun>) {
    let logical = (cluster_start..cluster_end).collect::<Vec<_>>();
    let Some(paragraph) = paragraph else {
        return (logical, Vec::new());
    };
    if source_start >= source_end {
        return (logical, Vec::new());
    }
    let (levels, runs) = bidi.visual_runs(paragraph, source_start..source_end);
    let mut visual = Vec::new();
    let mut descriptions = Vec::new();
    let mut included = vec![false; clusters.len()];
    for run in runs {
        let level = levels.get(run.start).copied().unwrap_or(paragraph.level);
        let mut indices = (cluster_start..cluster_end)
            .filter(|index| {
                let cluster = &clusters[*index];
                cluster.start < run.end && cluster.end > run.start
            })
            .collect::<Vec<_>>();
        if level.is_rtl() {
            indices.reverse();
        }
        for index in indices {
            if !included[index] {
                included[index] = true;
                visual.push(index);
            }
        }
        descriptions.push(ParagraphVisualRun {
            source_start: run.start as u32,
            source_end: run.end as u32,
            embedding_level: level.number(),
            rtl: level.is_rtl(),
        });
    }
    for (index, is_included) in included
        .iter()
        .enumerate()
        .take(cluster_end)
        .skip(cluster_start)
    {
        if !is_included {
            visual.push(index);
        }
    }
    (visual, descriptions)
}

#[allow(clippy::too_many_arguments)]
fn append_segment_lines(
    clusters: &[Cluster],
    start: usize,
    end: usize,
    source_start: usize,
    source_end: usize,
    limit: f32,
    wrap: TextWrap,
    allowed: &HashSet<usize>,
    output: &mut Vec<(usize, usize, usize, usize)>,
) {
    if start == end {
        output.push((start, end, source_start, source_end));
        return;
    }
    if wrap == TextWrap::None || !limit.is_finite() {
        output.push((start, end, source_start, source_end));
        return;
    }
    let mut line_start = start;
    while line_start < end && output.len() < MAX_PARAGRAPH_LINES {
        let mut cursor = 0.0;
        let mut fit_end = line_start;
        let mut allowed_end = None;
        while fit_end < end {
            let next = cursor + clusters[fit_end].advance;
            if fit_end > line_start && next > limit.max(0.0) {
                break;
            }
            cursor = next;
            fit_end += 1;
            if allowed.contains(&clusters[fit_end - 1].end) {
                allowed_end = Some(fit_end);
            }
            if next > limit.max(0.0) {
                break;
            }
        }
        if fit_end == end {
            output.push((line_start, end, clusters[line_start].start, source_end));
            break;
        }
        let break_end = match wrap {
            TextWrap::Word => allowed_end.unwrap_or_else(|| {
                let mut word_end = fit_end.max(line_start + 1);
                while word_end < end && !allowed.contains(&clusters[word_end - 1].end) {
                    word_end += 1;
                }
                word_end
            }),
            TextWrap::GraphemeFallback => allowed_end.unwrap_or(fit_end.max(line_start + 1)),
            TextWrap::None => end,
        };
        output.push((
            line_start,
            break_end,
            clusters[line_start].start,
            clusters[break_end - 1].end,
        ));
        line_start = break_end;
    }
}

fn line_metrics(clusters: &[Cluster], font_size: f32) -> (f32, f32, f32) {
    let ascender = clusters
        .iter()
        .flat_map(|cluster| &cluster.glyphs)
        .map(|glyph| glyph.ascender)
        .fold(0.0, f32::max);
    let descender = clusters
        .iter()
        .flat_map(|cluster| &cluster.glyphs)
        .map(|glyph| glyph.descender)
        .fold(0.0, f32::min);
    let gap = clusters
        .iter()
        .flat_map(|cluster| &cluster.glyphs)
        .map(|glyph| glyph.line_gap)
        .fold(0.0, f32::max);
    let height = (ascender - descender + gap).max(font_size);
    (ascender.max(font_size * 0.8), descender, height)
}

fn append_ellipsis(
    glyphs: &mut Vec<ParagraphGlyph>,
    lines: &mut [ParagraphLine],
    sources: &[&RuntimePackageSourceFontFace],
    font_size: f32,
    limit: f32,
    font_bundle_id: &str,
) -> Result<(), ParagraphError> {
    let Some(line) = lines.last_mut() else {
        return Ok(());
    };
    let runs = shape_stack(sources, "…").ok_or(ParagraphError::ShapeFailed)?;
    let ellipsis = clusters_from_runs(
        &runs,
        line.source_end as usize,
        font_size,
        font_bundle_id,
        None,
        AuiFontRasterPolicy::default(),
    );
    let ellipsis_width = ellipsis.iter().map(|cluster| cluster.advance).sum::<f32>();
    while line.glyph_end > line.glyph_start
        && limit.is_finite()
        && line.width + ellipsis_width > limit
    {
        let cluster_start = glyphs[line.glyph_end - 1].cluster_start;
        while line.glyph_end > line.glyph_start
            && glyphs[line.glyph_end - 1].cluster_start == cluster_start
        {
            line.glyph_end -= 1;
            let removed = glyphs.pop().expect("line glyph exists");
            line.width -= removed.advance;
        }
    }
    let mut cursor = line.width;
    for cluster in ellipsis {
        for glyph in cluster.glyphs {
            glyphs.push(ParagraphGlyph {
                font_bundle_id: glyph.font_bundle_id,
                face_id: glyph.face_id,
                glyph_id: glyph.glyph_id,
                cluster_start: line.source_end,
                cluster_end: line.source_end,
                x: cursor + glyph.offset_x,
                y: line.baseline - glyph.offset_y,
                advance: glyph.advance,
                has_outline: glyph.has_outline,
                ink_bounds: glyph.ink_bounds,
                fallback_used: glyph.fallback_used,
                font_size: glyph.font_size,
                color: glyph.color,
                raster_policy: glyph.raster_policy,
            });
            cursor += glyph.advance;
        }
    }
    line.width = cursor;
    line.glyph_end = glyphs.len();
    Ok(())
}

enum ParagraphNodeSlots<'a> {
    Owned(HashMap<&'a str, usize>),
    Shared(&'a HashMap<String, usize>),
}

impl ParagraphNodeSlots<'_> {
    fn get(&self, id: &str) -> Option<&usize> {
        match self {
            Self::Owned(slots) => slots.get(id),
            Self::Shared(slots) => slots.get(id),
        }
    }
}

pub struct AuiParagraphContext<'a> {
    document: &'a AuiDocument,
    nodes_by_id: ParagraphNodeSlots<'a>,
    font_bundles: &'a RuntimeFontBundleRegistry,
    cache: &'a mut AuiParagraphCache,
    layouts: HashMap<String, Arc<ParagraphLayout>>,
    errors: HashMap<String, ParagraphError>,
    custom_measure_inputs:
        HashMap<String, project_runtime_sdk::aui_extension::UiExtensionConstraints>,
}

impl<'a> AuiParagraphContext<'a> {
    pub fn new(
        document: &'a AuiDocument,
        font_bundles: &'a RuntimeFontBundleRegistry,
        cache: &'a mut AuiParagraphCache,
    ) -> Self {
        Self {
            document,
            nodes_by_id: ParagraphNodeSlots::Owned(
                document
                    .nodes
                    .iter()
                    .enumerate()
                    .map(|(slot, node)| (node.node_id.as_str(), slot))
                    .collect(),
            ),
            font_bundles,
            cache,
            layouts: HashMap::new(),
            errors: HashMap::new(),
            custom_measure_inputs: HashMap::new(),
        }
    }

    pub(crate) fn with_node_slots(
        document: &'a AuiDocument,
        slots: &'a HashMap<String, usize>,
        font_bundles: &'a RuntimeFontBundleRegistry,
        cache: &'a mut AuiParagraphCache,
    ) -> Self {
        Self {
            document,
            nodes_by_id: ParagraphNodeSlots::Shared(slots),
            font_bundles,
            cache,
            layouts: HashMap::new(),
            errors: HashMap::new(),
            custom_measure_inputs: HashMap::new(),
        }
    }

    pub fn layout_node(
        &mut self,
        node_id: &str,
        width: f32,
        height: Option<f32>,
    ) -> Option<Arc<ParagraphLayout>> {
        self.layout_node_with_text(node_id, None, width, height)
    }

    fn layout_node_with_text(
        &mut self,
        node_id: &str,
        text: Option<&str>,
        width: f32,
        height: Option<f32>,
    ) -> Option<Arc<ParagraphLayout>> {
        let node = &self.document.nodes[*self.nodes_by_id.get(node_id)?];
        let result = self.layout_for_node(node, text, width, height);
        match result {
            Ok(layout) => {
                self.layouts.insert(node_id.to_owned(), Arc::clone(&layout));
                Some(layout)
            }
            Err(error) => {
                self.errors.insert(node_id.to_owned(), error);
                None
            }
        }
    }

    pub fn frame_layouts(&self) -> &HashMap<String, Arc<ParagraphLayout>> {
        &self.layouts
    }

    pub fn has_text(&self, node_id: &str) -> bool {
        self.nodes_by_id.get(node_id).is_some_and(|&slot| {
            let node = &self.document.nodes[slot];
            node.text.is_some() || node.placeholder.is_some()
        })
    }

    pub fn errors(&self) -> &HashMap<String, ParagraphError> {
        &self.errors
    }

    pub fn custom_measure_inputs(
        &self,
    ) -> &HashMap<String, project_runtime_sdk::aui_extension::UiExtensionConstraints> {
        &self.custom_measure_inputs
    }

    fn layout_for_node(
        &mut self,
        node: &AuiNode,
        text: Option<&str>,
        width: f32,
        height: Option<f32>,
    ) -> Result<Arc<ParagraphLayout>, ParagraphError> {
        layout_paragraph(
            self.cache,
            ParagraphRequest {
                text: text.unwrap_or_else(|| {
                    node.text
                        .as_deref()
                        .or(node.placeholder.as_deref())
                        .unwrap_or("")
                }),
                font_size: node
                    .style
                    .as_ref()
                    .and_then(|style| style.font_size)
                    .unwrap_or(12.0),
                font: node.style.as_ref().and_then(|style| style.font.as_ref()),
                spans: &node.text_spans,
                text_layout: node
                    .text_layout
                    .as_ref()
                    .unwrap_or(&TextLayoutSpec::default()),
                constraint_width: width,
                box_height: height,
                font_bundles: self.font_bundles,
            },
        )
    }
}

impl AuiIntrinsicMeasure for AuiParagraphContext<'_> {
    fn measure(&mut self, input: AuiMeasureInput<'_>) -> AuiMeasureOutput {
        if self
            .nodes_by_id
            .get(input.node_id)
            .is_some_and(|&slot| self.document.nodes[slot].custom.is_some())
        {
            self.custom_measure_inputs.insert(
                input.node_id.to_string(),
                project_runtime_sdk::aui_extension::UiExtensionConstraints {
                    known_width: input.known_width,
                    known_height: input.known_height,
                    available_width: definite_space(input.available_width),
                    available_height: definite_space(input.available_height),
                },
            );
        }
        if let Some(output) = self
            .nodes_by_id
            .get(input.node_id)
            .and_then(|&slot| self.document.nodes[slot].custom_runtime.as_ref())
            .and_then(|runtime| runtime.measure)
        {
            return AuiMeasureOutput {
                width: input.known_width.unwrap_or(output.size[0]),
                height: input.known_height.unwrap_or(output.size[1]),
                first_baseline: output.first_baseline,
            };
        }
        if input.text.is_none() {
            return AuiMeasureOutput {
                width: input.known_width.unwrap_or(0.0),
                height: input.known_height.unwrap_or(0.0),
                first_baseline: None,
            };
        }
        let width = input.known_width.unwrap_or(match input.available_width {
            AuiAvailableSpace::Definite(width) => width,
            AuiAvailableSpace::MinContent => 0.0,
            AuiAvailableSpace::MaxContent => f32::INFINITY,
        });
        let Some(layout) =
            self.layout_node_with_text(input.node_id, input.text, width, input.known_height)
        else {
            // Geometry-only legacy callers may lack fonts. The context records the error;
            // the runtime must discard this canvas's measurement cache before the next call.
            return AuiMeasureOutput::default();
        };
        AuiMeasureOutput {
            width: input.known_width.unwrap_or(layout.width),
            height: input.known_height.unwrap_or(layout.height),
            first_baseline: layout.first_baseline,
        }
    }

    fn layout_children(
        &mut self,
        node_id: &str,
        _size: [f32; 2],
        child_ids: &[String],
    ) -> Option<Vec<crate::aui_layout_runtime::AuiCustomChildLayout>> {
        let runtime = self
            .nodes_by_id
            .get(node_id)
            .and_then(|&slot| self.document.nodes[slot].custom_runtime.as_ref())?;
        if runtime.child_rects.is_empty() {
            return None;
        }
        let mut output = Vec::with_capacity(runtime.child_rects.len());
        for child in &runtime.child_rects {
            let source = self.document.nodes.get(child.handle.slot as usize)?;
            if !child_ids.iter().any(|id| id == &source.node_id) {
                return None;
            }
            output.push(crate::aui_layout_runtime::AuiCustomChildLayout {
                node_id: source.node_id.clone(),
                rect: crate::aui::AuiComputedRect {
                    x: child.rect[0],
                    y: child.rect[1],
                    width: child.rect[2],
                    height: child.rect[3],
                },
            });
        }
        Some(output)
    }
}

fn definite_space(value: AuiAvailableSpace) -> Option<f32> {
    match value {
        AuiAvailableSpace::Definite(value) if value.is_finite() => Some(value),
        AuiAvailableSpace::Definite(_)
        | AuiAvailableSpace::MinContent
        | AuiAvailableSpace::MaxContent => None,
    }
}
