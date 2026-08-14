#[cfg(feature = "real-wgpu")]
pub(crate) fn backend_present_label(backend_name: &str) -> String {
    format!("wgpu::{backend_name}")
}

#[cfg(feature = "real-wgpu")]
pub(crate) fn backend_error_label(backend_name: &str) -> String {
    format!("wgpu::{backend_name:?}")
}
