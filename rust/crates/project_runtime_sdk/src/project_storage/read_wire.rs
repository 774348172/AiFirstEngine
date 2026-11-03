//! Read-result bytes stay out of JSON. The owner claims a result only after an OK copy.
use super::*;
use project_runtime_abi::{ProjectRuntimeAbiStatus, ProjectRuntimeByteBuffer};

const MAGIC: &[u8; 8] = b"AIFEPB01";
const HEADER_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectBytes {
    pub ticket: ContentTicket,
    pub bytes: Vec<u8>,
}
pub type StorageReadReply = Result<ProjectBytes, ContentError>;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadMetadata {
    ticket: ContentTicket,
    byte_count: u64,
}

/// Copy a whole result or report its required envelope length without copying any bytes.
/// The storage owner must keep its payload and reservation on every status except OK.
/// After OK for a successful byte result, it can mark that result claimed and release its backing.
/// Copying a semantic error never claims a byte result. No project callback runs here.
///
/// # Safety
/// `output` points to a live, aligned, exclusively borrowed descriptor, whose non-null data
/// points to `capacity` writable bytes. The output cannot alias the result or metadata.
pub unsafe fn write_project_bytes_reply(
    output: *mut ProjectRuntimeByteBuffer,
    result: Result<(ContentTicket, &[u8]), &ContentError>,
) -> ProjectRuntimeAbiStatus {
    let invalid = ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    if output.is_null() {
        return invalid;
    }
    let (metadata, payload) = match result {
        Ok((ticket, bytes)) => {
            if (StorageQuery::Ticket { ticket }).validate().is_err() {
                return invalid;
            }
            (
                Ok(ReadMetadata {
                    ticket,
                    byte_count: bytes.len() as u64,
                }),
                bytes,
            )
        }
        Err(error) => (Err(error), &[][..]),
    };
    if payload.len() > crate::spatial_bulk::MAX_SESSION_BYTES {
        return invalid;
    }
    let metadata = match serde_json::to_vec(&metadata) {
        Ok(bytes) => bytes,
        Err(_) => return ProjectRuntimeAbiStatus::FAILED,
    };
    let Some(total) = HEADER_BYTES
        .checked_add(metadata.len())
        .and_then(|n| n.checked_add(payload.len()))
        .filter(|n| *n <= crate::spatial_bulk::MAX_SESSION_BYTES)
    else {
        return invalid;
    };
    // SAFETY: the ABI caller guarantees exclusive descriptor and backing storage.
    let output = unsafe { &mut *output };
    output.written = total as u64;
    if output.capacity < total as u64 {
        return ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL;
    }
    if output.data.is_null() {
        return invalid;
    }
    let mut header = [0; HEADER_BYTES];
    header[..8].copy_from_slice(MAGIC);
    header[8..12].copy_from_slice(&(metadata.len() as u32).to_le_bytes());
    header[12..].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    let mut offset = 0;
    for bytes in [header.as_slice(), metadata.as_slice(), payload] {
        if !bytes.is_empty() {
            // SAFETY: the complete envelope fits the checked caller buffer and cannot alias input.
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), output.data.add(offset), bytes.len())
            };
        }
        offset += bytes.len();
    }
    ProjectRuntimeAbiStatus::OK
}

pub fn decode_project_bytes_reply(
    wire: &[u8],
) -> Result<StorageReadReply, ProjectRuntimeAbiStatus> {
    let invalid = ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    if wire.len() < HEADER_BYTES
        || wire.len() > crate::spatial_bulk::MAX_SESSION_BYTES
        || !wire.starts_with(MAGIC)
    {
        return Err(invalid);
    }
    let metadata_len = u32::from_le_bytes(wire[8..12].try_into().unwrap()) as usize;
    let payload_len = u32::from_le_bytes(wire[12..16].try_into().unwrap()) as usize;
    let payload_start = HEADER_BYTES.checked_add(metadata_len).ok_or(invalid)?;
    if payload_start.checked_add(payload_len) != Some(wire.len()) {
        return Err(invalid);
    }
    let metadata: Result<ReadMetadata, ContentError> =
        serde_json::from_slice(&wire[HEADER_BYTES..payload_start]).map_err(|_| invalid)?;
    match metadata {
        Ok(metadata) => {
            StorageQuery::Ticket {
                ticket: metadata.ticket,
            }
            .validate()
            .map_err(|_| invalid)?;
            if metadata.byte_count != payload_len as u64 {
                return Err(invalid);
            }
            Ok(Ok(ProjectBytes {
                ticket: metadata.ticket,
                bytes: wire[payload_start..].to_vec(),
            }))
        }
        Err(error) if payload_len == 0 => Ok(Err(error)),
        Err(_) => Err(invalid),
    }
}

/// Uses the existing world-query callback and caller-owned buffers. A sizing retry never
/// consumes the result; the host marks it claimed only after the complete binary copy succeeds.
pub fn call_take_project_bytes(
    call: project_runtime_abi::ProjectRuntimeHostCall,
    context: project_runtime_abi::ProjectRuntimeOpaqueHandle,
    ticket: ContentTicket,
) -> Result<StorageReadReply, crate::ProjectRuntimeSdkError> {
    let failed = |message: &str| crate::ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::FAILED,
        message: message.into(),
    };
    StorageQuery::Ticket { ticket }
        .validate()
        .map_err(|error| crate::ProjectRuntimeSdkError {
            status: ProjectRuntimeAbiStatus::INVALID_ARGUMENT,
            message: error.to_string(),
        })?;
    let request = serde_json::to_vec(&StorageTakeRequest {
        project_storage_take: ticket,
    })
    .map_err(|_| failed("Cannot encode project byte result request"))?;
    let mut bytes = vec![0; 4096];
    for _ in 0..2 {
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut bytes);
        // SAFETY: all pointers borrow caller-owned storage for this callback only.
        let status = unsafe {
            call(
                context,
                project_runtime_abi::ProjectRuntimeByteSlice::from_slice(&request),
                &mut output,
            )
        };
        let written = usize::try_from(output.written)
            .map_err(|_| failed("Invalid project byte result size"))?;
        if written > crate::spatial_bulk::MAX_SESSION_BYTES {
            return Err(failed("Project byte result exceeds the envelope limit"));
        }
        if status == ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL {
            if written <= bytes.len() {
                return Err(failed("Invalid project byte result sizing reply"));
            }
            bytes.resize(written, 0);
            continue;
        }
        if !status.is_ok() {
            return Err(crate::ProjectRuntimeSdkError {
                status,
                message: "Project byte result host call failed".into(),
            });
        }
        if written > bytes.len() {
            return Err(failed("Project byte result exceeds caller buffer"));
        }
        let reply = decode_project_bytes_reply(&bytes[..written])
            .map_err(|_| failed("Invalid project byte result envelope"))?;
        if reply.as_ref().is_ok_and(|result| result.ticket != ticket) {
            return Err(failed(
                "Project byte result ticket does not match the requested ticket",
            ));
        }
        return Ok(reply);
    }
    Err(failed("Project byte result size changed after retry"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial_bulk::MAX_SESSION_BYTES;
    fn ticket() -> ContentTicket {
        ContentTicket {
            session_generation: 4,
            request_id: 8,
        }
    }

    #[test]
    fn generated_content_376_storage_read_short_buffer_preserves_result_until_successful_copy() {
        let bytes = [1, 0, 255, 5];
        let mut retained = Some(bytes.to_vec());
        let mut small = [0xa5; 8];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut small);
        let status = unsafe {
            write_project_bytes_reply(&mut output, Ok((ticket(), retained.as_deref().unwrap())))
        };
        if status.is_ok() {
            retained = None;
        }
        assert_eq!(status, ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL);
        assert_eq!(small, [0xa5; 8]);
        assert_eq!(retained.as_deref(), Some(bytes.as_slice()));
        let needed = output.written as usize;
        let mut full = vec![0; needed];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut full);
        let status = unsafe {
            write_project_bytes_reply(&mut output, Ok((ticket(), retained.as_deref().unwrap())))
        };
        if status.is_ok() {
            retained = None;
        }
        assert_eq!(status, ProjectRuntimeAbiStatus::OK);
        assert!(retained.is_none());
        assert_eq!(
            decode_project_bytes_reply(&full).unwrap().unwrap(),
            ProjectBytes {
                ticket: ticket(),
                bytes: bytes.to_vec()
            }
        );
        assert!(serde_json::from_slice::<serde_json::Value>(&full).is_err());
    }

    #[test]
    fn generated_content_376_storage_read_empty_value_and_not_found_are_distinct() {
        let mut storage = vec![0; 4096];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut storage);
        assert_eq!(
            unsafe { write_project_bytes_reply(&mut output, Ok((ticket(), &[]))) },
            ProjectRuntimeAbiStatus::OK
        );
        let empty = decode_project_bytes_reply(&storage[..output.written as usize])
            .unwrap()
            .unwrap();
        assert_eq!(empty.ticket, ticket());
        assert!(empty.bytes.is_empty());
        let mut error = ContentError::invalid("storage.key");
        error.code = crate::generated_content::ContentErrorCode::NotFound;
        error.request_id = ticket().request_id;
        assert_eq!(
            unsafe { write_project_bytes_reply(&mut output, Err(&error)) },
            ProjectRuntimeAbiStatus::OK
        );
        assert_eq!(
            decode_project_bytes_reply(&storage[..output.written as usize]).unwrap(),
            Err(error)
        );
    }

    #[test]
    fn generated_content_376_storage_read_total_limit_truncation_and_owned_decode() {
        let mut probe = ProjectRuntimeByteBuffer::from_slice(&mut []);
        let payload = vec![7; MAX_SESSION_BYTES];
        assert_eq!(
            unsafe { write_project_bytes_reply(&mut probe, Ok((ticket(), &payload))) },
            ProjectRuntimeAbiStatus::INVALID_ARGUMENT
        );
        let payload = [3, 2, 1];
        let mut wire = vec![0; 4096];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut wire);
        assert_eq!(
            unsafe { write_project_bytes_reply(&mut output, Ok((ticket(), &payload))) },
            ProjectRuntimeAbiStatus::OK
        );
        wire.truncate(output.written as usize);
        for length in 0..wire.len() {
            assert!(
                decode_project_bytes_reply(&wire[..length]).is_err(),
                "length={length}"
            );
        }
        let result = decode_project_bytes_reply(&wire).unwrap().unwrap();
        let mut bad = wire.clone();
        bad[12..16].copy_from_slice(&4u32.to_le_bytes());
        assert!(decode_project_bytes_reply(&bad).is_err());
        wire.fill(0);
        assert_eq!(result.bytes, payload);
    }

    #[test]
    fn generated_content_376_storage_read_exact_envelope_limit_and_invalid_descriptor() {
        let metadata = serde_json::to_vec(&Result::<_, ContentError>::Ok(ReadMetadata {
            ticket: ticket(),
            byte_count: MAX_SESSION_BYTES as u64,
        }))
        .unwrap();
        let payload = vec![7; MAX_SESSION_BYTES - HEADER_BYTES - metadata.len()];
        let mut wire = vec![0; MAX_SESSION_BYTES];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut wire);
        assert_eq!(
            unsafe { write_project_bytes_reply(&mut output, Ok((ticket(), &payload))) },
            ProjectRuntimeAbiStatus::OK
        );
        assert_eq!(output.written as usize, MAX_SESSION_BYTES);
        assert_eq!(
            decode_project_bytes_reply(&wire).unwrap().unwrap().bytes,
            payload
        );
        assert_eq!(
            unsafe { write_project_bytes_reply(std::ptr::null_mut(), Ok((ticket(), &[]))) },
            ProjectRuntimeAbiStatus::INVALID_ARGUMENT
        );
        output.data = std::ptr::null_mut();
        assert_eq!(
            unsafe { write_project_bytes_reply(&mut output, Ok((ticket(), &[]))) },
            ProjectRuntimeAbiStatus::INVALID_ARGUMENT
        );
    }

    #[test]
    fn generated_content_376_storage_metadata_and_abandon_are_compact_and_versioned() {
        let query = StorageQueryRequest {
            project_storage: StorageQuery::Ticket { ticket: ticket() },
        };
        let json = serde_json::to_vec(&query).unwrap();
        assert!(json.len() < 256);
        assert_eq!(
            serde_json::from_slice::<StorageQueryRequest>(&json).unwrap(),
            query
        );
        assert!(serde_json::from_value::<StorageQueryRequest>(
            serde_json::json!({"projectStorage":{"query":"request","requestId":1},"all":[]})
        )
        .is_err());
        let mut output = crate::ProjectRuntimeSessionOutput::no_op();
        output.mutations.push(
            crate::ProjectRuntimeDeferredMutation::ProjectStorageAbandon { ticket: ticket() },
        );
        let wire = crate::spatial_bulk::encode_session(output.clone()).unwrap();
        assert!(wire.starts_with(b"AIFESB02"));
        assert_eq!(crate::spatial_bulk::decode_session(&wire).unwrap(), output);
        assert!(
            crate::spatial_bulk::decode_session(&serde_json::to_vec(&output).unwrap()).is_err()
        );
    }

    #[test]
    fn generated_content_376_storage_read_sdk_retries_then_owns_bytes_after_host_release() {
        struct Host {
            calls: usize,
            retained: Option<Vec<u8>>,
        }
        unsafe extern "C" fn callback(
            context: project_runtime_abi::ProjectRuntimeOpaqueHandle,
            input: project_runtime_abi::ProjectRuntimeByteSlice,
            output: *mut ProjectRuntimeByteBuffer,
        ) -> ProjectRuntimeAbiStatus {
            // SAFETY: this test keeps Host alive and exclusively borrowed for the synchronous call.
            let host = unsafe { &mut *(context.value as usize as *mut Host) };
            let request: StorageTakeRequest = match unsafe { crate::read_input(input) } {
                Ok(request) => request,
                Err(status) => return status,
            };
            host.calls += 1;
            let status = unsafe {
                write_project_bytes_reply(
                    output,
                    Ok((
                        request.project_storage_take,
                        host.retained.as_deref().unwrap(),
                    )),
                )
            };
            if status.is_ok() {
                host.retained = None;
            }
            status
        }
        let payload = vec![0xa5; 6000];
        let mut host = Host {
            calls: 0,
            retained: Some(payload.clone()),
        };
        let context = project_runtime_abi::ProjectRuntimeOpaqueHandle {
            value: &mut host as *mut Host as usize as u64,
            generation: 1,
        };
        let result = call_take_project_bytes(callback, context, ticket())
            .unwrap()
            .unwrap();
        assert_eq!(host.calls, 2);
        assert!(host.retained.is_none());
        assert_eq!(result.bytes, payload);
        assert_eq!(result.ticket, ticket());
    }

    #[test]
    fn generated_content_376_storage_read_sdk_rejects_a_different_result_ticket() {
        unsafe extern "C" fn callback(
            _: project_runtime_abi::ProjectRuntimeOpaqueHandle,
            _: project_runtime_abi::ProjectRuntimeByteSlice,
            output: *mut ProjectRuntimeByteBuffer,
        ) -> ProjectRuntimeAbiStatus {
            let different = ContentTicket {
                request_id: ticket().request_id + 1,
                ..ticket()
            };
            // SAFETY: the SDK supplies a live output descriptor for this callback.
            unsafe { write_project_bytes_reply(output, Ok((different, &[1, 2, 3]))) }
        }
        let error = call_take_project_bytes(
            callback,
            project_runtime_abi::ProjectRuntimeOpaqueHandle::NULL,
            ticket(),
        )
        .unwrap_err();
        assert!(error.message.contains("ticket"));
    }

    #[test]
    fn generated_content_376_storage_read_sdk_rejects_unbounded_host_sizing() {
        unsafe extern "C" fn callback(
            _: project_runtime_abi::ProjectRuntimeOpaqueHandle,
            _: project_runtime_abi::ProjectRuntimeByteSlice,
            output: *mut ProjectRuntimeByteBuffer,
        ) -> ProjectRuntimeAbiStatus {
            // SAFETY: call_take_project_bytes supplies a live exclusive descriptor.
            unsafe {
                (*output).written = MAX_SESSION_BYTES as u64 + 1;
            }
            ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL
        }
        let error = call_take_project_bytes(
            callback,
            project_runtime_abi::ProjectRuntimeOpaqueHandle::NULL,
            ticket(),
        )
        .unwrap_err();
        assert!(error.message.contains("envelope limit"));
        let error = call_take_project_bytes(
            callback,
            project_runtime_abi::ProjectRuntimeOpaqueHandle::NULL,
            ContentTicket {
                request_id: 0,
                ..ticket()
            },
        )
        .unwrap_err();
        assert_eq!(error.status, ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
    }
}
