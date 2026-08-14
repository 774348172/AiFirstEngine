use editor_ui_model::{
    AssetBrowserDiagnostic, AssetBrowserEntry, AssetBrowserModel, AssetEntryRole, AssetPreviewKind,
    AssetPreviewStatus, AssetThumbnailAspectRatio,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use crate::asset_browser::stable_content_hash;

pub const ASSET_THUMBNAIL_MAX_ITEMS: usize = 128;
pub const ASSET_THUMBNAIL_MAX_CPU_BYTES: usize = 64 * 1024 * 1024;
pub const ASSET_THUMBNAIL_MAX_PENDING: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetThumbnailRequest {
    pub thumbnail_id: String,
    pub source_key: String,
    pub content_hash: String,
    pub project_root: PathBuf,
    pub source_path: PathBuf,
    pub source_project_relative_path: String,
    pub requested_size: u32,
}

impl AssetThumbnailRequest {
    pub fn for_entry(
        project_root: &Path,
        catalog_entries: &[AssetBrowserEntry],
        entry: &AssetBrowserEntry,
        requested_size: u32,
    ) -> Option<Self> {
        if entry.preview.preview_kind != AssetPreviewKind::Thumbnail || !entry.exists {
            return None;
        }
        let source = if entry.role == AssetEntryRole::SourceFile {
            entry
        } else {
            let source_path = entry
                .source_path
                .as_deref()
                .or(entry.preview.thumbnail_source_path.as_deref())?;
            catalog_entries.iter().find(|candidate| {
                candidate.role == AssetEntryRole::SourceFile
                    && candidate.canonical_path == source_path
            })?
        };
        let content_hash = source.entry_key.content_hash()?.to_string();
        let requested_size = requested_size.clamp(16, 512);
        let source_key = format!(
            "{}::{}",
            project_root.to_string_lossy().replace('\\', "/"),
            source.entry_key.stable_token()
        );
        let thumbnail_id = thumbnail_id(&source_key, &content_hash, requested_size);
        Some(Self {
            thumbnail_id,
            source_key,
            content_hash,
            project_root: project_root.to_path_buf(),
            source_path: project_root.join(&source.canonical_path),
            source_project_relative_path: source.canonical_path.clone(),
            requested_size,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetThumbnailCpuPayload {
    pub thumbnail_id: String,
    pub source_key: String,
    pub content_hash: String,
    pub source_project_relative_path: String,
    pub requested_size: u32,
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}

impl AssetThumbnailCpuPayload {
    pub fn byte_len(&self) -> usize {
        self.rgba8.len()
    }

    pub fn aspect_ratio(&self) -> Option<AssetThumbnailAspectRatio> {
        AssetThumbnailAspectRatio::new(self.width, self.height)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetThumbnailServiceSummary {
    pub record_count: usize,
    pub pending_count: usize,
    pub ready_count: usize,
    pub failed_count: usize,
    pub cpu_bytes: usize,
    pub decode_count: u64,
    pub cache_hit_count: u64,
    pub eviction_count: u64,
    pub diagnostics: Vec<AssetBrowserDiagnostic>,
}

struct AssetThumbnailRecord {
    request: AssetThumbnailRequest,
    status: AssetPreviewStatus,
    aspect_ratio: Option<AssetThumbnailAspectRatio>,
    payload: Option<AssetThumbnailCpuPayload>,
    last_used_tick: u64,
}

struct AssetThumbnailWorkerResult {
    request: AssetThumbnailRequest,
    result: Result<AssetThumbnailCpuPayload, AssetBrowserDiagnostic>,
}

pub struct AssetThumbnailService {
    records: HashMap<String, AssetThumbnailRecord>,
    sender: Sender<AssetThumbnailWorkerResult>,
    receiver: Receiver<AssetThumbnailWorkerResult>,
    tick: u64,
    cpu_bytes: usize,
    decode_count: u64,
    cache_hit_count: u64,
    eviction_count: u64,
    diagnostics: Vec<AssetBrowserDiagnostic>,
}

impl Default for AssetThumbnailService {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            records: HashMap::new(),
            sender,
            receiver,
            tick: 0,
            cpu_bytes: 0,
            decode_count: 0,
            cache_hit_count: 0,
            eviction_count: 0,
            diagnostics: Vec::new(),
        }
    }
}

impl AssetThumbnailService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn request_ids(
        &mut self,
        project_root: &Path,
        catalog_entries: &[AssetBrowserEntry],
        thumbnail_ids: &BTreeSet<String>,
        requested_size: u32,
    ) -> usize {
        if thumbnail_ids.is_empty() {
            return 0;
        }
        let descriptors = catalog_entries
            .iter()
            .filter_map(|entry| {
                AssetThumbnailRequest::for_entry(
                    project_root,
                    catalog_entries,
                    entry,
                    requested_size,
                )
            })
            .map(|request| (request.thumbnail_id.clone(), request))
            .collect::<BTreeMap<_, _>>();
        let mut started = 0;
        for thumbnail_id in thumbnail_ids {
            let Some(request) = descriptors.get(thumbnail_id).cloned() else {
                continue;
            };
            if self.request(request) {
                started += 1;
            }
        }
        started
    }

    pub fn request(&mut self, request: AssetThumbnailRequest) -> bool {
        self.tick = self.tick.saturating_add(1);
        if let Some(record) = self.records.get_mut(&request.thumbnail_id) {
            record.last_used_tick = self.tick;
            if record.status == AssetPreviewStatus::Ready {
                self.cache_hit_count = self.cache_hit_count.saturating_add(1);
            }
            return false;
        }
        if self.pending_count() >= ASSET_THUMBNAIL_MAX_PENDING {
            return false;
        }
        self.evict_to_fit(1, 0, None);
        if self.records.len() >= ASSET_THUMBNAIL_MAX_ITEMS {
            return false;
        }

        let sender = self.sender.clone();
        let worker_request = request.clone();
        std::thread::spawn(move || {
            let result = decode_png_thumbnail(&worker_request);
            let _ = sender.send(AssetThumbnailWorkerResult {
                request: worker_request,
                result,
            });
        });
        self.records.insert(
            request.thumbnail_id.clone(),
            AssetThumbnailRecord {
                request,
                status: AssetPreviewStatus::Pending,
                aspect_ratio: None,
                payload: None,
                last_used_tick: self.tick,
            },
        );
        true
    }

    pub fn pump(&mut self) -> bool {
        let mut changed = false;
        loop {
            let worker = match self.receiver.try_recv() {
                Ok(worker) => worker,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            let Some(record) = self.records.get(&worker.request.thumbnail_id) else {
                continue;
            };
            if record.request != worker.request {
                continue;
            }
            changed = true;
            match worker.result {
                Ok(payload) => {
                    let byte_len = payload.byte_len();
                    self.evict_to_fit(0, byte_len, Some(&payload.thumbnail_id));
                    if self.cpu_bytes.saturating_add(byte_len) > ASSET_THUMBNAIL_MAX_CPU_BYTES {
                        let diagnostic = AssetBrowserDiagnostic::error(
                            "asset_thumbnail_cpu_budget_exceeded",
                            "Decoded thumbnail could not fit within the 64 MiB CPU cache budget.",
                            Some(payload.source_project_relative_path.clone()),
                        );
                        if let Some(record) = self.records.get_mut(&payload.thumbnail_id) {
                            record.status = AssetPreviewStatus::Failed;
                            record.payload = None;
                            record.aspect_ratio = None;
                        }
                        self.diagnostics.push(diagnostic);
                        continue;
                    }
                    self.cpu_bytes = self.cpu_bytes.saturating_add(byte_len);
                    self.decode_count = self.decode_count.saturating_add(1);
                    if let Some(record) = self.records.get_mut(&payload.thumbnail_id) {
                        record.status = AssetPreviewStatus::Ready;
                        record.aspect_ratio = payload.aspect_ratio();
                        record.payload = Some(payload);
                        record.last_used_tick = self.tick;
                    }
                }
                Err(diagnostic) => {
                    if let Some(record) = self.records.get_mut(&worker.request.thumbnail_id) {
                        record.status = AssetPreviewStatus::Failed;
                        record.payload = None;
                        record.aspect_ratio = None;
                    }
                    self.diagnostics.push(diagnostic);
                }
            }
        }
        changed
    }

    pub fn decorate_model(
        &self,
        project_root: &Path,
        catalog_entries: &[AssetBrowserEntry],
        model: &mut AssetBrowserModel,
    ) {
        for entry in &mut model.entries {
            let Some(request) = AssetThumbnailRequest::for_entry(
                project_root,
                catalog_entries,
                entry,
                model.thumbnail_size,
            ) else {
                if entry.preview.preview_kind == AssetPreviewKind::Thumbnail {
                    entry.preview.status = AssetPreviewStatus::NotAvailable;
                    entry.preview.thumbnail_id = None;
                    entry.preview.thumbnail_aspect_ratio = None;
                }
                continue;
            };
            entry.preview.thumbnail_id = Some(request.thumbnail_id.clone());
            if let Some(record) = self.records.get(&request.thumbnail_id) {
                entry.preview.status = record.status;
                entry.preview.thumbnail_aspect_ratio = record.aspect_ratio;
            } else {
                entry.preview.status = AssetPreviewStatus::Pending;
                entry.preview.thumbnail_aspect_ratio = None;
            }
        }
        model.report.diagnostics.extend(self.diagnostics.clone());
    }

    pub fn payloads_for_ids(
        &mut self,
        thumbnail_ids: &BTreeSet<String>,
    ) -> Vec<AssetThumbnailCpuPayload> {
        let mut payloads = Vec::new();
        for thumbnail_id in thumbnail_ids {
            let Some(record) = self.records.get_mut(thumbnail_id) else {
                continue;
            };
            let Some(payload) = record.payload.clone() else {
                continue;
            };
            self.tick = self.tick.saturating_add(1);
            record.last_used_tick = self.tick;
            payloads.push(payload);
        }
        payloads
    }

    pub fn summary(&self) -> AssetThumbnailServiceSummary {
        AssetThumbnailServiceSummary {
            record_count: self.records.len(),
            pending_count: self.pending_count(),
            ready_count: self
                .records
                .values()
                .filter(|record| record.status == AssetPreviewStatus::Ready)
                .count(),
            failed_count: self
                .records
                .values()
                .filter(|record| record.status == AssetPreviewStatus::Failed)
                .count(),
            cpu_bytes: self.cpu_bytes,
            decode_count: self.decode_count,
            cache_hit_count: self.cache_hit_count,
            eviction_count: self.eviction_count,
            diagnostics: self.diagnostics.clone(),
        }
    }

    pub fn pending_count(&self) -> usize {
        self.records
            .values()
            .filter(|record| record.status == AssetPreviewStatus::Pending)
            .count()
    }

    fn evict_to_fit(
        &mut self,
        additional_entries: usize,
        additional_bytes: usize,
        protected_id: Option<&str>,
    ) {
        while self.records.len().saturating_add(additional_entries) > ASSET_THUMBNAIL_MAX_ITEMS
            || self.cpu_bytes.saturating_add(additional_bytes) > ASSET_THUMBNAIL_MAX_CPU_BYTES
        {
            let candidate = self
                .records
                .iter()
                .filter(|(id, record)| {
                    protected_id != Some(id.as_str())
                        && record.status != AssetPreviewStatus::Pending
                })
                .min_by_key(|(_, record)| record.last_used_tick)
                .map(|(id, _)| id.clone());
            let Some(candidate) = candidate else {
                break;
            };
            if let Some(record) = self.records.remove(&candidate) {
                self.cpu_bytes = self.cpu_bytes.saturating_sub(
                    record
                        .payload
                        .as_ref()
                        .map_or(0, |payload| payload.byte_len()),
                );
                self.eviction_count = self.eviction_count.saturating_add(1);
            }
        }
    }
}

fn thumbnail_id(source_key: &str, content_hash: &str, requested_size: u32) -> String {
    let raw = format!("{source_key}\0{content_hash}\0{requested_size}");
    format!("asset-thumbnail::{}", stable_content_hash(raw.as_bytes()))
}

fn decode_png_thumbnail(
    request: &AssetThumbnailRequest,
) -> Result<AssetThumbnailCpuPayload, AssetBrowserDiagnostic> {
    if request
        .source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("png"))
    {
        return Err(AssetBrowserDiagnostic::warning(
            "asset_thumbnail_format_not_supported",
            "B-min+ thumbnail decode currently supports PNG source files only.",
            Some(request.source_project_relative_path.clone()),
        ));
    }
    let canonical_root = fs::canonicalize(&request.project_root).map_err(|error| {
        AssetBrowserDiagnostic::error(
            "asset_thumbnail_project_root_invalid",
            format!("Failed to resolve thumbnail project root: {error}"),
            Some(request.project_root.display().to_string()),
        )
    })?;
    let canonical_source = fs::canonicalize(&request.source_path).map_err(|error| {
        AssetBrowserDiagnostic::error(
            "asset_thumbnail_source_missing",
            format!("Failed to resolve thumbnail source: {error}"),
            Some(request.source_project_relative_path.clone()),
        )
    })?;
    if !canonical_source.starts_with(&canonical_root) {
        return Err(AssetBrowserDiagnostic::error(
            "asset_thumbnail_source_outside_project",
            "Thumbnail source resolved outside the active project root.",
            Some(request.source_project_relative_path.clone()),
        ));
    }
    let bytes = fs::read(&canonical_source).map_err(|error| {
        AssetBrowserDiagnostic::error(
            "asset_thumbnail_source_read_failed",
            format!("Failed to read thumbnail source: {error}"),
            Some(request.source_project_relative_path.clone()),
        )
    })?;
    let actual_hash = stable_content_hash(&bytes);
    if actual_hash != request.content_hash {
        return Err(AssetBrowserDiagnostic::warning(
            "asset_thumbnail_source_hash_changed",
            "Thumbnail source changed after indexing; refresh assets before decoding again.",
            Some(request.source_project_relative_path.clone()),
        ));
    }

    let mut decoder = png::Decoder::new(BufReader::new(bytes.as_slice()));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| {
        AssetBrowserDiagnostic::error(
            "asset_thumbnail_png_header_failed",
            format!("Failed to decode PNG header: {error}"),
            Some(request.source_project_relative_path.clone()),
        )
    })?;
    let mut decoded = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut decoded).map_err(|error| {
        AssetBrowserDiagnostic::error(
            "asset_thumbnail_png_decode_failed",
            format!("Failed to decode PNG pixels: {error}"),
            Some(request.source_project_relative_path.clone()),
        )
    })?;
    let rgba =
        png_output_to_rgba(&decoded[..info.buffer_size()], info.color_type).ok_or_else(|| {
            AssetBrowserDiagnostic::error(
                "asset_thumbnail_png_color_type_unsupported",
                format!("PNG color type {:?} is not supported.", info.color_type),
                Some(request.source_project_relative_path.clone()),
            )
        })?;
    let (width, height, rgba8) =
        resize_rgba_to_fit(info.width, info.height, &rgba, request.requested_size)?;
    Ok(AssetThumbnailCpuPayload {
        thumbnail_id: request.thumbnail_id.clone(),
        source_key: request.source_key.clone(),
        content_hash: request.content_hash.clone(),
        source_project_relative_path: request.source_project_relative_path.clone(),
        requested_size: request.requested_size,
        width,
        height,
        rgba8,
    })
}

fn png_output_to_rgba(bytes: &[u8], color_type: png::ColorType) -> Option<Vec<u8>> {
    match color_type {
        png::ColorType::Rgba => Some(bytes.to_vec()),
        png::ColorType::Rgb => Some(
            bytes
                .chunks_exact(3)
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
                .collect(),
        ),
        png::ColorType::Grayscale => Some(
            bytes
                .iter()
                .flat_map(|value| [*value, *value, *value, 255])
                .collect(),
        ),
        png::ColorType::GrayscaleAlpha => Some(
            bytes
                .chunks_exact(2)
                .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
                .collect(),
        ),
        png::ColorType::Indexed => None,
    }
}

fn resize_rgba_to_fit(
    source_width: u32,
    source_height: u32,
    source: &[u8],
    requested_size: u32,
) -> Result<(u32, u32, Vec<u8>), AssetBrowserDiagnostic> {
    if source_width == 0 || source_height == 0 {
        return Err(AssetBrowserDiagnostic::error(
            "asset_thumbnail_zero_sized_png",
            "PNG thumbnail source has zero width or height.",
            None,
        ));
    }
    let expected = source_width as usize * source_height as usize * 4;
    if source.len() != expected {
        return Err(AssetBrowserDiagnostic::error(
            "asset_thumbnail_rgba_size_mismatch",
            "Decoded PNG byte length does not match its dimensions.",
            None,
        ));
    }
    let scale = (requested_size as f64 / source_width as f64)
        .min(requested_size as f64 / source_height as f64)
        .min(1.0);
    let width = ((source_width as f64 * scale).round() as u32).max(1);
    let height = ((source_height as f64 * scale).round() as u32).max(1);
    if width == source_width && height == source_height {
        return Ok((width, height, source.to_vec()));
    }
    let mut output = vec![0; width as usize * height as usize * 4];
    for y in 0..height {
        let source_y = ((y as u64 * source_height as u64) / height as u64)
            .min(source_height.saturating_sub(1) as u64) as u32;
        for x in 0..width {
            let source_x = ((x as u64 * source_width as u64) / width as u64)
                .min(source_width.saturating_sub(1) as u64) as u32;
            let source_offset = ((source_y * source_width + source_x) * 4) as usize;
            let target_offset = ((y * width + x) * 4) as usize;
            output[target_offset..target_offset + 4]
                .copy_from_slice(&source[source_offset..source_offset + 4]);
        }
    }
    Ok((width, height, output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_ui_model::{AssetEntryKey, AssetKind};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    #[test]
    fn asset_thumbnail_decodes_png_and_preserves_non_empty_alpha_pixels() {
        let fixture = ThumbnailFixture::new("decode");
        let source = fixture.write_png("Assets/icon.png", 4, 2);
        let request = fixture.request(&source, 96);
        let mut service = AssetThumbnailService::new();

        assert!(service.request(request.clone()));
        wait_for_service(&mut service);
        let payloads = service.payloads_for_ids(&BTreeSet::from([request.thumbnail_id.clone()]));

        assert_eq!(payloads.len(), 1);
        assert_eq!((payloads[0].width, payloads[0].height), (4, 2));
        assert!(payloads[0]
            .rgba8
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0 && pixel[..3] != [0, 0, 0]));
        assert_eq!(service.summary().decode_count, 1);
        assert!(!service.request(request));
        assert_eq!(service.summary().decode_count, 1);
    }

    #[test]
    fn asset_thumbnail_request_key_changes_with_hash_or_size() {
        let fixture = ThumbnailFixture::new("key");
        let source = fixture.write_png("Assets/icon.png", 2, 2);
        let first = fixture.request(&source, 64);
        let resized = fixture.request(&source, 96);
        let mut changed_hash = first.clone();
        changed_hash.content_hash = "fnv1a64:0000000000000000".to_string();
        changed_hash.thumbnail_id = thumbnail_id(
            &changed_hash.source_key,
            &changed_hash.content_hash,
            changed_hash.requested_size,
        );

        assert_ne!(first.thumbnail_id, resized.thumbnail_id);
        assert_ne!(first.thumbnail_id, changed_hash.thumbnail_id);
    }

    #[test]
    fn asset_thumbnail_pending_and_cache_budgets_are_bounded() {
        let fixture = ThumbnailFixture::new("budget");
        let source = fixture.write_png("Assets/icon.png", 2, 2);
        let mut service = AssetThumbnailService::new();
        for index in 0..64 {
            let mut request = fixture.request(&source, 32);
            request.thumbnail_id = format!("thumbnail-{index}");
            request.source_key = format!("source-{index}");
            service.request(request);
        }

        assert!(service.summary().pending_count <= ASSET_THUMBNAIL_MAX_PENDING);
        assert!(service.summary().record_count <= ASSET_THUMBNAIL_MAX_ITEMS);
    }

    fn wait_for_service(service: &mut AssetThumbnailService) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while service.summary().pending_count > 0 && Instant::now() < deadline {
            service.pump();
            std::thread::sleep(Duration::from_millis(5));
        }
        service.pump();
        assert_eq!(service.summary().pending_count, 0);
    }

    struct ThumbnailFixture {
        root: PathBuf,
    }

    impl ThumbnailFixture {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "aife-asset-thumbnail-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(root.join("Assets")).expect("create fixture");
            Self { root }
        }

        fn write_png(&self, relative: &str, width: u32, height: u32) -> AssetBrowserEntry {
            let path = self.root.join(relative);
            let file = fs::File::create(&path).expect("create png");
            let mut encoder = png::Encoder::new(file, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("png header");
            let pixels = (0..width * height)
                .flat_map(|index| [255, (index % 255) as u8, 32, 255])
                .collect::<Vec<_>>();
            writer.write_image_data(&pixels).expect("png pixels");
            drop(writer);
            let bytes = fs::read(&path).expect("read png");
            let mut entry = AssetBrowserEntry::new(relative, "icon.png", AssetKind::Texture);
            entry.entry_key = AssetEntryKey::SourceFile {
                canonical_project_relative_path: relative.to_string(),
                content_hash: Some(stable_content_hash(&bytes)),
            };
            entry.preview = editor_ui_model::AssetPreviewDescriptor::for_kind(AssetKind::Texture);
            entry.preview.thumbnail_source_path = Some(relative.to_string());
            entry
        }

        fn request(&self, entry: &AssetBrowserEntry, size: u32) -> AssetThumbnailRequest {
            AssetThumbnailRequest::for_entry(&self.root, std::slice::from_ref(entry), entry, size)
                .expect("thumbnail request")
        }
    }

    impl Drop for ThumbnailFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
