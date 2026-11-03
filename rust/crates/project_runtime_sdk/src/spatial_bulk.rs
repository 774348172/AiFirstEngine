//! Bounded session wire. Explicit bytes, no Rust layout or borrowed memory crosses the DLL.
use crate::*;

pub const MAX_SPATIAL_ITEMS: usize = 16_384;
pub const MAX_SESSION_BYTES: usize = 8 * 1024 * 1024;
const MAGIC: &[u8; 8] = b"AIFESB01";
const RECORD_BYTES: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialEntityKey {
    pub world_epoch: u64,
    pub index: u32,
    pub generation: u32,
}
impl SpatialEntityKey {
    pub fn is_valid(self) -> bool {
        self.world_epoch != 0
    }
}

pub fn valid_transform(t: &ProjectRuntimeTransform3D) -> bool {
    t.translation
        .iter()
        .chain(&t.rotation_xyzw)
        .chain(&t.scale)
        .all(|v| v.is_finite())
        && t.scale.iter().all(|v| *v != 0.)
        && t.rotation_xyzw
            .iter()
            .map(|v| f64::from(*v).powi(2))
            .sum::<f64>()
            > 0.
}

pub fn encode_session(
    output: ProjectRuntimeSessionOutput,
) -> Result<Vec<u8>, ProjectRuntimeAbiStatus> {
    if output.mutations.len() > MAX_SPATIAL_ITEMS {
        return Err(ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
    }
    if output.mutations.iter().any(is_attached_mutation) {
        return encode_attached_session(output);
    }
    encode_legacy_session(output)
}

fn encode_legacy_session(
    mut output: ProjectRuntimeSessionOutput,
) -> Result<Vec<u8>, ProjectRuntimeAbiStatus> {
    let mut records = Vec::new();
    let mut cold = Vec::new();
    for (ordinal, mutation) in std::mem::take(&mut output.mutations)
        .into_iter()
        .enumerate()
    {
        match mutation {
            ProjectRuntimeDeferredMutation::SpatialLocalTransform { entity, transform } => {
                if ordinal >= MAX_SPATIAL_ITEMS
                    || !entity.is_valid()
                    || !valid_transform(&transform)
                {
                    return Err(ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
                }
                records.extend_from_slice(&(ordinal as u32).to_le_bytes());
                records.extend_from_slice(&entity.world_epoch.to_le_bytes());
                records.extend_from_slice(&entity.index.to_le_bytes());
                records.extend_from_slice(&entity.generation.to_le_bytes());
                for v in transform
                    .translation
                    .iter()
                    .chain(&transform.rotation_xyzw)
                    .chain(&transform.scale)
                {
                    records.extend_from_slice(&v.to_le_bytes());
                }
            }
            other => cold.push(other),
        }
    }
    output.mutations = cold;
    let metadata = serde_json::to_vec(&output).map_err(|_| ProjectRuntimeAbiStatus::FAILED)?;
    if records.is_empty() {
        return (metadata.len() <= MAX_SESSION_BYTES)
            .then_some(metadata)
            .ok_or(ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
    }
    let count = records.len() / RECORD_BYTES;
    if count + output.mutations.len() > MAX_SPATIAL_ITEMS
        || metadata.len() + records.len() + 16 > MAX_SESSION_BYTES
    {
        return Err(ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
    }
    let mut bytes = Vec::with_capacity(16 + metadata.len() + records.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(count as u32).to_le_bytes());
    bytes.extend_from_slice(&metadata);
    bytes.extend_from_slice(&records);
    Ok(bytes)
}

pub fn decode_session(
    bytes: &[u8],
) -> Result<ProjectRuntimeSessionOutput, ProjectRuntimeAbiStatus> {
    if bytes.starts_with(b"AIFESB02") {
        return decode_attached_session(bytes);
    }
    let output = decode_legacy_session(bytes)?;
    if output.mutations.iter().any(is_attached_mutation) {
        return Err(ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
    }
    Ok(output)
}

fn decode_legacy_session(
    bytes: &[u8],
) -> Result<ProjectRuntimeSessionOutput, ProjectRuntimeAbiStatus> {
    let invalid = ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    if bytes.len() > MAX_SESSION_BYTES {
        return Err(invalid);
    }
    if !bytes.starts_with(MAGIC) {
        let output: ProjectRuntimeSessionOutput =
            serde_json::from_slice(bytes).map_err(|_| invalid)?;
        if output.mutations.len() > MAX_SPATIAL_ITEMS
            || output.mutations.iter().any(|m| {
                matches!(
                    m,
                    ProjectRuntimeDeferredMutation::SpatialLocalTransform { .. }
                )
            })
        {
            return Err(invalid);
        }
        return Ok(output);
    }
    if bytes.len() < 16 {
        return Err(invalid);
    }
    let u32_at =
        |i| u32::from_le_bytes(bytes[i..i + 4].try_into().expect("checked header")) as usize;
    let meta_len = u32_at(8);
    let count = u32_at(12);
    if count == 0 || count > MAX_SPATIAL_ITEMS || meta_len > bytes.len() - 16 {
        return Err(invalid);
    }
    let start = 16 + meta_len;
    if bytes.len() - start != count * RECORD_BYTES {
        return Err(invalid);
    }
    let mut output: ProjectRuntimeSessionOutput =
        serde_json::from_slice(&bytes[16..start]).map_err(|_| invalid)?;
    if output.mutations.iter().any(|m| {
        matches!(
            m,
            ProjectRuntimeDeferredMutation::SpatialLocalTransform { .. }
        )
    }) {
        return Err(invalid);
    }
    let total = count + output.mutations.len();
    if total > MAX_SPATIAL_ITEMS {
        return Err(invalid);
    }
    let mut cold = std::mem::take(&mut output.mutations).into_iter();
    let mut merged = Vec::with_capacity(total);
    for record in bytes[start..].chunks_exact(RECORD_BYTES) {
        let word = |i| u32::from_le_bytes(record[i..i + 4].try_into().expect("fixed record"));
        let ordinal = word(0) as usize;
        if ordinal < merged.len() || ordinal >= total {
            return Err(invalid);
        }
        while merged.len() < ordinal {
            merged.push(cold.next().ok_or(invalid)?);
        }
        let entity = SpatialEntityKey {
            world_epoch: u64::from_le_bytes(record[4..12].try_into().expect("fixed record")),
            index: word(12),
            generation: word(16),
        };
        let values: [f32; 10] = std::array::from_fn(|i| f32::from_bits(word(20 + i * 4)));
        let transform = ProjectRuntimeTransform3D {
            translation: values[..3].try_into().expect("three"),
            rotation_xyzw: values[3..7].try_into().expect("four"),
            scale: values[7..].try_into().expect("three"),
        };
        if !entity.is_valid() || !valid_transform(&transform) {
            return Err(invalid);
        }
        merged.push(ProjectRuntimeDeferredMutation::SpatialLocalTransform { entity, transform });
    }
    merged.extend(cold);
    if merged.len() != total {
        return Err(invalid);
    }
    output.mutations = merged;
    Ok(output)
}

fn is_attached_mutation(m: &ProjectRuntimeDeferredMutation) -> bool {
    matches!(
        m,
        ProjectRuntimeDeferredMutation::GeneratedContent { .. }
            | ProjectRuntimeDeferredMutation::GeneratedContentCancel { .. }
            | ProjectRuntimeDeferredMutation::ProjectStorage { .. }
            | ProjectRuntimeDeferredMutation::ProjectStorageAbandon { .. }
    )
}
fn payloads_mut(m: &mut ProjectRuntimeDeferredMutation, f: impl FnMut(&mut Vec<u8>)) {
    match m {
        ProjectRuntimeDeferredMutation::GeneratedContent { request } => request.payloads_mut(f),
        ProjectRuntimeDeferredMutation::ProjectStorage { request } => request.payloads_mut(f),
        _ => (),
    }
}
fn validate_attached(m: &ProjectRuntimeDeferredMutation) -> Result<(), ProjectRuntimeAbiStatus> {
    let invalid = ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    match m {
        ProjectRuntimeDeferredMutation::GeneratedContent { request } => {
            request.validate().map_err(|_| invalid)
        }
        ProjectRuntimeDeferredMutation::GeneratedContentCancel { ticket }
        | ProjectRuntimeDeferredMutation::ProjectStorageAbandon { ticket } => {
            if ticket.request_id == 0 || ticket.session_generation == 0 {
                Err(invalid)
            } else {
                Ok(())
            }
        }
        ProjectRuntimeDeferredMutation::ProjectStorage { request } => {
            request.validate().map_err(|_| invalid)
        }
        _ => Ok(()),
    }
}

// v2 wraps the unchanged v1 JSON/60-byte transform wire. Each canonical 16-byte attachment
// record is (mutation ordinal, payload slot, blob-relative offset, length), all LE u32.
// No borrowed slice survives this decoder; the returned output owns every attachment.
fn encode_attached_session(
    mut output: ProjectRuntimeSessionOutput,
) -> Result<Vec<u8>, ProjectRuntimeAbiStatus> {
    let invalid = ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    let mut payloads = Vec::new();
    let mut payload_size = 0usize;
    for (ordinal, m) in output.mutations.iter_mut().enumerate() {
        validate_attached(m)?;
        let mut slot = 0u32;
        payloads_mut(m, |bytes| {
            payload_size = payload_size.saturating_add(bytes.len());
            payloads.push((ordinal as u32, slot, std::mem::take(bytes)));
            slot += 1;
        });
        if payload_size > MAX_SESSION_BYTES {
            return Err(invalid);
        }
    }
    let inner = encode_legacy_session(output)?;
    let table_len = payloads.len().checked_mul(16).ok_or(invalid)?;
    let total = 16usize
        .checked_add(inner.len())
        .and_then(|n| n.checked_add(table_len))
        .and_then(|n| n.checked_add(payload_size))
        .ok_or(invalid)?;
    if total > MAX_SESSION_BYTES {
        return Err(invalid);
    }
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(b"AIFESB02");
    bytes.extend_from_slice(&(inner.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(payloads.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&inner);
    let mut offset = 0u32;
    for (ordinal, slot, payload) in &payloads {
        for word in [*ordinal, *slot, offset, payload.len() as u32] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        offset += payload.len() as u32;
    }
    for (_, _, payload) in payloads {
        bytes.extend_from_slice(&payload);
    }
    Ok(bytes)
}
fn decode_attached_session(
    bytes: &[u8],
) -> Result<ProjectRuntimeSessionOutput, ProjectRuntimeAbiStatus> {
    let invalid = ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    if bytes.len() < 16 || bytes.len() > MAX_SESSION_BYTES {
        return Err(invalid);
    }
    let word = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
    let inner_len = word(8);
    let count = word(12);
    if count > MAX_SPATIAL_ITEMS * 4 {
        return Err(invalid);
    }
    let table_start = 16usize.checked_add(inner_len).ok_or(invalid)?;
    let blob_start = table_start
        .checked_add(count.checked_mul(16).ok_or(invalid)?)
        .ok_or(invalid)?;
    if blob_start > bytes.len() || bytes[16..table_start].starts_with(b"AIFESB02") {
        return Err(invalid);
    }
    let table = &bytes[table_start..blob_start];
    let blob = &bytes[blob_start..];
    // Validate all offsets before metadata decoding or attachment allocation.
    let mut expected_offset = 0usize;
    let mut previous = None;
    for record in table.chunks_exact(16) {
        let w = |i| u32::from_le_bytes(record[i..i + 4].try_into().unwrap()) as usize;
        let key = (w(0), w(4));
        let offset = w(8);
        let len = w(12);
        if key.0 >= MAX_SPATIAL_ITEMS
            || previous.is_some_and(|p| p >= key)
            || offset != expected_offset
        {
            return Err(invalid);
        }
        expected_offset = offset
            .checked_add(len)
            .filter(|n| *n <= blob.len())
            .ok_or(invalid)?;
        previous = Some(key);
    }
    if expected_offset != blob.len() {
        return Err(invalid);
    }
    let mut output = decode_legacy_session(&bytes[16..table_start])?;
    if !output.mutations.iter().any(is_attached_mutation) {
        return Err(invalid);
    }
    // Metadata may not smuggle payload bytes; payload fields are serde(skip).
    let mut records = table.chunks_exact(16);
    let mut failed = false;
    for (ordinal, m) in output.mutations.iter_mut().enumerate() {
        let mut slot = 0usize;
        payloads_mut(m, |payload| {
            let Some(record) = records.next() else {
                failed = true;
                return;
            };
            let w = |i| u32::from_le_bytes(record[i..i + 4].try_into().unwrap()) as usize;
            if w(0) != ordinal || w(4) != slot {
                failed = true;
                return;
            }
            *payload = blob[w(8)..w(8) + w(12)].to_vec();
            slot += 1;
        });
        if failed {
            return Err(invalid);
        }
        validate_attached(m)?;
    }
    if records.next().is_some() {
        return Err(invalid);
    }
    Ok(output)
}

pub fn call_session_once<Request: Serialize>(
    call: ProjectRuntimeModuleCall,
    module: ProjectRuntimeOpaqueHandle,
    session: ProjectRuntimeOpaqueHandle,
    context: Option<&ProjectRuntimeCallContext>,
    request: &Request,
    buffer: &mut [u8],
) -> Result<ProjectRuntimeSessionOutput, ProjectRuntimeSdkError> {
    call_once_with_buffer_decoder(
        call,
        module,
        session,
        context,
        request,
        buffer,
        decode_session,
    )
}

/// # Safety
/// The caller owns an aligned exclusive descriptor and its `capacity` writable backing bytes.
/// Both stay live for this call and cannot alias memory accessed by the callback.
pub unsafe fn session_boundary(
    output: *mut ProjectRuntimeByteBuffer,
    capacity_floor: usize,
    callback: impl FnOnce() -> Result<ProjectRuntimeSessionOutput, ProjectRuntimeAbiStatus>,
) -> ProjectRuntimeAbiStatus {
    if output.is_null() {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    // SAFETY: upheld by caller; no callback runs while this temporary borrow exists.
    let buffer = unsafe { &mut *output };
    if buffer.capacity < capacity_floor as u64 {
        buffer.written = capacity_floor as u64;
        return ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL;
    }
    if buffer.capacity > 0 && buffer.data.is_null() {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    match catch_unwind(AssertUnwindSafe(|| callback().and_then(encode_session))) {
        Ok(Ok(bytes)) => write_output(output, &bytes),
        Ok(Err(status)) => status,
        Err(_) => ProjectRuntimeAbiStatus::PANICKED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_content_376_release_has_explicit_versioned_session_wire() {
        let mut value = serde_json::to_value(ProjectRuntimeSessionOutput::no_op()).unwrap();
        value["mutations"] = serde_json::json!([{
            "kind":"generated_content",
            "request": {
                "requestId":1,
                "target":{"entityId":"surface","generation":7},
                "expectedAppliedRevision":0,
                "operation":{"operation":"release"}
            }
        }]);
        let output: ProjectRuntimeSessionOutput = serde_json::from_value(value)
            .expect("376 release must be a supported typed session mutation");
        let bytes = encode_session(output.clone()).unwrap();
        assert_eq!(&bytes[..8], b"AIFESB02");
        assert_eq!(decode_session(&bytes).unwrap(), output);
    }
    fn hot(i: u32) -> ProjectRuntimeDeferredMutation {
        ProjectRuntimeDeferredMutation::SpatialLocalTransform {
            entity: SpatialEntityKey {
                world_epoch: 42,
                index: i,
                generation: 7,
            },
            transform: ProjectRuntimeTransform3D::identity(),
        }
    }
    #[test]
    fn spatial_bulk_preserves_order_and_scales_exact_record_work() {
        for n in [1, 100, 1000, 10000] {
            let mut output = ProjectRuntimeSessionOutput::no_op();
            output.mutations = (0..n).map(hot).collect();
            output.mutations.insert(
                0,
                ProjectRuntimeDeferredMutation::DespawnEntity {
                    entity_id: "cold".into(),
                },
            );
            let bytes = encode_session(output.clone()).unwrap();
            assert_eq!(decode_session(&bytes).unwrap(), output);
            let meta_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
            assert_eq!(bytes.len(), 16 + meta_len + n as usize * RECORD_BYTES);
        }
    }
    #[test]
    fn spatial_bulk_rejects_corrupt_lengths_order_tail_identity_and_values() {
        let mut output = ProjectRuntimeSessionOutput::no_op();
        output.mutations = vec![hot(1), hot(2)];
        let bytes = encode_session(output).unwrap();
        let offset = 16 + u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        for (pos, replacement) in [
            (12, u32::MAX),
            (offset + 60, 0),
            (offset, 2),
            (offset + 4, 0),
            (offset + 20, f32::NAN.to_bits()),
        ] {
            let mut bad = bytes.clone();
            bad[pos..pos + 4].copy_from_slice(&replacement.to_le_bytes());
            assert!(decode_session(&bad).is_err(), "offset {pos}");
        }
        for length in [0, 7, 15, bytes.len() - 1] {
            assert!(decode_session(&bytes[..length]).is_err());
        }
        let mut bad = bytes.clone();
        bad.push(0);
        assert!(decode_session(&bad).is_err());
        let mut bad = bytes;
        bad[7] = b'2';
        assert!(decode_session(&bad).is_err());
    }
    #[test]
    fn spatial_bulk_stateful_callback_is_never_replayed() {
        let calls = std::cell::Cell::new(0);
        let mut bytes = vec![0; 256];
        let mut buffer = ProjectRuntimeByteBuffer::from_slice(&mut bytes);
        // SAFETY: live exclusive descriptor and backing storage.
        let small = unsafe {
            session_boundary(&mut buffer, 512, || {
                calls.set(calls.get() + 1);
                Ok(ProjectRuntimeSessionOutput::no_op())
            })
        };
        assert_eq!(small, ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL);
        assert_eq!(calls.get(), 0);
        // SAFETY: as above, now the bounded capacity is sufficient.
        let ok = unsafe {
            session_boundary(&mut buffer, 256, || {
                calls.set(calls.get() + 1);
                Ok(ProjectRuntimeSessionOutput::no_op())
            })
        };
        assert!(ok.is_ok());
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn spatial_bulk_does_not_accept_hot_json_or_excess_count_and_keeps_error_status() {
        let mut output = ProjectRuntimeSessionOutput::no_op();
        output.mutations = vec![hot(0)];
        let json = serde_json::to_vec(&output).unwrap();
        assert!(
            decode_session(&json).is_err(),
            "hot records require the versioned envelope"
        );
        for status in [
            ProjectRuntimeStatus::NoOp,
            ProjectRuntimeStatus::Rejected,
            ProjectRuntimeStatus::Faulted,
        ] {
            output.status = status;
            let restored = decode_session(&encode_session(output.clone()).unwrap()).unwrap();
            assert_eq!(
                restored, output,
                "transport cannot promote a rejected result to Applied"
            );
        }
        output.mutations = (0..MAX_SPATIAL_ITEMS as u32 + 1).map(hot).collect();
        assert!(encode_session(output).is_err());
    }
}
