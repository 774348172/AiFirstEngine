use engine_runtime::project_runtime_module::LinkedProjectRuntimeSet;
use std::sync::Arc;

pub fn default_editor_linked_project_runtimes() -> Arc<LinkedProjectRuntimeSet> {
    Arc::new(LinkedProjectRuntimeSet::explicit_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_editor_composition_links_only_explicit_empty_module() {
        let linked = default_editor_linked_project_runtimes();
        assert_eq!(linked.len(), 1);
        assert_eq!(
            linked.only_descriptor().unwrap(),
            &engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor::empty()
        );
    }
}
