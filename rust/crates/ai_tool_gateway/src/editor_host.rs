use crate::{
    default_discovery_root, GatewayControlError, GatewayDiscoveryPublication,
    GatewayDiscoveryRecord, GatewayNamedPipeServer, GatewayOwnerThreadClient,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorGatewayHostBinding {
    pub editor_instance_id: String,
}

pub struct EditorGatewayHost {
    binding: EditorGatewayHostBinding,
    discovery_path: PathBuf,
    publication: Option<GatewayDiscoveryPublication>,
    server: Option<GatewayNamedPipeServer>,
}

impl EditorGatewayHost {
    pub fn start(
        editor_instance_id: impl Into<String>,
        owner_client: GatewayOwnerThreadClient,
    ) -> Result<Self, GatewayControlError> {
        let root = default_discovery_root()?;
        Self::start_in_root(&root, editor_instance_id, owner_client)
    }

    pub fn start_in_root(
        discovery_root: &Path,
        editor_instance_id: impl Into<String>,
        owner_client: GatewayOwnerThreadClient,
    ) -> Result<Self, GatewayControlError> {
        let editor_instance_id = editor_instance_id.into();
        let binding = EditorGatewayHostBinding {
            editor_instance_id: editor_instance_id.clone(),
        };
        let record = GatewayDiscoveryRecord::new(editor_instance_id);
        let server = GatewayNamedPipeServer::spawn(&record.pipe_locator, owner_client)?;
        let publication = GatewayDiscoveryPublication::publish(discovery_root, &record)?;
        let discovery_path = publication.path().to_path_buf();
        Ok(Self {
            binding,
            discovery_path,
            publication: Some(publication),
            server: Some(server),
        })
    }

    pub fn binding(&self) -> &EditorGatewayHostBinding {
        &self.binding
    }

    pub fn discovery_path(&self) -> &Path {
        &self.discovery_path
    }

    pub fn shutdown(mut self) -> Result<(), GatewayControlError> {
        self.remove_publication();
        self.shutdown_server()
    }

    fn remove_publication(&mut self) {
        self.publication.take();
    }

    fn shutdown_server(&mut self) -> Result<(), GatewayControlError> {
        match self.server.take() {
            Some(mut server) => server.shutdown_and_join(),
            None => Ok(()),
        }
    }
}

impl Drop for EditorGatewayHost {
    fn drop(&mut self) {
        self.remove_publication();
        let _ = self.shutdown_server();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::{gateway_owner_thread_channel, resolve_gateway_discovery_path_in_root};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn editor_gateway_host_publishes_resolves_and_removes_its_binding() {
        let root = unique_root("lifecycle");
        let (owner_client, _dispatcher) = gateway_owner_thread_channel();
        let host =
            EditorGatewayHost::start_in_root(&root, "editor-instance-lifecycle", owner_client)
                .unwrap();
        let path = host.discovery_path().to_path_buf();

        assert_eq!(
            resolve_gateway_discovery_path_in_root(&root, None).unwrap(),
            path
        );
        assert!(path.exists());

        host.shutdown().unwrap();
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_discovery_resolver_fails_closed_on_ambiguity_and_accepts_exact_instance() {
        let root = unique_root("selector");
        let (client_a, _dispatcher_a) = gateway_owner_thread_channel();
        let (client_b, _dispatcher_b) = gateway_owner_thread_channel();
        let host_a =
            EditorGatewayHost::start_in_root(&root, "editor-instance-a", client_a).unwrap();
        let host_b =
            EditorGatewayHost::start_in_root(&root, "editor-instance-b", client_b).unwrap();

        let error = resolve_gateway_discovery_path_in_root(&root, None).unwrap_err();
        assert_eq!(error.code, "gateway.discovery.ambiguous_editor_instance");
        assert_eq!(
            resolve_gateway_discovery_path_in_root(
                &root,
                Some(&host_b.binding().editor_instance_id),
            )
            .unwrap(),
            host_b.discovery_path()
        );

        drop(host_a);
        drop(host_b);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn editor_gateway_host_binding_is_instance_owned_and_project_independent() {
        let root = unique_root("editor-instance");
        let (owner_client, _dispatcher) = gateway_owner_thread_channel();
        let host =
            EditorGatewayHost::start_in_root(&root, "editor-instance-host", owner_client).unwrap();

        assert_eq!(host.binding().editor_instance_id, "editor-instance-host");
        assert!(host.discovery_path().exists());

        host.shutdown().unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    fn unique_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ai-tool-gateway-host-{label}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
