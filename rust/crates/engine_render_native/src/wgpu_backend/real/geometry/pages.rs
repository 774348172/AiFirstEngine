//! Physical mesh pages inside GeometryGpuState. Range guards are retired by the
//! existing GPU completion owner; their Drop, never a frame number, enables reuse.
use super::*;
use std::sync::Mutex;

const TARGET_PAGE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PAGES: usize = 4096;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Vertex,
    Index,
}

#[derive(Clone)]
struct FreeRanges {
    free: BTreeMap<u64, u64>,
    live: usize,
}
impl FreeRanges {
    fn new(bytes: u64) -> Self {
        Self {
            free: BTreeMap::from([(0, bytes)]),
            live: 0,
        }
    }
    fn allocate(&mut self, bytes: u64, alignment: u64) -> Option<u64> {
        if bytes == 0 || alignment == 0 {
            return None;
        }
        let (offset, length, start) = self.free.iter().find_map(|(&offset, &length)| {
            let start = offset.checked_add(alignment - 1)? / alignment * alignment;
            let end = start.checked_add(bytes)?;
            (end <= offset.checked_add(length)?).then_some((offset, length, start))
        })?;
        self.free.remove(&offset);
        if start > offset {
            self.free.insert(offset, start - offset);
        }
        let end = start + bytes;
        if end < offset + length {
            self.free.insert(end, offset + length - end);
        }
        self.live += 1;
        Some(start)
    }
    fn release(&mut self, mut start: u64, mut bytes: u64) {
        if let Some((&before, &length)) = self.free.range(..start).next_back() {
            if before + length == start {
                self.free.remove(&before);
                start = before;
                bytes += length;
            }
        }
        if let Some(&length) = self.free.get(&(start + bytes)) {
            self.free.remove(&(start + bytes));
            bytes += length;
        }
        assert!(
            self.free.insert(start, bytes).is_none(),
            "range guard released twice"
        );
        self.live -= 1;
    }
}

struct Page {
    buffer: wgpu::Buffer,
    kind: Kind,
    free: Mutex<FreeRanges>,
}
pub(super) struct PageRange {
    page: Arc<Page>,
    pub(super) start: u64,
    bytes: u64,
}
impl PageRange {
    pub(super) fn buffer(&self) -> &wgpu::Buffer {
        &self.page.buffer
    }
}
impl Drop for PageRange {
    fn drop(&mut self) {
        self.page
            .free
            .lock()
            .expect("geometry page poisoned")
            .release(self.start, self.bytes);
    }
}

#[derive(Default)]
pub(super) struct GeometryPages {
    pages: Vec<Arc<Page>>,
}
struct PlanPage {
    original: Option<Arc<Page>>,
    kind: Kind,
    bytes: u64,
    free: FreeRanges,
}
impl GeometryPages {
    pub(in crate::wgpu_backend::real) fn observe_memory<'a>(
        &'a self,
        gpu: &mut crate::wgpu_backend::real::memory::GpuMemoryCensus<'a>,
    ) {
        for page in &self.pages {
            gpu.buffer(&page.buffer, "renderer", &["resident"]);
        }
    }
    pub(super) fn physical_bytes(&self) -> u64 {
        self.pages.iter().map(|p| p.buffer.size()).sum()
    }
    pub(super) fn collect_empty(&mut self) {
        self.pages
            .retain(|p| p.free.lock().expect("geometry page poisoned").live != 0);
    }
    /// Plans all ranges and additional physical pages before touching live state.
    /// Existing pages, including ranges awaiting GPU completion, count once.
    pub(super) fn reserve(
        &mut self,
        device: &wgpu::Device,
        sources: &[(RenderResourceHandle, Arc<GeometryBuffers>)],
        maximum_physical_bytes: u64,
    ) -> Result<Vec<(RenderResourceHandle, ResidentGeometry)>, String> {
        let mut needs = Vec::with_capacity(sources.len() * 2);
        for (_, source) in sources {
            for (kind, length, align) in [
                (
                    Kind::Vertex,
                    source.vertex_byte_len(),
                    u64::from(source.vertex_stride()),
                ),
                (Kind::Index, source.index_byte_len(), 4),
            ] {
                if align == 0 || align % 4 != 0 {
                    return Err("geometry.page_alignment".into());
                }
                let bytes = u64::try_from(length)
                    .ok()
                    .and_then(|n| n.checked_add(3))
                    .map(|n| (n & !3).max(4))
                    .ok_or("geometry.page_size_overflow")?;
                if bytes > device.limits().max_buffer_size {
                    return Err("geometry.device_limit".into());
                }
                needs.push((kind, bytes, align));
            }
        }
        let mut pages: Vec<_> = self
            .pages
            .iter()
            .map(|p| PlanPage {
                original: Some(p.clone()),
                kind: p.kind,
                bytes: p.buffer.size(),
                free: p.free.lock().expect("geometry page poisoned").clone(),
            })
            .collect();
        let mut physical = self.physical_bytes();
        if physical > maximum_physical_bytes {
            return Err("geometry.upload_resident_budget".into());
        }
        let mut ranges = Vec::with_capacity(needs.len());
        for (need_index, (kind, bytes, align)) in needs.iter().copied().enumerate() {
            let found = pages.iter_mut().enumerate().find_map(|(i, p)| {
                (p.kind == kind)
                    .then(|| p.free.allocate(bytes, align))
                    .flatten()
                    .map(|start| (i, start))
            });
            let (page, start) = match found {
                Some(pair) => pair,
                None => {
                    if pages.len() >= MAX_PAGES {
                        return Err("geometry.page_capacity".into());
                    }
                    // Exact small-request pages preserve small resource budgets.
                    // Large batches use bounded pages rather than one buffer per mesh.
                    let capacity = TARGET_PAGE_BYTES
                        .max(bytes)
                        .min(device.limits().max_buffer_size);
                    let mut size = 0u64;
                    for &(next_kind, next_bytes, next_align) in &needs[need_index..] {
                        if next_kind != kind {
                            continue;
                        }
                        let end = size
                            .checked_add(next_align - 1)
                            .map(|n| n / next_align * next_align)
                            .and_then(|n| n.checked_add(next_bytes))
                            .ok_or("geometry.page_size_overflow")?;
                        if end > capacity {
                            break;
                        }
                        size = end;
                    }
                    let size = size
                        .checked_add(3)
                        .map(|n| n & !3)
                        .ok_or("geometry.page_size_overflow")?;
                    if size > device.limits().max_buffer_size {
                        return Err("geometry.device_limit".into());
                    }
                    physical = physical
                        .checked_add(size)
                        .filter(|n| *n <= maximum_physical_bytes)
                        .ok_or("geometry.upload_resident_budget")?;
                    let mut free = FreeRanges::new(size);
                    let start = free
                        .allocate(bytes, align)
                        .expect("new page starts aligned");
                    pages.push(PlanPage {
                        original: None,
                        kind,
                        bytes: size,
                        free,
                    });
                    (pages.len() - 1, start)
                }
            };
            let offset_fits = match kind {
                Kind::Vertex => i32::try_from(start / align).is_ok(),
                Kind::Index => u32::try_from(start / 2 + bytes / 2).is_ok(),
            };
            if !offset_fits {
                return Err("geometry.page_draw_offset_overflow".into());
            }
            ranges.push((page, start, bytes));
        }
        let mut committed = Vec::with_capacity(pages.len());
        for p in pages {
            let page = if let Some(original) = p.original {
                *original.free.lock().expect("geometry page poisoned") = p.free;
                original
            } else {
                Arc::new(Page {
                    buffer: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("resident-geometry-page"),
                        size: p.bytes,
                        usage: wgpu::BufferUsages::COPY_DST
                            | if p.kind == Kind::Vertex {
                                wgpu::BufferUsages::VERTEX
                            } else {
                                wgpu::BufferUsages::INDEX
                            },
                        mapped_at_creation: false,
                    }),
                    kind: p.kind,
                    free: Mutex::new(p.free),
                })
            };
            committed.push(page);
        }
        self.pages = committed;
        let mut ranges = ranges.into_iter().map(|(page, start, bytes)| PageRange {
            page: self.pages[page].clone(),
            start,
            bytes,
        });
        Ok(sources
            .iter()
            .map(|(handle, source)| {
                (
                    *handle,
                    ResidentGeometry {
                        source: source.clone(),
                        vertices: ranges.next().unwrap(),
                        indices: ranges.next().unwrap(),
                    },
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_align_split_and_coalesce_without_overlapping_neighbors() {
        let mut free = FreeRanges::new(128);
        let a = free.allocate(20, 20).unwrap();
        let b = free.allocate(24, 24).unwrap();
        let c = free.allocate(32, 4).unwrap();
        assert_eq!((a, b, c), (0, 24, 48));
        free.release(b, 24);
        assert_eq!(free.allocate(24, 24), Some(b));
        free.release(a, 20);
        free.release(c, 32);
        free.release(b, 24);
        assert_eq!(free.live, 0);
        assert_eq!(free.free, BTreeMap::from([(0, 128)]));
    }
    #[test]
    fn rejected_range_request_preserves_allocator_and_checks_overflow() {
        let mut free = FreeRanges::new(64);
        let old = free.free.clone();
        assert_eq!(free.allocate(65, 4), None);
        assert_eq!(free.free, old);
        assert_eq!(free.live, 0);
        assert_eq!(free.allocate(0, 4), None);
        assert_eq!(free.allocate(4, 0), None);
        let mut edge = FreeRanges {
            free: BTreeMap::from([(u64::MAX - 1, 1)]),
            live: 0,
        };
        assert_eq!(edge.allocate(4, 4), None);
        assert_eq!(edge.live, 0);
    }

    fn source() -> (RenderResourceHandle, Arc<GeometryBuffers>) {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let draw = graph
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .find_map(|c| match c {
                crate::render_graph::RenderPassCommand::DrawGeometry { draw } => Some(draw),
                _ => None,
            })
            .unwrap();
        (draw.geometry_handle().raw(), draw.geometry.clone())
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn gpu_page_reservation_is_atomic_and_retained_range_blocks_reuse() {
        let backend = RealWgpuBackend::new_offscreen(16, 16).unwrap();
        let mut pages = GeometryPages::default();
        let (handle, source) = source();
        let bytes = [source.vertex_byte_len(), source.index_byte_len()]
            .into_iter()
            .map(|n| ((n as u64 + 3) & !3).max(4))
            .sum::<u64>();
        assert!(pages
            .reserve(&backend.device, &[(handle, source.clone())], bytes - 1)
            .is_err());
        assert_eq!(pages.physical_bytes(), 0);
        let pair = [
            (handle, source.clone()),
            (
                RenderResourceHandle {
                    index: handle.index + 1,
                    ..handle
                },
                source.clone(),
            ),
        ];
        let mut resident = pages.reserve(&backend.device, &pair, bytes * 2).unwrap();
        let neighbor = resident.pop().unwrap().1;
        let retired = resident.pop().unwrap().1;
        let old_start = retired.vertices.start;
        let physical = pages.physical_bytes();
        // A completion owner retaining this guard makes the range unavailable;
        // a failed allocation must leave its neighbor and physical pool intact.
        assert!(pages
            .reserve(&backend.device, &[(handle, source.clone())], physical)
            .is_err());
        assert_eq!(pages.physical_bytes(), physical);
        drop(retired);
        let reused = pages
            .reserve(&backend.device, &[(handle, source)], physical)
            .unwrap();
        assert_eq!(reused[0].1.vertices.start, old_start);
        assert_eq!(pages.physical_bytes(), physical);
        drop(reused);
        pages.collect_empty();
        assert_eq!(pages.physical_bytes(), physical);
        drop(neighbor);
        pages.collect_empty();
        assert_eq!(pages.physical_bytes(), 0);
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn gpu_mixed_stride_pages_charge_alignment_and_reject_underbudget_atomically() {
        let backend = RealWgpuBackend::new_offscreen(16, 16).unwrap();
        let (handle, source) = source();
        let mut wire = serde_json::to_value(source.as_ref()).unwrap();
        // Preserve POSITION, add unused per-vertex padding to form a distinct stride.
        let stride = source.vertex_stride() as usize;
        let next = stride + 4;
        let mut vertices = Vec::new();
        for chunk in source.vertex_bytes().chunks_exact(stride) {
            vertices.extend_from_slice(chunk);
            vertices.extend_from_slice(&[0; 4]);
        }
        wire["vertexStride"] = serde_json::json!(next);
        wire["vertexBytes"] = serde_json::json!(vertices);
        let wide = Arc::new(serde_json::from_value::<GeometryBuffers>(wire).unwrap());
        let sources = [
            (handle, source),
            (
                RenderResourceHandle {
                    index: handle.index + 1,
                    ..handle
                },
                wide,
            ),
        ];
        let mut pages = GeometryPages::default();
        let allocations = pages.reserve(&backend.device, &sources, 1024).unwrap();
        for (_, m) in &allocations {
            assert_eq!(m.vertices.start % u64::from(m.source.vertex_stride()), 0);
            assert_eq!(m.indices.start % 4, 0);
        }
        let physical = pages.physical_bytes();
        let logical = sources
            .iter()
            .map(|(_, s)| (s.vertex_byte_len() + s.index_byte_len()) as u64)
            .sum::<u64>();
        assert!(physical >= logical);
        let mut rejected = GeometryPages::default();
        assert!(rejected
            .reserve(&backend.device, &sources, physical - 1)
            .is_err());
        assert_eq!(rejected.physical_bytes(), 0);
        drop(allocations);
        pages.collect_empty();
        assert_eq!(pages.physical_bytes(), 0);
    }
}
