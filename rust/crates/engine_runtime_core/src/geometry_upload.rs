//! Existing upload receipt/state shared by CPU cell transactions and native.
/// Opaque backend-issued receipt. A project cannot supply a boolean GPU ACK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometryUploadTicket {
    id: u64,
    epoch: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryUploadState {
    Uploading,
    Submitted,
    Ready,
    Abandoned,
}

impl GeometryUploadTicket {
    /// Internal issuance of a fresh identity, never a completion acknowledgement.
    /// Callers cannot choose an existing native ticket's identity.
    #[doc(hidden)]
    pub fn issue_for_backend(epoch: u64) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |value| value.checked_add(1),
            )
            .expect("geometry upload ticket identity exhausted");
        Self { id, epoch }
    }
    #[doc(hidden)]
    pub fn backend_parts(self) -> (u64, u64) {
        (self.id, self.epoch)
    }
}
