use engine_runtime::project_runtime_module::{
    EmptyProjectRuntimeModule, LinkedProjectRuntimeSet, ProjectRuntimeModule,
};
use std::sync::Arc;

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--describe-project-runtime-module") {
        let module = EmptyProjectRuntimeModule::new();
        match serde_json::to_string(module.descriptor()) {
            Ok(descriptor) => {
                println!("{descriptor}");
                return;
            }
            Err(error) => {
                eprintln!("project module descriptor serialization failed: {error}");
                std::process::exit(1);
            }
        }
    }

    std::process::exit(runtime_cli::run_from_env_with_linked_modules(Arc::new(
        LinkedProjectRuntimeSet::explicit_empty(),
    )));
}
