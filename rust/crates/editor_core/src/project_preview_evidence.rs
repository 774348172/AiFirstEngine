use crate::{ProjectRelativePath, ProjectWriteScope};
use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

pub const PROJECT_PREVIEW_FRAME_TICKET_SCHEMA_VERSION: &str = "project-preview-frame-ticket.v1";
pub const PROJECT_PREVIEW_FRAME_EVIDENCE_SCHEMA_VERSION: &str = "project-preview-frame-evidence.v1";
pub const PROJECT_PREVIEW_EVIDENCE_ROOT: &str = "Library/AiCapability/Preview";
const PROJECT_PREVIEW_FRAME_PNG_FILE: &str = "frame.png";
const PROJECT_PREVIEW_FRAME_EVIDENCE_FILE: &str = "frame-evidence.json";
const PROJECT_PREVIEW_MAX_DIMENSION: u32 = 16_384;
const PROJECT_PREVIEW_MAX_RGBA_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectPreviewCaptureKind {
    RealWgpuExactSharedTexture,
    DeterministicTestAdapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectPreviewPixelFormat {
    Rgba8Unorm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectPreviewFrameTicket {
    pub schema_version: String,
    pub operation_id: String,
    pub project_identity: String,
    pub expected_project_digest: String,
    pub game_view_session_id: String,
    pub expected_texture_id: String,
    pub expected_frame_index: u64,
    pub expected_runtime_frame_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPreviewFrameCapture {
    pub project_digest: String,
    pub game_view_session_id: String,
    pub texture_id: String,
    pub frame_index: u64,
    pub runtime_frame_hash: String,
    pub width: u32,
    pub height: u32,
    pub pixel_format: ProjectPreviewPixelFormat,
    pub capture_kind: ProjectPreviewCaptureKind,
    pub present_report_ref: String,
    pub rgba8: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPreviewFrameReadback {
    pub game_view_session_id: String,
    pub texture_id: String,
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub pixel_format: ProjectPreviewPixelFormat,
    pub capture_kind: ProjectPreviewCaptureKind,
    pub rgba8: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectPreviewFrameResultStatus {
    Captured,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPreviewFrameResult {
    pub operation_id: String,
    pub status: ProjectPreviewFrameResultStatus,
    pub evidence_ref: Option<String>,
    pub captured_evidence: Option<ProjectPreviewFrameEvidence>,
    pub diagnostic_code: Option<String>,
    pub diagnostic_message: Option<String>,
}

impl ProjectPreviewFrameResult {
    pub fn captured(
        evidence_ref: impl Into<String>,
        captured_evidence: ProjectPreviewFrameEvidence,
    ) -> Self {
        Self {
            operation_id: captured_evidence.operation_id.clone(),
            status: ProjectPreviewFrameResultStatus::Captured,
            evidence_ref: Some(evidence_ref.into()),
            captured_evidence: Some(captured_evidence),
            diagnostic_code: None,
            diagnostic_message: None,
        }
    }

    pub fn failed(
        operation_id: impl Into<String>,
        diagnostic_code: impl Into<String>,
        diagnostic_message: impl Into<String>,
    ) -> Self {
        Self {
            operation_id: operation_id.into(),
            status: ProjectPreviewFrameResultStatus::Failed,
            evidence_ref: None,
            captured_evidence: None,
            diagnostic_code: Some(diagnostic_code.into()),
            diagnostic_message: Some(diagnostic_message.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectPreviewFrameEvidence {
    pub schema_version: String,
    pub operation_id: String,
    pub project_identity: String,
    pub project_digest: String,
    pub game_view_session_id: String,
    pub texture_id: String,
    pub frame_index: u64,
    pub frame_digest: String,
    pub runtime_frame_hash: String,
    pub screenshot_ref: String,
    pub screenshot_digest: String,
    pub width: u32,
    pub height: u32,
    pub pixel_format: ProjectPreviewPixelFormat,
    pub capture_kind: ProjectPreviewCaptureKind,
    pub present_report_ref: String,
    pub evidence_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPreviewEvidenceError {
    pub code: &'static str,
    pub message: String,
    pub evidence_ref: Option<String>,
}

impl std::fmt::Display for ProjectPreviewEvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProjectPreviewEvidenceError {}

impl ProjectPreviewEvidenceError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            evidence_ref: None,
        }
    }

    fn with_ref(mut self, evidence_ref: impl Into<String>) -> Self {
        self.evidence_ref = Some(evidence_ref.into());
        self
    }
}

pub struct ProjectPreviewEvidence;

impl ProjectPreviewEvidence {
    pub fn validate_ticket(
        ticket: &ProjectPreviewFrameTicket,
    ) -> Result<(), ProjectPreviewEvidenceError> {
        validate_ticket(ticket)
    }

    pub fn frame_evidence_ref(operation_id: &str) -> Result<String, ProjectPreviewEvidenceError> {
        validate_operation_id(operation_id)?;
        Ok(format!(
            "{PROJECT_PREVIEW_EVIDENCE_ROOT}/{operation_id}/{PROJECT_PREVIEW_FRAME_EVIDENCE_FILE}"
        ))
    }

    pub fn screenshot_ref(operation_id: &str) -> Result<String, ProjectPreviewEvidenceError> {
        validate_operation_id(operation_id)?;
        Ok(format!(
            "{PROJECT_PREVIEW_EVIDENCE_ROOT}/{operation_id}/{PROJECT_PREVIEW_FRAME_PNG_FILE}"
        ))
    }

    pub fn persist_frame(
        scope: &ProjectWriteScope,
        ticket: &ProjectPreviewFrameTicket,
        capture: ProjectPreviewFrameCapture,
    ) -> Result<ProjectPreviewFrameEvidence, ProjectPreviewEvidenceError> {
        validate_ticket(ticket)?;
        validate_capture(ticket, &capture)?;

        let screenshot_ref = Self::screenshot_ref(&ticket.operation_id)?;
        let evidence_ref = Self::frame_evidence_ref(&ticket.operation_id)?;
        let operation_root = format!("{PROJECT_PREVIEW_EVIDENCE_ROOT}/{}", ticket.operation_id);
        let png_bytes = encode_rgba_png(capture.width, capture.height, &capture.rgba8)?;
        let mut evidence = ProjectPreviewFrameEvidence {
            schema_version: PROJECT_PREVIEW_FRAME_EVIDENCE_SCHEMA_VERSION.to_string(),
            operation_id: ticket.operation_id.clone(),
            project_identity: ticket.project_identity.clone(),
            project_digest: capture.project_digest,
            game_view_session_id: capture.game_view_session_id,
            texture_id: capture.texture_id,
            frame_index: capture.frame_index,
            frame_digest: sha256_prefixed(&capture.rgba8),
            runtime_frame_hash: capture.runtime_frame_hash,
            screenshot_ref,
            screenshot_digest: sha256_prefixed(&png_bytes),
            width: capture.width,
            height: capture.height,
            pixel_format: capture.pixel_format,
            capture_kind: capture.capture_kind,
            present_report_ref: capture.present_report_ref,
            evidence_digest: String::new(),
        };
        evidence.evidence_digest = evidence_digest(&evidence)?;
        let metadata_bytes = serde_json::to_vec_pretty(&evidence).map_err(|error| {
            ProjectPreviewEvidenceError::new(
                "project_preview_evidence.metadata_encode_failed",
                format!("Preview frame metadata could not be encoded: {error}"),
            )
            .with_ref(&evidence_ref)
        })?;

        scope
            .publish_directory_atomic(&operation_root, |writer| {
                writer.write_atomic(PROJECT_PREVIEW_FRAME_PNG_FILE, &png_bytes)?;
                writer.write_atomic(PROJECT_PREVIEW_FRAME_EVIDENCE_FILE, &metadata_bytes)?;
                Ok(())
            })
            .map_err(|error| {
                ProjectPreviewEvidenceError::new(
                    "project_preview_evidence.project_write_failed",
                    error.to_string(),
                )
                .with_ref(error.relative_path.unwrap_or(operation_root))
            })?;

        Self::validate_frame(scope, ticket, &evidence_ref)
    }

    pub fn read_frame(
        scope: &ProjectWriteScope,
        evidence_ref: &str,
    ) -> Result<ProjectPreviewFrameEvidence, ProjectPreviewEvidenceError> {
        let normalized_ref = normalize_project_ref(evidence_ref, "frame evidence")?;
        let metadata_bytes = scope.read(normalized_ref.as_path()).map_err(|error| {
            ProjectPreviewEvidenceError::new(
                "project_preview_evidence.metadata_read_failed",
                error.to_string(),
            )
            .with_ref(normalized_ref.as_str())
        })?;
        let evidence: ProjectPreviewFrameEvidence = serde_json::from_slice(&metadata_bytes)
            .map_err(|error| {
                ProjectPreviewEvidenceError::new(
                    "project_preview_evidence.metadata_decode_failed",
                    format!("Preview frame metadata is not strict valid JSON: {error}"),
                )
                .with_ref(normalized_ref.as_str())
            })?;
        validate_evidence_fields(&evidence, normalized_ref.as_str())?;

        let expected_evidence_ref = Self::frame_evidence_ref(&evidence.operation_id)?;
        if normalized_ref.as_str() != expected_evidence_ref {
            return Err(ProjectPreviewEvidenceError::new(
                "project_preview_evidence.evidence_ref_mismatch",
                "Frame evidence metadata is not stored at its operation-owned path.",
            )
            .with_ref(normalized_ref.as_str()));
        }
        if evidence_digest(&evidence)? != evidence.evidence_digest {
            return Err(ProjectPreviewEvidenceError::new(
                "project_preview_evidence.evidence_digest_mismatch",
                "Frame evidence metadata no longer matches its canonical digest.",
            )
            .with_ref(normalized_ref.as_str()));
        }

        let expected_screenshot_ref = Self::screenshot_ref(&evidence.operation_id)?;
        if evidence.screenshot_ref != expected_screenshot_ref {
            return Err(ProjectPreviewEvidenceError::new(
                "project_preview_evidence.screenshot_ref_mismatch",
                "Frame evidence screenshot reference is not operation-owned.",
            )
            .with_ref(&evidence.screenshot_ref));
        }
        let screenshot_ref = normalize_project_ref(&evidence.screenshot_ref, "screenshot")?;
        let png_bytes = scope.read(screenshot_ref.as_path()).map_err(|error| {
            ProjectPreviewEvidenceError::new(
                "project_preview_evidence.screenshot_read_failed",
                error.to_string(),
            )
            .with_ref(screenshot_ref.as_str())
        })?;
        if sha256_prefixed(&png_bytes) != evidence.screenshot_digest {
            return Err(ProjectPreviewEvidenceError::new(
                "project_preview_evidence.screenshot_digest_mismatch",
                "Screenshot PNG bytes no longer match the recorded digest.",
            )
            .with_ref(screenshot_ref.as_str()));
        }
        let decoded = decode_rgba_png(&png_bytes, evidence.width, evidence.height)?;
        if sha256_prefixed(&decoded) != evidence.frame_digest {
            return Err(ProjectPreviewEvidenceError::new(
                "project_preview_evidence.frame_digest_mismatch",
                "Decoded screenshot pixels no longer match the recorded frame digest.",
            )
            .with_ref(screenshot_ref.as_str()));
        }
        Ok(evidence)
    }

    pub fn validate_frame(
        scope: &ProjectWriteScope,
        ticket: &ProjectPreviewFrameTicket,
        evidence_ref: &str,
    ) -> Result<ProjectPreviewFrameEvidence, ProjectPreviewEvidenceError> {
        validate_ticket(ticket)?;
        let normalized_ref = normalize_project_ref(evidence_ref, "frame evidence")?;
        let expected_ref = Self::frame_evidence_ref(&ticket.operation_id)?;
        if normalized_ref.as_str() != expected_ref {
            return Err(ProjectPreviewEvidenceError::new(
                "project_preview_evidence.ticket_evidence_ref_mismatch",
                "Frame evidence reference does not belong to the pending operation ticket.",
            )
            .with_ref(normalized_ref.as_str()));
        }
        let evidence = Self::read_frame(scope, normalized_ref.as_str())?;
        ensure_match(
            evidence.operation_id == ticket.operation_id,
            "project_preview_evidence.operation_mismatch",
            "Frame evidence operation does not match the pending ticket.",
            evidence_ref,
        )?;
        ensure_match(
            evidence.project_identity == ticket.project_identity,
            "project_preview_evidence.project_identity_mismatch",
            "Frame evidence project identity does not match the pending ticket.",
            evidence_ref,
        )?;
        ensure_match(
            evidence.project_digest == ticket.expected_project_digest,
            "project_preview_evidence.project_digest_mismatch",
            "Frame evidence project digest does not match the pending ticket.",
            evidence_ref,
        )?;
        ensure_match(
            evidence.game_view_session_id == ticket.game_view_session_id,
            "project_preview_evidence.game_view_session_mismatch",
            "Frame evidence GameView session does not match the pending ticket.",
            evidence_ref,
        )?;
        ensure_match(
            evidence.texture_id == ticket.expected_texture_id,
            "project_preview_evidence.texture_mismatch",
            "Frame evidence texture does not match the pending ticket.",
            evidence_ref,
        )?;
        ensure_match(
            evidence.frame_index == ticket.expected_frame_index,
            "project_preview_evidence.frame_index_mismatch",
            "Frame evidence index does not match the pending ticket.",
            evidence_ref,
        )?;
        ensure_match(
            evidence.runtime_frame_hash == ticket.expected_runtime_frame_hash,
            "project_preview_evidence.runtime_frame_hash_mismatch",
            "Frame evidence runtime hash does not match the pending ticket.",
            evidence_ref,
        )?;
        Ok(evidence)
    }
}

fn validate_ticket(ticket: &ProjectPreviewFrameTicket) -> Result<(), ProjectPreviewEvidenceError> {
    if ticket.schema_version != PROJECT_PREVIEW_FRAME_TICKET_SCHEMA_VERSION {
        return Err(ProjectPreviewEvidenceError::new(
            "project_preview_evidence.ticket_schema_mismatch",
            "Preview frame ticket schema version is unsupported.",
        ));
    }
    validate_operation_id(&ticket.operation_id)?;
    validate_non_empty(&ticket.project_identity, "ticket project identity")?;
    validate_digest(&ticket.expected_project_digest, "ticket project digest")?;
    validate_non_empty(&ticket.game_view_session_id, "ticket GameView session")?;
    validate_non_empty(&ticket.expected_texture_id, "ticket texture id")?;
    validate_non_empty(
        &ticket.expected_runtime_frame_hash,
        "ticket runtime frame hash",
    )?;
    Ok(())
}

fn validate_capture(
    ticket: &ProjectPreviewFrameTicket,
    capture: &ProjectPreviewFrameCapture,
) -> Result<(), ProjectPreviewEvidenceError> {
    validate_digest(&capture.project_digest, "capture project digest")?;
    ensure_match(
        capture.project_digest == ticket.expected_project_digest,
        "project_preview_evidence.project_digest_mismatch",
        "Captured project digest does not match the pending ticket.",
        &ticket.operation_id,
    )?;
    ensure_match(
        capture.game_view_session_id == ticket.game_view_session_id,
        "project_preview_evidence.game_view_session_mismatch",
        "Captured GameView session does not match the pending ticket.",
        &ticket.operation_id,
    )?;
    ensure_match(
        capture.texture_id == ticket.expected_texture_id,
        "project_preview_evidence.texture_mismatch",
        "Captured texture does not match the pending ticket.",
        &ticket.operation_id,
    )?;
    ensure_match(
        capture.frame_index == ticket.expected_frame_index,
        "project_preview_evidence.frame_index_mismatch",
        "Captured frame index does not match the pending ticket.",
        &ticket.operation_id,
    )?;
    ensure_match(
        capture.runtime_frame_hash == ticket.expected_runtime_frame_hash,
        "project_preview_evidence.runtime_frame_hash_mismatch",
        "Captured runtime frame hash does not match the pending ticket.",
        &ticket.operation_id,
    )?;
    let expected_len = expected_rgba_len(capture.width, capture.height)?;
    if capture.rgba8.len() != expected_len {
        return Err(ProjectPreviewEvidenceError::new(
            "project_preview_evidence.invalid_pixel_length",
            format!(
                "RGBA readback has {} bytes; {expected_len} bytes are required for {}x{}.",
                capture.rgba8.len(),
                capture.width,
                capture.height
            ),
        ));
    }
    normalize_project_ref(&capture.present_report_ref, "present report")?;
    Ok(())
}

fn validate_evidence_fields(
    evidence: &ProjectPreviewFrameEvidence,
    evidence_ref: &str,
) -> Result<(), ProjectPreviewEvidenceError> {
    if evidence.schema_version != PROJECT_PREVIEW_FRAME_EVIDENCE_SCHEMA_VERSION {
        return Err(ProjectPreviewEvidenceError::new(
            "project_preview_evidence.schema_mismatch",
            "Preview frame evidence schema version is unsupported.",
        )
        .with_ref(evidence_ref));
    }
    validate_operation_id(&evidence.operation_id)?;
    validate_non_empty(&evidence.project_identity, "evidence project identity")?;
    validate_digest(&evidence.project_digest, "evidence project digest")?;
    validate_non_empty(&evidence.game_view_session_id, "evidence GameView session")?;
    validate_non_empty(&evidence.texture_id, "evidence texture id")?;
    validate_non_empty(&evidence.runtime_frame_hash, "evidence runtime frame hash")?;
    validate_digest(&evidence.frame_digest, "frame digest")?;
    validate_digest(&evidence.screenshot_digest, "screenshot digest")?;
    validate_digest(&evidence.evidence_digest, "evidence digest")?;
    expected_rgba_len(evidence.width, evidence.height)?;
    normalize_project_ref(&evidence.present_report_ref, "present report")?;
    Ok(())
}

fn validate_operation_id(operation_id: &str) -> Result<(), ProjectPreviewEvidenceError> {
    let valid = !operation_id.is_empty()
        && operation_id.len() <= 128
        && operation_id != "."
        && operation_id != ".."
        && operation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid {
        Ok(())
    } else {
        Err(ProjectPreviewEvidenceError::new(
            "project_preview_evidence.operation_id_invalid",
            "Operation id must be one bounded path-safe ASCII segment.",
        ))
    }
}

fn validate_non_empty(value: &str, role: &str) -> Result<(), ProjectPreviewEvidenceError> {
    if !value.trim().is_empty() && value == value.trim() {
        Ok(())
    } else {
        Err(ProjectPreviewEvidenceError::new(
            "project_preview_evidence.field_invalid",
            format!("{role} must be a non-empty canonical string."),
        ))
    }
}

fn validate_digest(value: &str, role: &str) -> Result<(), ProjectPreviewEvidenceError> {
    let valid = value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if valid {
        Ok(())
    } else {
        Err(ProjectPreviewEvidenceError::new(
            "project_preview_evidence.digest_invalid",
            format!("{role} is not a canonical SHA-256 digest."),
        ))
    }
}

fn expected_rgba_len(width: u32, height: u32) -> Result<usize, ProjectPreviewEvidenceError> {
    if width == 0
        || height == 0
        || width > PROJECT_PREVIEW_MAX_DIMENSION
        || height > PROJECT_PREVIEW_MAX_DIMENSION
    {
        return Err(ProjectPreviewEvidenceError::new(
            "project_preview_evidence.invalid_dimensions",
            format!("Preview frame dimensions {width}x{height} are invalid or unbounded."),
        ));
    }
    let byte_len = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .filter(|byte_len| *byte_len <= PROJECT_PREVIEW_MAX_RGBA_BYTES)
        .ok_or_else(|| {
            ProjectPreviewEvidenceError::new(
                "project_preview_evidence.invalid_dimensions",
                format!("Preview frame dimensions {width}x{height} exceed the evidence budget."),
            )
        })?;
    Ok(byte_len)
}

fn normalize_project_ref(
    value: &str,
    role: &str,
) -> Result<ProjectRelativePath, ProjectPreviewEvidenceError> {
    ProjectRelativePath::parse(value).map_err(|error| {
        ProjectPreviewEvidenceError::new(
            "project_preview_evidence.path_invalid",
            format!("{role} reference is not project-contained: {error}"),
        )
        .with_ref(value)
    })
}

fn ensure_match(
    matches: bool,
    code: &'static str,
    message: &'static str,
    evidence_ref: &str,
) -> Result<(), ProjectPreviewEvidenceError> {
    if matches {
        Ok(())
    } else {
        Err(ProjectPreviewEvidenceError::new(code, message).with_ref(evidence_ref))
    }
}

fn evidence_digest(
    evidence: &ProjectPreviewFrameEvidence,
) -> Result<String, ProjectPreviewEvidenceError> {
    let mut unsigned = evidence.clone();
    unsigned.evidence_digest.clear();
    let value = serde_json::to_value(unsigned).map_err(|error| {
        ProjectPreviewEvidenceError::new(
            "project_preview_evidence.metadata_encode_failed",
            format!("Preview frame metadata could not be canonicalized: {error}"),
        )
    })?;
    let bytes = canonical_json_bytes(&value).map_err(|error| {
        ProjectPreviewEvidenceError::new(
            "project_preview_evidence.metadata_encode_failed",
            format!("Preview frame metadata could not be canonicalized: {error}"),
        )
    })?;
    Ok(sha256_prefixed(&bytes))
}

fn encode_rgba_png(
    width: u32,
    height: u32,
    rgba8: &[u8],
) -> Result<Vec<u8>, ProjectPreviewEvidenceError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| {
            ProjectPreviewEvidenceError::new(
                "project_preview_evidence.png_encode_failed",
                format!("Screenshot PNG header could not be encoded: {error}"),
            )
        })?;
        writer.write_image_data(rgba8).map_err(|error| {
            ProjectPreviewEvidenceError::new(
                "project_preview_evidence.png_encode_failed",
                format!("Screenshot PNG pixels could not be encoded: {error}"),
            )
        })?;
    }
    Ok(bytes)
}

fn decode_rgba_png(
    bytes: &[u8],
    expected_width: u32,
    expected_height: u32,
) -> Result<Vec<u8>, ProjectPreviewEvidenceError> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|error| {
        ProjectPreviewEvidenceError::new(
            "project_preview_evidence.png_decode_failed",
            format!("Screenshot is not a decodable PNG: {error}"),
        )
    })?;
    let source = reader.info();
    if source.width != expected_width || source.height != expected_height {
        return Err(ProjectPreviewEvidenceError::new(
            "project_preview_evidence.png_dimensions_mismatch",
            format!(
                "Screenshot PNG is {}x{}, expected {expected_width}x{expected_height}.",
                source.width, source.height
            ),
        ));
    }
    if source.color_type != png::ColorType::Rgba || source.bit_depth != png::BitDepth::Eight {
        return Err(ProjectPreviewEvidenceError::new(
            "project_preview_evidence.png_format_mismatch",
            "Screenshot PNG must be encoded as 8-bit RGBA.",
        ));
    }
    let expected_len = expected_rgba_len(expected_width, expected_height)?;
    if reader.output_buffer_size() != expected_len {
        return Err(ProjectPreviewEvidenceError::new(
            "project_preview_evidence.invalid_pixel_length",
            "Screenshot PNG output length does not match its dimensions.",
        ));
    }
    let mut decoded = vec![0; expected_len];
    let output = reader.next_frame(&mut decoded).map_err(|error| {
        ProjectPreviewEvidenceError::new(
            "project_preview_evidence.png_decode_failed",
            format!("Screenshot PNG pixels could not be decoded: {error}"),
        )
    })?;
    if output.width != expected_width
        || output.height != expected_height
        || output.color_type != png::ColorType::Rgba
        || output.bit_depth != png::BitDepth::Eight
        || output.buffer_size() != expected_len
    {
        return Err(ProjectPreviewEvidenceError::new(
            "project_preview_evidence.png_format_mismatch",
            "Decoded screenshot PNG does not match the recorded RGBA8 frame contract.",
        ));
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempProject(PathBuf);

    impl TempProject {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "aife-project-preview-evidence-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&root).expect("fixture project root");
            Self(root)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn ticket() -> ProjectPreviewFrameTicket {
        ProjectPreviewFrameTicket {
            schema_version: PROJECT_PREVIEW_FRAME_TICKET_SCHEMA_VERSION.to_string(),
            operation_id: "operation-preview-1".to_string(),
            project_identity: "project.preview.fixture".to_string(),
            expected_project_digest: digest('a'),
            game_view_session_id: "game-view-session-1".to_string(),
            expected_texture_id: "viewport-main::frame-7".to_string(),
            expected_frame_index: 7,
            expected_runtime_frame_hash: "runtime-frame-hash-7".to_string(),
        }
    }

    fn capture(width: u32, height: u32, rgba8: Vec<u8>) -> ProjectPreviewFrameCapture {
        ProjectPreviewFrameCapture {
            project_digest: digest('a'),
            game_view_session_id: "game-view-session-1".to_string(),
            texture_id: "viewport-main::frame-7".to_string(),
            frame_index: 7,
            runtime_frame_hash: "runtime-frame-hash-7".to_string(),
            width,
            height,
            pixel_format: ProjectPreviewPixelFormat::Rgba8Unorm,
            capture_kind: ProjectPreviewCaptureKind::DeterministicTestAdapter,
            present_report_ref: "Library/Reports/editor-gameview-present-report.json".to_string(),
            rgba8,
        }
    }

    fn persist_fixture(
        label: &str,
    ) -> (
        TempProject,
        ProjectWriteScope,
        ProjectPreviewFrameTicket,
        ProjectPreviewFrameEvidence,
    ) {
        let project = TempProject::new(label);
        let scope = ProjectWriteScope::open(project.path()).expect("write scope");
        let ticket = ticket();
        let evidence =
            ProjectPreviewEvidence::persist_frame(&scope, &ticket, capture(2, 2, vec![0; 16]))
                .expect("persist frame evidence");
        (project, scope, ticket, evidence)
    }

    #[test]
    fn preview_frame_evidence_round_trips_black_frame_atomically() {
        let (project, scope, ticket, evidence) = persist_fixture("round-trip");
        let evidence_ref =
            ProjectPreviewEvidence::frame_evidence_ref(&ticket.operation_id).expect("evidence ref");

        assert_eq!(evidence.width, 2);
        assert_eq!(evidence.height, 2);
        assert!(project.path().join(&evidence.screenshot_ref).is_file());
        assert!(project.path().join(&evidence_ref).is_file());
        assert_eq!(
            ProjectPreviewEvidence::validate_frame(&scope, &ticket, &evidence_ref)
                .expect("validated frame"),
            evidence
        );
    }

    #[test]
    fn preview_frame_evidence_rejects_ticket_mismatches_and_path_escape() {
        let (_project, scope, ticket, _evidence) = persist_fixture("ticket-mismatch");
        let evidence_ref =
            ProjectPreviewEvidence::frame_evidence_ref(&ticket.operation_id).expect("evidence ref");

        let cases = [
            ("project", {
                let mut value = ticket.clone();
                value.project_identity = "another-project".to_string();
                value
            }),
            ("digest", {
                let mut value = ticket.clone();
                value.expected_project_digest = digest('b');
                value
            }),
            ("session", {
                let mut value = ticket.clone();
                value.game_view_session_id = "other-session".to_string();
                value
            }),
            ("texture", {
                let mut value = ticket.clone();
                value.expected_texture_id = "other-texture".to_string();
                value
            }),
            ("frame", {
                let mut value = ticket.clone();
                value.expected_frame_index = 8;
                value
            }),
            ("hash", {
                let mut value = ticket.clone();
                value.expected_runtime_frame_hash = "other-frame-hash".to_string();
                value
            }),
        ];
        for (label, wrong_ticket) in cases {
            assert!(
                ProjectPreviewEvidence::validate_frame(&scope, &wrong_ticket, &evidence_ref)
                    .is_err(),
                "{label} mismatch must fail closed"
            );
        }

        let mut wrong_operation = ticket.clone();
        wrong_operation.operation_id = "operation-preview-2".to_string();
        assert!(
            ProjectPreviewEvidence::validate_frame(&scope, &wrong_operation, &evidence_ref)
                .is_err()
        );
        assert!(ProjectPreviewEvidence::read_frame(&scope, "../frame-evidence.json").is_err());
    }

    #[test]
    fn preview_frame_evidence_rejects_png_and_metadata_tampering() {
        let (project, scope, ticket, evidence) = persist_fixture("tamper-png");
        let evidence_ref =
            ProjectPreviewEvidence::frame_evidence_ref(&ticket.operation_id).expect("evidence ref");
        let png_path = project.path().join(&evidence.screenshot_ref);
        let mut png = fs::read(&png_path).expect("png bytes");
        let last = png.last_mut().expect("non-empty png");
        *last ^= 0x01;
        fs::write(&png_path, png).expect("tamper png");
        assert_eq!(
            ProjectPreviewEvidence::validate_frame(&scope, &ticket, &evidence_ref)
                .expect_err("tampered png")
                .code,
            "project_preview_evidence.screenshot_digest_mismatch"
        );

        let (project, scope, ticket, _evidence) = persist_fixture("tamper-metadata");
        let evidence_ref =
            ProjectPreviewEvidence::frame_evidence_ref(&ticket.operation_id).expect("evidence ref");
        let metadata_path = project.path().join(&evidence_ref);
        let mut metadata: Value =
            serde_json::from_slice(&fs::read(&metadata_path).expect("metadata bytes"))
                .expect("metadata json");
        metadata["width"] = Value::from(3);
        fs::write(
            &metadata_path,
            serde_json::to_vec_pretty(&metadata).expect("tampered metadata json"),
        )
        .expect("tamper metadata");
        assert_eq!(
            ProjectPreviewEvidence::validate_frame(&scope, &ticket, &evidence_ref)
                .expect_err("tampered metadata")
                .code,
            "project_preview_evidence.evidence_digest_mismatch"
        );

        let (project, scope, ticket, evidence) = persist_fixture("invalid-png");
        let evidence_ref =
            ProjectPreviewEvidence::frame_evidence_ref(&ticket.operation_id).expect("evidence ref");
        let invalid_png = b"not-a-png";
        fs::write(project.path().join(&evidence.screenshot_ref), invalid_png)
            .expect("replace png encoding");
        let metadata_path = project.path().join(&evidence_ref);
        let mut metadata: ProjectPreviewFrameEvidence =
            serde_json::from_slice(&fs::read(&metadata_path).expect("metadata bytes"))
                .expect("metadata json");
        metadata.screenshot_digest = sha256_prefixed(invalid_png);
        metadata.evidence_digest.clear();
        metadata.evidence_digest = evidence_digest(&metadata).expect("metadata digest");
        fs::write(
            &metadata_path,
            serde_json::to_vec_pretty(&metadata).expect("invalid png metadata json"),
        )
        .expect("update invalid png metadata");
        assert_eq!(
            ProjectPreviewEvidence::validate_frame(&scope, &ticket, &evidence_ref)
                .expect_err("invalid png encoding")
                .code,
            "project_preview_evidence.png_decode_failed"
        );
    }

    #[test]
    fn preview_frame_evidence_rejects_artifact_copied_to_another_project() {
        let (source_project, _source_scope, ticket, evidence) = persist_fixture("cross-project-a");
        let target_project = TempProject::new("cross-project-b");
        let evidence_ref =
            ProjectPreviewEvidence::frame_evidence_ref(&ticket.operation_id).expect("evidence ref");
        let target_evidence_path = target_project.path().join(&evidence_ref);
        fs::create_dir_all(
            target_evidence_path
                .parent()
                .expect("target evidence parent"),
        )
        .expect("target evidence directory");
        fs::copy(
            source_project.path().join(&evidence_ref),
            &target_evidence_path,
        )
        .expect("copy metadata");
        fs::copy(
            source_project.path().join(&evidence.screenshot_ref),
            target_project.path().join(&evidence.screenshot_ref),
        )
        .expect("copy screenshot");

        let target_scope = ProjectWriteScope::open(target_project.path()).expect("target scope");
        let mut target_ticket = ticket;
        target_ticket.project_identity = "project.preview.other".to_string();
        target_ticket.expected_project_digest = digest('b');
        assert_eq!(
            ProjectPreviewEvidence::validate_frame(&target_scope, &target_ticket, &evidence_ref,)
                .expect_err("cross-project evidence")
                .code,
            "project_preview_evidence.project_identity_mismatch"
        );
    }

    #[test]
    fn preview_frame_evidence_rejects_invalid_dimensions_length_and_operation_id() {
        let project = TempProject::new("invalid-capture");
        let scope = ProjectWriteScope::open(project.path()).expect("write scope");
        assert_eq!(
            ProjectPreviewEvidence::persist_frame(&scope, &ticket(), capture(0, 2, Vec::new()))
                .expect_err("zero width")
                .code,
            "project_preview_evidence.invalid_dimensions"
        );
        assert_eq!(
            ProjectPreviewEvidence::persist_frame(&scope, &ticket(), capture(2, 2, vec![0; 15]))
                .expect_err("invalid rgba length")
                .code,
            "project_preview_evidence.invalid_pixel_length"
        );
        let mut escaped = ticket();
        escaped.operation_id = "../escape".to_string();
        assert_eq!(
            ProjectPreviewEvidence::persist_frame(&scope, &escaped, capture(1, 1, vec![0; 4]))
                .expect_err("escaped operation id")
                .code,
            "project_preview_evidence.operation_id_invalid"
        );
    }
}
