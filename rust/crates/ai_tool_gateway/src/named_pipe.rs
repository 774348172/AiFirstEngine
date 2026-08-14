use crate::{
    ClientHello, ClientSessionBinding, CloseReceipt, GatewayControlError, GatewayOwnerThreadClient,
    GatewayReply, GatewayRequest,
};
use serde::{Deserialize, Serialize};

pub const GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION: &str = "ai-tool-gateway-pipe-wire.v2";
const MAX_PIPE_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "wireKind",
    content = "wire",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GatewayPipeWireRequest {
    Connect {
        schema_version: String,
        hello: ClientHello,
    },
    Dispatch {
        schema_version: String,
        request: GatewayRequest,
    },
    Close {
        schema_version: String,
        client_session_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "wireKind",
    content = "wire",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GatewayPipeWireReply {
    Connected(ClientSessionBinding),
    Dispatched(GatewayReply),
    Closed(CloseReceipt),
    Rejected(GatewayControlError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayNamedPipeSecurityEvidence {
    pub local_machine_only: bool,
    pub current_os_user_only: bool,
    pub protected_dacl: bool,
    pub tcp_listener_present: bool,
}

impl Default for GatewayNamedPipeSecurityEvidence {
    fn default() -> Self {
        Self {
            local_machine_only: true,
            current_os_user_only: true,
            protected_dacl: true,
            tcp_listener_present: false,
        }
    }
}

fn validate_wire_schema(schema_version: &str) -> Result<(), GatewayControlError> {
    if schema_version != GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION {
        return Err(pipe_error(
            "gateway.pipe.wire_schema_unsupported",
            "Named Pipe request uses an unsupported wire schema.",
            "Reconnect using ai-tool-gateway-pipe-wire.v1.",
        ));
    }
    Ok(())
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::ffi::c_void;
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::mem::size_of;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::ptr::null_mut;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc::Receiver, mpsc::RecvTimeoutError, Arc};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, LocalFree, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, HANDLE,
        INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        ReadFile, WriteFile, FILE_ATTRIBUTE_NORMAL, PIPE_ACCESS_DUPLEX,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, WaitNamedPipeW, PIPE_READMODE_BYTE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows_sys::Win32::System::IO::CancelSynchronousIo;

    pub struct GatewayNamedPipeServer {
        pipe_locator: String,
        shutdown: Arc<AtomicBool>,
        join: Option<JoinHandle<Result<(), GatewayControlError>>>,
    }

    impl GatewayNamedPipeServer {
        pub fn spawn(
            pipe_locator: impl Into<String>,
            owner_client: GatewayOwnerThreadClient,
        ) -> Result<Self, GatewayControlError> {
            let pipe_locator = pipe_locator.into();
            if !pipe_locator.starts_with(r"\\.\pipe\ai-first-game-engine\") {
                return Err(pipe_error(
                    "gateway.pipe.locator_invalid",
                    "Named Pipe locator is outside the Gateway namespace.",
                    "Use the unpredictable locator from the active discovery record.",
                ));
            }
            let handle = create_current_user_pipe(&pipe_locator)?;
            let shutdown = Arc::new(AtomicBool::new(false));
            let thread_shutdown = Arc::clone(&shutdown);
            let thread_locator = pipe_locator.clone();
            let join = thread::Builder::new()
                .name("ai-tool-gateway-pipe".to_string())
                .spawn(move || run_server(handle, &thread_locator, owner_client, thread_shutdown))
                .map_err(|error| {
                    pipe_error(
                        "gateway.pipe.thread_spawn_failed",
                        format!("Failed to start Named Pipe bridge: {error}"),
                        "Repair local process resources and restart the Editor.",
                    )
                })?;
            Ok(Self {
                pipe_locator,
                shutdown,
                join: Some(join),
            })
        }

        pub fn join(mut self) -> Result<(), GatewayControlError> {
            self.shutdown_and_join()
        }

        pub fn shutdown_and_join(&mut self) -> Result<(), GatewayControlError> {
            self.shutdown.store(true, Ordering::Release);
            let Some(join) = self.join.take() else {
                return Ok(());
            };
            let deadline = Instant::now() + Duration::from_secs(5);
            while !join.is_finished() {
                unsafe {
                    CancelSynchronousIo(join.as_raw_handle().cast());
                }
                // The server may cross its shutdown check before creating the next pipe
                // instance, so keep waking until the accept thread actually exits.
                let _ = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(FILE_ATTRIBUTE_NORMAL)
                    .open(&self.pipe_locator);
                if Instant::now() >= deadline {
                    self.join = Some(join);
                    return Err(pipe_error(
                        "gateway.pipe.shutdown_timeout",
                        "Named Pipe bridge did not stop within the bounded shutdown deadline.",
                        "Restart the Editor and inspect the owner-thread lifecycle evidence.",
                    ));
                }
                thread::sleep(Duration::from_millis(1));
            }
            join.join().map_err(|_| {
                pipe_error(
                    "gateway.pipe.thread_panicked",
                    "Named Pipe bridge thread panicked.",
                    "Restart the Editor and inspect the local crash evidence.",
                )
            })?
        }
    }

    impl Drop for GatewayNamedPipeServer {
        fn drop(&mut self) {
            let _ = self.shutdown_and_join();
        }
    }

    pub struct GatewayNamedPipeClient {
        file: std::fs::File,
    }

    impl GatewayNamedPipeClient {
        pub fn connect(pipe_locator: &str) -> Result<Self, GatewayControlError> {
            let deadline = Instant::now() + Duration::from_secs(5);
            let locator = wide(pipe_locator);
            loop {
                match OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(FILE_ATTRIBUTE_NORMAL)
                    .open(pipe_locator)
                {
                    Ok(file) => return Ok(Self { file }),
                    Err(error)
                        if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
                            && Instant::now() < deadline =>
                    unsafe {
                        WaitNamedPipeW(locator.as_ptr(), 50);
                    },
                    Err(error) => {
                        return Err(pipe_error(
                            "gateway.pipe.client_connect_failed",
                            format!("Failed to connect to the local Editor Named Pipe: {error}"),
                            "Rediscover the active Editor endpoint and retry as the same OS user.",
                        ));
                    }
                }
            }
        }

        pub fn exchange(
            &mut self,
            request: &GatewayPipeWireRequest,
        ) -> Result<GatewayPipeWireReply, GatewayControlError> {
            write_frame_io(&mut self.file, request)?;
            read_frame_io(&mut self.file)
        }
    }

    fn run_server(
        first_handle: OwnedHandle,
        pipe_locator: &str,
        owner_client: GatewayOwnerThreadClient,
        shutdown: Arc<AtomicBool>,
    ) -> Result<(), GatewayControlError> {
        let mut first_handle = Some(first_handle);
        let mut connection_workers = Vec::new();
        let mut connection_sequence = 1u64;
        let accept_result = loop {
            if shutdown.load(Ordering::Acquire) {
                break Ok(());
            }
            if let Err(error) = reap_finished_connection_workers(&mut connection_workers) {
                break Err(error);
            }
            let handle = match first_handle.take() {
                Some(handle) => handle,
                None => match create_current_user_pipe(pipe_locator) {
                    Ok(handle) => handle,
                    Err(error) => break Err(error),
                },
            };
            let connected = unsafe { ConnectNamedPipe(handle.0, null_mut()) };
            if connected == 0 && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
                if shutdown.load(Ordering::Acquire) {
                    break Ok(());
                }
                break Err(pipe_error(
                    "gateway.pipe.connect_failed",
                    format!("Named Pipe accept failed with Win32 error {}.", unsafe {
                        GetLastError()
                    }),
                    "Restart the Editor and reconnect using fresh discovery data.",
                ));
            }
            if shutdown.load(Ordering::Acquire) {
                break Ok(());
            }
            let worker_owner_client = owner_client.clone();
            let worker_shutdown = Arc::clone(&shutdown);
            let worker = match thread::Builder::new()
                .name(format!("ai-tool-gateway-pipe-{connection_sequence}"))
                .spawn(move || {
                    run_connection(&handle, &worker_owner_client, worker_shutdown.as_ref())
                }) {
                Ok(worker) => worker,
                Err(error) => {
                    break Err(pipe_error(
                        "gateway.pipe.connection_thread_spawn_failed",
                        format!("Failed to start Named Pipe connection worker: {error}"),
                        "Repair local process resources and restart the Editor.",
                    ));
                }
            };
            connection_sequence = connection_sequence.saturating_add(1);
            connection_workers.push(worker);
        };
        shutdown.store(true, Ordering::Release);
        let shutdown_result = shutdown_connection_workers(&mut connection_workers);
        accept_result.and(shutdown_result)
    }

    fn reap_finished_connection_workers(
        workers: &mut Vec<JoinHandle<Result<(), GatewayControlError>>>,
    ) -> Result<(), GatewayControlError> {
        let mut index = 0;
        while index < workers.len() {
            if workers[index].is_finished() {
                let worker = workers.swap_remove(index);
                match worker.join() {
                    Ok(_) => {}
                    Err(_) => {
                        return Err(pipe_error(
                            "gateway.pipe.connection_thread_panicked",
                            "Named Pipe connection worker panicked.",
                            "Restart the Editor and inspect the connection worker failure.",
                        ));
                    }
                }
            } else {
                index += 1;
            }
        }
        Ok(())
    }

    fn shutdown_connection_workers(
        workers: &mut Vec<JoinHandle<Result<(), GatewayControlError>>>,
    ) -> Result<(), GatewayControlError> {
        while workers.iter().any(|worker| !worker.is_finished()) {
            for worker in workers.iter().filter(|worker| !worker.is_finished()) {
                unsafe {
                    CancelSynchronousIo(worker.as_raw_handle().cast());
                }
            }
            thread::sleep(Duration::from_millis(1));
        }
        while let Some(worker) = workers.pop() {
            match worker.join() {
                Ok(_) => {}
                Err(_) => {
                    return Err(pipe_error(
                        "gateway.pipe.connection_thread_panicked",
                        "Named Pipe connection worker panicked.",
                        "Restart the Editor and inspect the connection worker failure.",
                    ));
                }
            }
        }
        Ok(())
    }

    fn run_connection(
        handle: &OwnedHandle,
        owner_client: &GatewayOwnerThreadClient,
        shutdown: &AtomicBool,
    ) -> Result<(), GatewayControlError> {
        let mut connected_session_id = None;
        let mut connect_attempted = false;
        loop {
            if shutdown.load(Ordering::Acquire) {
                queue_peer_session_close(owner_client, connected_session_id.take())?;
                return Ok(());
            }
            let request: GatewayPipeWireRequest = match read_frame_handle(handle.0) {
                Ok(request) => request,
                Err(_) if shutdown.load(Ordering::Acquire) => {
                    queue_peer_session_close(owner_client, connected_session_id.take())?;
                    return Ok(());
                }
                Err(error) if error.code == "gateway.pipe.peer_closed" => {
                    close_peer_session(owner_client, connected_session_id.take())?;
                    return Ok(());
                }
                Err(error) => {
                    close_peer_session(owner_client, connected_session_id.take())?;
                    return Err(error);
                }
            };
            let is_connect = matches!(&request, GatewayPipeWireRequest::Connect { .. });
            let reply = match validate_connection_request(
                connect_attempted,
                connected_session_id.as_deref(),
                &request,
            ) {
                Ok(()) => route_request(owner_client, request, shutdown),
                Err(error) => GatewayPipeWireReply::Rejected(error),
            };
            connect_attempted |= is_connect;
            if let GatewayPipeWireReply::Connected(binding) = &reply {
                connected_session_id = Some(binding.client_session_id.clone());
            }
            let close_after_reply = matches!(&reply, GatewayPipeWireReply::Closed(_));
            if let Err(error) = write_frame_handle(handle.0, &reply) {
                close_peer_session(owner_client, connected_session_id.take())?;
                return Err(error);
            }
            if shutdown.load(Ordering::Acquire) {
                queue_peer_session_close(owner_client, connected_session_id.take())?;
                return Ok(());
            }
            if close_after_reply {
                return Ok(());
            }
        }
    }

    fn validate_connection_request(
        connect_attempted: bool,
        connected_session_id: Option<&str>,
        request: &GatewayPipeWireRequest,
    ) -> Result<(), GatewayControlError> {
        let schema_version = match request {
            GatewayPipeWireRequest::Connect { schema_version, .. }
            | GatewayPipeWireRequest::Dispatch { schema_version, .. }
            | GatewayPipeWireRequest::Close { schema_version, .. } => schema_version,
        };
        validate_wire_schema(schema_version)?;

        match request {
            GatewayPipeWireRequest::Connect { .. } if connect_attempted => Err(pipe_error(
                "gateway.pipe.connect_already_attempted",
                "Named Pipe peers may submit exactly one Connect request.",
                "Open a fresh pipe connection before attempting another Gateway session.",
            )),
            GatewayPipeWireRequest::Connect { .. } => Ok(()),
            GatewayPipeWireRequest::Dispatch { .. } | GatewayPipeWireRequest::Close { .. }
                if connected_session_id.is_none() =>
            {
                Err(pipe_error(
                    "gateway.pipe.connect_required",
                    "Named Pipe peer must complete Connect before dispatching or closing a session.",
                    "Submit one valid ClientHello on this connection first.",
                ))
            }
            GatewayPipeWireRequest::Dispatch { request, .. }
                if connected_session_id != Some(request.client_session_id.as_str()) =>
            {
                Err(pipe_error(
                    "gateway.pipe.session_mismatch",
                    "Gateway dispatch session does not belong to this Named Pipe peer.",
                    "Use the exact ClientSessionBinding returned to this connection.",
                ))
            }
            GatewayPipeWireRequest::Close {
                client_session_id, ..
            } if connected_session_id != Some(client_session_id.as_str()) => Err(pipe_error(
                "gateway.pipe.session_mismatch",
                "Gateway close session does not belong to this Named Pipe peer.",
                "Close only the exact ClientSessionBinding returned to this connection.",
            )),
            GatewayPipeWireRequest::Dispatch { .. } | GatewayPipeWireRequest::Close { .. } => {
                Ok(())
            }
        }
    }

    fn close_peer_session(
        owner_client: &GatewayOwnerThreadClient,
        client_session_id: Option<String>,
    ) -> Result<(), GatewayControlError> {
        let Some(client_session_id) = client_session_id else {
            return Ok(());
        };
        let receiver = owner_client.submit_close(client_session_id)?;
        receiver
            .recv_timeout(Duration::from_secs(30))
            .map_err(map_wait_error)?;
        Ok(())
    }

    fn queue_peer_session_close(
        owner_client: &GatewayOwnerThreadClient,
        client_session_id: Option<String>,
    ) -> Result<(), GatewayControlError> {
        let Some(client_session_id) = client_session_id else {
            return Ok(());
        };
        let _receiver = owner_client.submit_close(client_session_id)?;
        Ok(())
    }

    fn route_request(
        owner_client: &GatewayOwnerThreadClient,
        request: GatewayPipeWireRequest,
        shutdown: &AtomicBool,
    ) -> GatewayPipeWireReply {
        let result = match request {
            GatewayPipeWireRequest::Connect {
                schema_version,
                hello,
            } => validate_wire_schema(&schema_version)
                .and_then(|_| {
                    let receiver = owner_client.submit_connect(hello)?;
                    wait_for_owner_reply(receiver, shutdown)?
                })
                .map(GatewayPipeWireReply::Connected),
            GatewayPipeWireRequest::Dispatch {
                schema_version,
                request,
            } => validate_wire_schema(&schema_version)
                .and_then(|_| {
                    let receiver = owner_client.submit_dispatch(request)?;
                    wait_for_owner_reply(receiver, shutdown)
                })
                .map(GatewayPipeWireReply::Dispatched),
            GatewayPipeWireRequest::Close {
                schema_version,
                client_session_id,
            } => validate_wire_schema(&schema_version)
                .and_then(|_| {
                    let receiver = owner_client.submit_close(client_session_id)?;
                    wait_for_owner_reply(receiver, shutdown)
                })
                .map(GatewayPipeWireReply::Closed),
        };
        result.unwrap_or_else(GatewayPipeWireReply::Rejected)
    }

    fn wait_for_owner_reply<T>(
        receiver: Receiver<T>,
        shutdown: &AtomicBool,
    ) -> Result<T, GatewayControlError> {
        loop {
            match receiver.recv_timeout(Duration::from_millis(10)) {
                Ok(reply) => return Ok(reply),
                Err(RecvTimeoutError::Timeout) if shutdown.load(Ordering::Acquire) => {
                    return Err(pipe_error(
                        "gateway.status.reconnect_required",
                        "The fixed Gateway endpoint was retired while this request awaited the Editor owner thread.",
                        "End the old Adapter process, rediscover the active project, and reconnect.",
                    ));
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(error @ RecvTimeoutError::Disconnected) => return Err(map_wait_error(error)),
            }
        }
    }

    fn map_wait_error(error: RecvTimeoutError) -> GatewayControlError {
        pipe_error(
            "gateway.pipe.owner_dispatch_timeout",
            format!("Editor owner-thread dispatch did not complete: {error}"),
            "Keep the Editor event loop running and retry with a fresh request id.",
        )
    }

    struct OwnedHandle(HANDLE);

    // The wrapper has unique ownership and only closes the handle on drop.
    unsafe impl Send for OwnedHandle {}

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if self.0 != INVALID_HANDLE_VALUE {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }

    struct LocalSecurityDescriptor(*mut c_void);

    impl Drop for LocalSecurityDescriptor {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }

    fn create_current_user_pipe(pipe_locator: &str) -> Result<OwnedHandle, GatewayControlError> {
        let sid = current_user_sid_string()?;
        let sddl = format!("D:P(A;;GA;;;{sid})");
        let sddl_wide = wide(&sddl);
        let mut descriptor = null_mut();
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl_wide.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        };
        if converted == 0 {
            return Err(last_win32_error(
                "gateway.pipe.security_descriptor_failed",
                "Failed to create a protected current-user Named Pipe DACL.",
            ));
        }
        let descriptor = LocalSecurityDescriptor(descriptor);
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let locator = wide(pipe_locator);
        let handle = unsafe {
            CreateNamedPipeW(
                locator.as_ptr(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                MAX_PIPE_FRAME_BYTES as u32,
                MAX_PIPE_FRAME_BYTES as u32,
                30_000,
                &attributes,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(last_win32_error(
                "gateway.pipe.create_failed",
                "Failed to create the local current-user Named Pipe.",
            ));
        }
        Ok(OwnedHandle(handle))
    }

    fn current_user_sid_string() -> Result<String, GatewayControlError> {
        let mut token: HANDLE = null_mut();
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(last_win32_error(
                "gateway.pipe.open_process_token_failed",
                "Failed to query the current OS user token.",
            ));
        }
        let token = OwnedHandle(token);
        let mut required = 0;
        unsafe {
            GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut required);
        }
        if required == 0 {
            return Err(last_win32_error(
                "gateway.pipe.token_user_size_failed",
                "Failed to size the current OS user identity.",
            ));
        }
        let mut buffer = vec![0u8; required as usize];
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                required,
                &mut required,
            )
        } == 0
        {
            return Err(last_win32_error(
                "gateway.pipe.token_user_read_failed",
                "Failed to read the current OS user identity.",
            ));
        }
        let token_user = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) };
        let mut sid_text = null_mut();
        if unsafe { ConvertSidToStringSidW(token_user.User.Sid, &mut sid_text) } == 0 {
            return Err(last_win32_error(
                "gateway.pipe.sid_conversion_failed",
                "Failed to convert the current OS user SID.",
            ));
        }
        let sid_allocation = LocalSecurityDescriptor(sid_text.cast());
        let mut length = 0;
        unsafe {
            while *sid_text.add(length) != 0 {
                length += 1;
            }
        }
        let sid = String::from_utf16(unsafe { std::slice::from_raw_parts(sid_text, length) })
            .map_err(|error| {
                pipe_error(
                    "gateway.pipe.sid_utf16_invalid",
                    format!("Current OS user SID is not valid UTF-16: {error}"),
                    "Restart the Editor under a valid interactive Windows user.",
                )
            })?;
        drop(sid_allocation);
        Ok(sid)
    }

    fn read_frame_handle<T: for<'de> Deserialize<'de>>(
        handle: HANDLE,
    ) -> Result<T, GatewayControlError> {
        let mut length = [0u8; 4];
        read_exact_handle(handle, &mut length)?;
        let length = u32::from_le_bytes(length) as usize;
        if length == 0 || length > MAX_PIPE_FRAME_BYTES {
            return Err(frame_size_error());
        }
        let mut payload = vec![0u8; length];
        read_exact_handle(handle, &mut payload)?;
        serde_json::from_slice(&payload).map_err(|error| {
            pipe_error(
                "gateway.pipe.frame_json_invalid",
                format!("Named Pipe frame is not strict valid JSON: {error}"),
                "Regenerate the frame from the Gateway wire schema.",
            )
        })
    }

    fn write_frame_handle<T: Serialize>(
        handle: HANDLE,
        value: &T,
    ) -> Result<(), GatewayControlError> {
        let payload = serde_json::to_vec(value).map_err(frame_serialize_error)?;
        if payload.is_empty() || payload.len() > MAX_PIPE_FRAME_BYTES {
            return Err(frame_size_error());
        }
        write_all_handle(handle, &(payload.len() as u32).to_le_bytes())?;
        write_all_handle(handle, &payload)
    }

    fn read_exact_handle(handle: HANDLE, bytes: &mut [u8]) -> Result<(), GatewayControlError> {
        let mut offset = 0;
        while offset < bytes.len() {
            let mut read = 0;
            let succeeded = unsafe {
                ReadFile(
                    handle,
                    bytes[offset..].as_mut_ptr(),
                    (bytes.len() - offset) as u32,
                    &mut read,
                    null_mut(),
                )
            };
            if succeeded == 0 || read == 0 {
                return Err(pipe_error(
                    "gateway.pipe.peer_closed",
                    "Named Pipe peer closed before a complete frame was read.",
                    "Reconnect and retry with a fresh request id.",
                ));
            }
            offset += read as usize;
        }
        Ok(())
    }

    fn write_all_handle(handle: HANDLE, bytes: &[u8]) -> Result<(), GatewayControlError> {
        let mut offset = 0;
        while offset < bytes.len() {
            let mut written = 0;
            let succeeded = unsafe {
                WriteFile(
                    handle,
                    bytes[offset..].as_ptr(),
                    (bytes.len() - offset) as u32,
                    &mut written,
                    null_mut(),
                )
            };
            if succeeded == 0 || written == 0 {
                return Err(last_win32_error(
                    "gateway.pipe.write_failed",
                    "Failed to write a complete Named Pipe frame.",
                ));
            }
            offset += written as usize;
        }
        Ok(())
    }

    fn write_frame_io<T: Serialize>(
        writer: &mut impl Write,
        value: &T,
    ) -> Result<(), GatewayControlError> {
        let payload = serde_json::to_vec(value).map_err(frame_serialize_error)?;
        if payload.is_empty() || payload.len() > MAX_PIPE_FRAME_BYTES {
            return Err(frame_size_error());
        }
        writer
            .write_all(&(payload.len() as u32).to_le_bytes())
            .and_then(|_| writer.write_all(&payload))
            .and_then(|_| writer.flush())
            .map_err(|error| {
                pipe_error(
                    "gateway.pipe.client_write_failed",
                    format!("Failed to write Named Pipe client frame: {error}"),
                    "Reconnect and retry with a fresh request id.",
                )
            })
    }

    fn read_frame_io<T: for<'de> Deserialize<'de>>(
        reader: &mut impl Read,
    ) -> Result<T, GatewayControlError> {
        let mut length = [0u8; 4];
        reader.read_exact(&mut length).map_err(|error| {
            pipe_error(
                "gateway.pipe.client_read_failed",
                format!("Failed to read Named Pipe reply length: {error}"),
                "Reconnect and retry with a fresh request id.",
            )
        })?;
        let length = u32::from_le_bytes(length) as usize;
        if length == 0 || length > MAX_PIPE_FRAME_BYTES {
            return Err(frame_size_error());
        }
        let mut payload = vec![0u8; length];
        reader.read_exact(&mut payload).map_err(|error| {
            pipe_error(
                "gateway.pipe.client_read_failed",
                format!("Failed to read complete Named Pipe reply: {error}"),
                "Reconnect and retry with a fresh request id.",
            )
        })?;
        serde_json::from_slice(&payload).map_err(|error| {
            pipe_error(
                "gateway.pipe.reply_json_invalid",
                format!("Named Pipe reply is not valid Gateway JSON: {error}"),
                "Discard the endpoint and rediscover the active Editor.",
            )
        })
    }

    fn frame_serialize_error(error: serde_json::Error) -> GatewayControlError {
        pipe_error(
            "gateway.pipe.frame_serialize_failed",
            format!("Failed to serialize Named Pipe frame: {error}"),
            "Use only strict Gateway wire types.",
        )
    }

    fn frame_size_error() -> GatewayControlError {
        pipe_error(
            "gateway.pipe.frame_size_invalid",
            "Named Pipe frame is empty or exceeds the one MiB transport limit.",
            "Send bounded requests and use evidence references for large data.",
        )
    }

    fn last_win32_error(code: &str, message: &str) -> GatewayControlError {
        pipe_error(
            code,
            format!("{message} Win32 error {}.", unsafe { GetLastError() }),
            "Restart the Editor under the same interactive OS user and retry.",
        )
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(windows)]
pub use windows_impl::{GatewayNamedPipeClient, GatewayNamedPipeServer};

#[cfg(not(windows))]
pub struct GatewayNamedPipeServer;

#[cfg(not(windows))]
impl GatewayNamedPipeServer {
    pub fn spawn(
        _pipe_locator: impl Into<String>,
        _owner_client: GatewayOwnerThreadClient,
    ) -> Result<Self, GatewayControlError> {
        Err(pipe_error(
            "gateway.pipe.windows_required",
            "Windows Named Pipe transport is unavailable on this platform.",
            "Use the in-memory Test Adapter or run the Windows Editor.",
        ))
    }

    pub fn join(self) -> Result<(), GatewayControlError> {
        Ok(())
    }

    pub fn shutdown_and_join(&mut self) -> Result<(), GatewayControlError> {
        Ok(())
    }
}

fn pipe_error(
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> GatewayControlError {
    GatewayControlError {
        code: code.into(),
        message: message.into(),
        next_action: next_action.into(),
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::{
        gateway_owner_thread_channel, ClientKind, GatewayCore, GatewayDiscoveryPublication,
        GatewayDiscoveryRecord, GatewayOwnerThreadDispatcher, GatewayReplyPayload,
        GatewayRequestPayload, GATEWAY_CLIENT_HELLO_SCHEMA_VERSION, GATEWAY_PROTOCOL_VERSION,
        GATEWAY_REQUEST_SCHEMA_VERSION,
    };
    use editor_core::{
        command_for_test, AiToolCatalogRequest, CommandStatus, EditorSession,
        AI_TOOL_CATALOG_SCHEMA_VERSION,
    };
    use editor_ui_model::UiCommandPayload;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn named_pipe_server_shutdown_is_bounded_across_accept_rearm() {
        for sequence in 0..64 {
            let discovery = GatewayDiscoveryRecord::new(format!("shutdown-race-{sequence}"));
            let (owner_client, _dispatcher) = gateway_owner_thread_channel();
            let mut server =
                GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client).unwrap();
            server.shutdown_and_join().unwrap();
        }
    }

    #[test]
    fn named_pipe_server_shutdown_cancels_active_connection_workers() {
        let (mut session, root, discovery, publication, owner_client, mut dispatcher, mut core) =
            pipe_fixture("active-worker-shutdown");
        let mut server =
            GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client).unwrap();
        let discovery_path = publication.path().to_path_buf();
        let (ready_sender, ready_receiver) = mpsc::channel();
        let mut releases = Vec::new();
        let mut clients = Vec::new();
        for sequence in 1..=2 {
            let client_discovery_path = discovery_path.clone();
            let client_ready_sender = ready_sender.clone();
            let (release_sender, release_receiver) = mpsc::channel();
            releases.push(release_sender);
            clients.push(std::thread::spawn(move || {
                let adapter = crate::GatewayRemoteAdapter::connect_from_discovery(
                    &client_discovery_path,
                    ClientKind::Test,
                    format!("shutdown-client-{sequence}.v1"),
                )
                .unwrap();
                client_ready_sender
                    .send(adapter.binding().client_session_id.clone())
                    .unwrap();
                release_receiver
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
                drop(adapter);
            }));
        }
        drop(ready_sender);

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut connected = Vec::new();
        while connected.len() < 2 && Instant::now() < deadline {
            while let Ok(client_session_id) = ready_receiver.try_recv() {
                connected.push(client_session_id);
            }
            dispatcher.pump(&mut core, &mut session);
            std::thread::yield_now();
        }
        assert_eq!(connected.len(), 2, "both connection workers must be active");
        assert_eq!(core.active_client_bindings().len(), 2);

        let shutdown_started = Instant::now();
        server.shutdown_and_join().unwrap();
        assert!(
            shutdown_started.elapsed() < Duration::from_secs(5),
            "active connection worker shutdown exceeded the bounded deadline"
        );
        let cleanup_deadline = Instant::now() + Duration::from_secs(10);
        while !core.active_client_bindings().is_empty() && Instant::now() < cleanup_deadline {
            dispatcher.pump(&mut core, &mut session);
            std::thread::yield_now();
        }
        assert!(core.active_client_bindings().is_empty());

        for release in releases {
            release.send(()).unwrap();
        }
        for client in clients {
            client.join().unwrap();
        }
        drop(publication);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_named_pipe_uses_current_user_acl_and_owner_thread_dispatch() {
        let security = GatewayNamedPipeSecurityEvidence::default();
        assert!(security.local_machine_only);
        assert!(security.current_os_user_only);
        assert!(security.protected_dacl);
        assert!(!security.tcp_listener_present);

        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-pipe-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut session = EditorSession::new();
        let created = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "Gateway Pipe".to_string(),
        }));
        assert_eq!(created.status, CommandStatus::Committed);
        let hello = ClientHello {
            schema_version: GATEWAY_CLIENT_HELLO_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            client_kind: ClientKind::Test,
            client_version: "named-pipe-test.v1".to_string(),
            supported_schema_versions: vec![AI_TOOL_CATALOG_SCHEMA_VERSION.to_string()],
            expected_editor_instance_id: crate::default_editor_instance_id(),
            requested_read_scope: vec!["catalog".to_string(), "project".to_string()],
        };
        let discovery = GatewayDiscoveryRecord::new("named-pipe-owner-thread");
        let (owner_client, mut dispatcher) = gateway_owner_thread_channel();
        let mut core = GatewayCore::new();
        let server = GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client).unwrap();
        let pipe_locator = discovery.pipe_locator.clone();
        let client_thread = std::thread::spawn(move || {
            let mut client = GatewayNamedPipeClient::connect(&pipe_locator).unwrap();
            let connected = client
                .exchange(&GatewayPipeWireRequest::Connect {
                    schema_version: GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION.to_string(),
                    hello,
                })
                .unwrap();
            let GatewayPipeWireReply::Connected(binding) = connected else {
                panic!("expected connected reply");
            };
            let catalog = client
                .exchange(&GatewayPipeWireRequest::Dispatch {
                    schema_version: GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION.to_string(),
                    request: GatewayRequest {
                        schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
                        gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                        request_id: "named-pipe-catalog".to_string(),
                        client_session_id: binding.client_session_id.clone(),
                        deadline_epoch_ms: None,
                        response_limit_bytes: 1024 * 1024,
                        payload: GatewayRequestPayload::Catalog(AiToolCatalogRequest::default()),
                    },
                })
                .unwrap();
            let closed = client
                .exchange(&GatewayPipeWireRequest::Close {
                    schema_version: GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION.to_string(),
                    client_session_id: binding.client_session_id,
                })
                .unwrap();
            (catalog, closed)
        });

        let deadline = Instant::now() + Duration::from_secs(10);
        while !client_thread.is_finished() && Instant::now() < deadline {
            dispatcher.pump(&mut core, &mut session);
            std::thread::yield_now();
        }
        assert!(client_thread.is_finished(), "Named Pipe test timed out");
        let (catalog, closed) = client_thread.join().unwrap();
        assert!(matches!(
            catalog,
            GatewayPipeWireReply::Dispatched(GatewayReply {
                payload: GatewayReplyPayload::Catalog(_),
                ..
            })
        ));
        assert!(matches!(closed, GatewayPipeWireReply::Closed(_)));
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_named_pipe_peer_is_bound_to_exactly_one_connected_session() {
        let (mut session, root, discovery, publication, owner_client, mut dispatcher, mut core) =
            pipe_fixture("peer-session-binding");
        let expected_editor_instance_id = discovery.editor_instance_id.clone();
        let hello_for = |client_version: &str| ClientHello {
            schema_version: GATEWAY_CLIENT_HELLO_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            client_kind: ClientKind::Test,
            client_version: client_version.to_string(),
            supported_schema_versions: vec![AI_TOOL_CATALOG_SCHEMA_VERSION.to_string()],
            expected_editor_instance_id: expected_editor_instance_id.clone(),
            requested_read_scope: vec!["catalog".to_string(), "project".to_string()],
        };
        let foreign_hello = hello_for("foreign-owner.v1");
        let foreign = core.connect(&mut session, foreign_hello).unwrap();
        let peer_hello = hello_for("raw-peer.v1");
        let server = GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client).unwrap();
        let pipe_locator = discovery.pipe_locator.clone();
        let foreign_for_peer = foreign.clone();
        let client_thread = std::thread::spawn(move || {
            let mut client = GatewayNamedPipeClient::connect(&pipe_locator).unwrap();
            let foreign_dispatch = |request_id: &str| GatewayPipeWireRequest::Dispatch {
                schema_version: GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION.to_string(),
                request: GatewayRequest {
                    schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
                    gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                    request_id: request_id.to_string(),
                    client_session_id: foreign_for_peer.client_session_id.clone(),
                    deadline_epoch_ms: None,
                    response_limit_bytes: 1024 * 1024,
                    payload: GatewayRequestPayload::Catalog(AiToolCatalogRequest::default()),
                },
            };

            let before_connect = client
                .exchange(&foreign_dispatch("before-connect"))
                .unwrap();
            let connected = client
                .exchange(&GatewayPipeWireRequest::Connect {
                    schema_version: GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION.to_string(),
                    hello: peer_hello.clone(),
                })
                .unwrap();
            let GatewayPipeWireReply::Connected(peer_binding) = connected else {
                panic!("raw peer must connect once");
            };
            let forged_dispatch = client
                .exchange(&foreign_dispatch("foreign-session-dispatch"))
                .unwrap();
            let forged_close = client
                .exchange(&GatewayPipeWireRequest::Close {
                    schema_version: GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION.to_string(),
                    client_session_id: foreign_for_peer.client_session_id.clone(),
                })
                .unwrap();
            let second_connect = client
                .exchange(&GatewayPipeWireRequest::Connect {
                    schema_version: GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION.to_string(),
                    hello: peer_hello,
                })
                .unwrap();
            let valid_close = client
                .exchange(&GatewayPipeWireRequest::Close {
                    schema_version: GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION.to_string(),
                    client_session_id: peer_binding.client_session_id,
                })
                .unwrap();
            (
                before_connect,
                forged_dispatch,
                forged_close,
                second_connect,
                valid_close,
            )
        });

        pump_until_finished(&client_thread, &mut dispatcher, &mut core, &mut session);
        let (before_connect, forged_dispatch, forged_close, second_connect, valid_close) =
            client_thread.join().unwrap();
        for (reply, expected_code) in [
            (before_connect, "gateway.pipe.connect_required"),
            (forged_dispatch, "gateway.pipe.session_mismatch"),
            (forged_close, "gateway.pipe.session_mismatch"),
            (second_connect, "gateway.pipe.connect_already_attempted"),
        ] {
            assert!(matches!(
                reply,
                GatewayPipeWireReply::Rejected(GatewayControlError { ref code, .. })
                    if code == expected_code
            ));
        }
        assert!(matches!(valid_close, GatewayPipeWireReply::Closed(_)));
        assert_eq!(core.active_client_bindings(), vec![foreign.clone()]);
        let inbox = core.approval_inbox(0);
        assert!(inbox.is_empty());
        core.close(&foreign.client_session_id);
        server.join().unwrap();
        drop(publication);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cli_adapter_uses_discovery_and_real_named_pipe_transport() {
        let (mut session, root, discovery, publication, owner_client, mut dispatcher, mut core) =
            pipe_fixture("cli-adapter");
        let server = GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client).unwrap();
        let discovery_path = publication.path().to_path_buf();
        let client_thread = std::thread::spawn(move || {
            let mut adapter = crate::GatewayRemoteAdapter::connect_from_discovery(
                &discovery_path,
                ClientKind::Cli,
                "cli-adapter-test.v1",
            )
            .unwrap();
            let reply = adapter
                .dispatch(GatewayRequestPayload::Catalog(
                    AiToolCatalogRequest::default(),
                ))
                .unwrap();
            let _ = adapter.close();
            reply
        });
        pump_until_finished(&client_thread, &mut dispatcher, &mut core, &mut session);
        let reply = client_thread.join().unwrap();
        assert!(matches!(reply.payload, GatewayReplyPayload::Catalog(_)));
        server.join().unwrap();
        drop(publication);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn named_pipe_server_accepts_sequential_client_reconnects() {
        let (mut session, root, discovery, publication, owner_client, mut dispatcher, mut core) =
            pipe_fixture("sequential-reconnect");
        let server = GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client).unwrap();
        let discovery_path = publication.path().to_path_buf();
        let client_thread = std::thread::spawn(move || {
            for sequence in 1..=2 {
                let mut adapter = crate::GatewayRemoteAdapter::connect_from_discovery(
                    &discovery_path,
                    ClientKind::Test,
                    format!("sequential-client-{sequence}.v1"),
                )
                .unwrap();
                let reply = adapter
                    .dispatch(GatewayRequestPayload::Catalog(
                        AiToolCatalogRequest::default(),
                    ))
                    .unwrap();
                assert!(matches!(reply.payload, GatewayReplyPayload::Catalog(_)));
                adapter.close().unwrap();
            }
        });

        pump_until_finished(&client_thread, &mut dispatcher, &mut core, &mut session);
        client_thread.join().unwrap();
        server.join().unwrap();
        drop(publication);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn named_pipe_server_accepts_concurrent_clients_and_isolates_peer_close() {
        let (mut session, root, discovery, publication, owner_client, mut dispatcher, mut core) =
            pipe_fixture("concurrent-clients");
        let server = GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client).unwrap();
        let first_discovery_path = publication.path().to_path_buf();
        let second_discovery_path = first_discovery_path.clone();
        let (first_ready_sender, first_ready_receiver) = mpsc::channel();
        let (release_first_sender, release_first_receiver) = mpsc::channel();
        let first_thread = std::thread::spawn(move || {
            let mut adapter = crate::GatewayRemoteAdapter::connect_from_discovery(
                &first_discovery_path,
                ClientKind::Test,
                "concurrent-client-1.v1",
            )?;
            let binding = adapter.binding().clone();
            let reply = adapter.dispatch(GatewayRequestPayload::Catalog(
                AiToolCatalogRequest::default(),
            ))?;
            first_ready_sender.send(binding).map_err(|error| {
                pipe_error("test.channel_closed", error.to_string(), "Fix test.")
            })?;
            release_first_receiver
                .recv_timeout(Duration::from_secs(10))
                .map_err(|error| {
                    pipe_error("test.release_timeout", error.to_string(), "Fix test.")
                })?;
            adapter.close()?;
            Ok::<_, GatewayControlError>(reply)
        });

        let first_deadline = Instant::now() + Duration::from_secs(10);
        let first_binding = loop {
            if let Ok(binding) = first_ready_receiver.try_recv() {
                break binding;
            }
            assert!(
                !first_thread.is_finished() && Instant::now() < first_deadline,
                "first concurrent client did not become ready"
            );
            dispatcher.pump(&mut core, &mut session);
            std::thread::yield_now();
        };

        let (second_ready_sender, second_ready_receiver) = mpsc::channel();
        let (continue_second_sender, continue_second_receiver) = mpsc::channel();
        let second_thread = std::thread::spawn(move || {
            let mut adapter = crate::GatewayRemoteAdapter::connect_from_discovery(
                &second_discovery_path,
                ClientKind::Test,
                "concurrent-client-2.v1",
            )?;
            let binding = adapter.binding().clone();
            let first_reply = adapter.dispatch(GatewayRequestPayload::Catalog(
                AiToolCatalogRequest::default(),
            ))?;
            second_ready_sender.send(binding).map_err(|error| {
                pipe_error("test.channel_closed", error.to_string(), "Fix test.")
            })?;
            continue_second_receiver
                .recv_timeout(Duration::from_secs(10))
                .map_err(|error| {
                    pipe_error("test.release_timeout", error.to_string(), "Fix test.")
                })?;
            let second_reply = adapter.dispatch(GatewayRequestPayload::Catalog(
                AiToolCatalogRequest::default(),
            ))?;
            adapter.close()?;
            Ok::<_, GatewayControlError>((first_reply, second_reply))
        });

        let second_deadline = Instant::now() + Duration::from_secs(10);
        let second_binding = loop {
            if let Ok(binding) = second_ready_receiver.try_recv() {
                break Some(binding);
            }
            if second_thread.is_finished() {
                break None;
            }
            assert!(
                Instant::now() < second_deadline,
                "second concurrent client did not become ready"
            );
            dispatcher.pump(&mut core, &mut session);
            std::thread::yield_now();
        };

        release_first_sender.send(()).unwrap();
        pump_until_finished(&first_thread, &mut dispatcher, &mut core, &mut session);
        let first_reply = first_thread.join().unwrap().unwrap();
        if second_binding.is_some() {
            continue_second_sender.send(()).unwrap();
            pump_until_finished(&second_thread, &mut dispatcher, &mut core, &mut session);
        }
        let second_result = second_thread.join().unwrap();
        server.join().unwrap();
        drop(publication);
        let _ = std::fs::remove_dir_all(root);

        assert!(matches!(
            first_reply.payload,
            GatewayReplyPayload::Catalog(_)
        ));
        let second_binding = second_binding.unwrap_or_else(|| {
            panic!(
                "second client must connect while the first remains connected: {:?}",
                second_result.as_ref().err()
            )
        });
        assert_ne!(
            first_binding.client_session_id,
            second_binding.client_session_id
        );
        let (second_first_reply, second_after_peer_close_reply) = second_result.unwrap();
        assert!(matches!(
            second_first_reply.payload,
            GatewayReplyPayload::Catalog(_)
        ));
        assert!(matches!(
            second_after_peer_close_reply.payload,
            GatewayReplyPayload::Catalog(_)
        ));
    }

    #[test]
    fn gateway_named_pipe_peer_eof_closes_session_and_access_request() {
        let (mut session, root, discovery, publication, owner_client, mut dispatcher, mut core) =
            pipe_fixture("peer-eof-cleanup");
        let server = GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client).unwrap();
        let discovery_path = publication.path().to_path_buf();
        let client_thread = std::thread::spawn(move || {
            let adapter = crate::GatewayRemoteAdapter::connect_from_discovery(
                &discovery_path,
                ClientKind::Test,
                "peer-eof-client.v1",
            )
            .unwrap();
            let client_session_id = adapter.binding().client_session_id.clone();
            drop(adapter);
            client_session_id
        });

        pump_until_finished(&client_thread, &mut dispatcher, &mut core, &mut session);
        let client_session_id = client_thread.join().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while core
            .active_client_bindings()
            .iter()
            .any(|binding| binding.client_session_id == client_session_id)
            && Instant::now() < deadline
        {
            dispatcher.pump(&mut core, &mut session);
            std::thread::yield_now();
        }
        assert!(core.active_client_bindings().is_empty());
        assert!(core.approval_inbox(0).is_empty());
        server.join().unwrap();
        drop(publication);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn mcp_stdio_adapter_lists_and_calls_gateway_tools_over_real_named_pipe() {
        let (mut session, root, discovery, publication, owner_client, mut dispatcher, mut core) =
            pipe_fixture("mcp-stdio-adapter");
        let server = GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client).unwrap();
        let discovery_path = publication.path().to_path_buf();
        let client_thread = std::thread::spawn(move || {
            let input = [
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
                r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
                r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"aife_catalog","arguments":{}}}"#,
            ]
            .join("\n");
            let mut output = Vec::new();
            crate::run_mcp_stdio(&discovery_path, Cursor::new(input), &mut output).unwrap();
            String::from_utf8(output).unwrap()
        });
        pump_until_finished(&client_thread, &mut dispatcher, &mut core, &mut session);
        let output = client_thread.join().unwrap();
        let messages = output
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(messages.len(), 3);
        assert_eq!(
            messages[0]["result"]["protocolVersion"],
            crate::MCP_PROTOCOL_VERSION
        );
        let tools = messages[1]["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 21);
        assert!(tools.iter().all(|tool| tool["name"] != "aife_execute"));
        assert!(tools
            .iter()
            .any(|tool| tool["name"] == "aife_project_search"));
        assert!(tools
            .iter()
            .any(|tool| tool["name"] == "aife_project_create"));
        assert!(tools
            .iter()
            .any(|tool| tool["name"] == "aife_project_mutate"));
        assert_eq!(messages[2]["result"]["isError"], false);
        assert!(
            messages[2]["result"]["structuredContent"]["sessionBinding"]["catalogDigest"]
                .as_str()
                .is_some_and(|digest| digest.starts_with("sha256:"))
        );
        server.join().unwrap();
        drop(publication);
        let _ = std::fs::remove_dir_all(root);
    }

    fn pipe_fixture(
        name: &str,
    ) -> (
        EditorSession,
        std::path::PathBuf,
        GatewayDiscoveryRecord,
        GatewayDiscoveryPublication,
        GatewayOwnerThreadClient,
        GatewayOwnerThreadDispatcher,
        GatewayCore,
    ) {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-{name}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut session = EditorSession::new();
        let created = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: name.to_string(),
        }));
        assert_eq!(created.status, CommandStatus::Committed);
        let discovery = GatewayDiscoveryRecord::new(format!("named-pipe-{name}"));
        let discovery_root = root.join("gateway-discovery");
        let publication =
            GatewayDiscoveryPublication::publish(&discovery_root, &discovery).unwrap();
        let (owner_client, dispatcher) = gateway_owner_thread_channel();
        let core = GatewayCore::new_for_editor_instance(discovery.editor_instance_id.clone());
        (
            session,
            root,
            discovery,
            publication,
            owner_client,
            dispatcher,
            core,
        )
    }

    fn pump_until_finished<T>(
        client_thread: &std::thread::JoinHandle<T>,
        dispatcher: &mut GatewayOwnerThreadDispatcher,
        core: &mut GatewayCore,
        session: &mut EditorSession,
    ) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !client_thread.is_finished() && Instant::now() < deadline {
            core.pump_operations(session, 1);
            dispatcher.pump(core, session);
            std::thread::yield_now();
        }
        assert!(client_thread.is_finished(), "Adapter test timed out");
    }
}
