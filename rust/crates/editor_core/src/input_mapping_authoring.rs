use editor_ui_model::{
    InputActionValueKind, InputControlCatalogEntryModel, InputControlCatalogModel,
    InputControlDeviceKindModel, InputMappingActionSummary, InputMappingAuthoringCommand,
    InputMappingAuthoringDiagnostic, InputMappingAuthoringModel, InputMappingAuthoringReport,
    InputMappingBindingSummary, InputMappingContextSummary, InputMappingDiagnosticSeverity,
    InputMappingPreviewAction, InputMappingPreviewResult, InputMappingPreviewStatus,
    InputMappingReportLevel, InputMappingValidationStatus, InputProcessorKind, InputTriggerKind,
};
use engine_input::{
    ActionValue, InputActionDefinition, InputActionValueType, InputBindingDefinition,
    InputContextDefinition, InputControlCatalog, InputControlDeviceKind, InputDiagnosticSeverity,
    InputMappingAsset, InputMappingDiagnostic, InputMappingReport, InputProcessorPreset,
    InputResolver, InputTriggerPreset, PointerPosition, RuntimeInputEvent, RuntimeInputFrame,
    RuntimePointerButton,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const INPUT_MAPPING_AUTHORING_REPORT_SCHEMA_VERSION: &str = "input-mapping-authoring-report.v1";
pub const INPUT_MAPPING_VISUAL_AUTHORING_REPORT_SCHEMA_VERSION: &str =
    "input-mapping-visual-authoring-report.v1";

#[derive(Debug, Clone, PartialEq)]
pub struct InputMappingEditorState {
    pub selected_path: String,
    pub selected_context_id: Option<String>,
    pub selected_action_id: Option<String>,
    pub selected_binding_id: Option<String>,
    pub source_hash: String,
    pub draft_mapping: InputMappingAsset,
    pub dirty: bool,
    pub capture_binding_id: Option<String>,
    pub preview: Option<InputMappingPreviewResult>,
    pub report_level: InputMappingReportLevel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputMappingEditCommand {
    AddContext {
        context_id: String,
        priority: i32,
    },
    RemoveContext {
        context_id: String,
    },
    SetContextPriority {
        context_id: String,
        priority: i32,
    },
    SetContextConsumeInput {
        context_id: String,
        consume_input: bool,
    },
    AddAction {
        action_id: String,
        value_type: InputActionValueKind,
    },
    RemoveAction {
        action_id: String,
    },
    SetActionValueType {
        action_id: String,
        value_type: InputActionValueKind,
    },
    AddBinding {
        context_id: String,
        action_id: String,
        device_path: String,
    },
    RemoveBinding {
        binding_index: usize,
    },
    SetBindingDevicePath {
        binding_index: usize,
        device_path: String,
    },
    SetBindingProcessorByIndex {
        binding_index: usize,
        processor: InputProcessorKind,
    },
    RemoveBindingById {
        binding_id: String,
    },
    SetBindingDevicePathById {
        binding_id: String,
        device_path: String,
    },
    SetBindingTrigger {
        binding_id: String,
        trigger: InputTriggerKind,
    },
    SetBindingProcessor {
        binding_id: String,
        processor: InputProcessorKind,
    },
}

pub struct InputMappingAuthoringService;

impl InputMappingAuthoringService {
    pub fn create_default() -> InputMappingAsset {
        InputMappingAsset::explicit_empty("input.default")
    }

    pub fn load(project_root: &Path, relative_path: &str) -> Result<InputMappingAsset, String> {
        let path = project_root.join(normalize_project_relative_path(relative_path));
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read InputMappingAsset: {error}"))?;
        serde_json::from_str::<InputMappingAsset>(&text)
            .map_err(|error| format!("Failed to parse InputMappingAsset: {error}"))
    }

    pub fn open_editor_state(
        project_root: &Path,
        relative_path: &str,
    ) -> Result<InputMappingEditorState, String> {
        let mapping = Self::load(project_root, relative_path)?;
        let source_hash = Self::source_hash(project_root, relative_path)?;
        Ok(InputMappingEditorState {
            selected_path: relative_path.to_string(),
            selected_context_id: mapping.contexts.first().map(|context| context.id.clone()),
            selected_action_id: mapping.actions.first().map(|action| action.id.clone()),
            selected_binding_id: mapping
                .bindings
                .first()
                .map(|binding| binding.binding_id.clone()),
            source_hash,
            draft_mapping: mapping,
            dirty: false,
            capture_binding_id: None,
            preview: None,
            report_level: InputMappingReportLevel::Summary,
        })
    }

    pub fn source_hash(project_root: &Path, relative_path: &str) -> Result<String, String> {
        let path = project_root.join(normalize_project_relative_path(relative_path));
        let bytes = fs::read(&path)
            .map_err(|error| format!("Failed to read InputMappingAsset hash: {error}"))?;
        Ok(hash_bytes(&bytes))
    }

    pub fn save(
        project_root: &Path,
        relative_path: &str,
        mapping: &InputMappingAsset,
    ) -> Result<(), String> {
        let scope =
            crate::ProjectWriteScope::open(project_root).map_err(|error| error.to_string())?;
        Self::save_in_scope(&scope, relative_path, mapping)
    }

    pub fn save_in_scope(
        scope: &crate::ProjectWriteScope,
        relative_path: &str,
        mapping: &InputMappingAsset,
    ) -> Result<(), String> {
        let mut normalized = mapping.clone();
        normalized.normalize();
        let json = serde_json::to_string_pretty(&normalized)
            .map_err(|error| format!("Failed to serialize InputMappingAsset: {error}"))?;
        scope
            .write_atomic(relative_path, json.as_bytes())
            .map(|_| ())
            .map_err(|error| format!("Failed to atomically save InputMappingAsset: {error}"))
    }

    pub fn save_editor_state(
        project_root: &Path,
        state: &mut InputMappingEditorState,
    ) -> Result<(), String> {
        let scope =
            crate::ProjectWriteScope::open(project_root).map_err(|error| error.to_string())?;
        Self::save_editor_state_in_scope(&scope, state)
    }

    pub fn save_editor_state_in_scope(
        scope: &crate::ProjectWriteScope,
        state: &mut InputMappingEditorState,
    ) -> Result<(), String> {
        let project_root = scope.display_root();
        let current_hash = Self::source_hash(project_root, &state.selected_path)?;
        if current_hash != state.source_hash {
            return Err(format!(
                "input_mapping.stale_source_hash: expected {}, got {}",
                state.source_hash, current_hash
            ));
        }
        let validation = state.draft_mapping.validate();
        if validation.has_errors() {
            return Err(
                "input_mapping.validation_failed: fix mapping errors before Save.".to_string(),
            );
        }
        Self::save_in_scope(scope, &state.selected_path, &state.draft_mapping)?;
        state.draft_mapping = Self::load(project_root, &state.selected_path)?;
        state.source_hash = Self::source_hash(project_root, &state.selected_path)?;
        state.dirty = false;
        Ok(())
    }

    pub fn preview(mapping: &InputMappingAsset, device_path: &str) -> InputMappingPreviewResult {
        let trigger = mapping
            .bindings
            .iter()
            .find(|binding| binding.device_path.eq_ignore_ascii_case(device_path))
            .map(|binding| &binding.trigger);
        let mut frame = RuntimeInputFrame::new(1, "input-mapping-preview");
        frame.pointer_position = Some(PointerPosition { x: 64.0, y: 32.0 });
        if let Some(event) = synthetic_input_event(device_path, trigger) {
            frame.events.push(event);
        }
        let input_event_kind = frame
            .events
            .first()
            .map(RuntimeInputEvent::kind)
            .unwrap_or("Unsupported")
            .to_string();
        let resolved = InputResolver::resolve(&frame, mapping);
        let mut candidates = mapping
            .bindings
            .iter()
            .filter(|binding| binding.device_path.eq_ignore_ascii_case(device_path))
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            mapping
                .contexts
                .iter()
                .find(|context| context.id == right.context_id)
                .map(|context| context.priority)
                .unwrap_or_default()
                .cmp(
                    &mapping
                        .contexts
                        .iter()
                        .find(|context| context.id == left.context_id)
                        .map(|context| context.priority)
                        .unwrap_or_default(),
                )
        });
        let mut consumed = false;
        let mut matched_binding_ids = Vec::new();
        let mut shadowed_binding_ids = Vec::new();
        for binding in candidates {
            if consumed {
                shadowed_binding_ids.push(binding.binding_id.clone());
                continue;
            }
            matched_binding_ids.push(binding.binding_id.clone());
            consumed = mapping
                .contexts
                .iter()
                .find(|context| context.id == binding.context_id)
                .is_some_and(|context| context.consume_input);
        }
        let actions = resolved
            .action_snapshot
            .actions
            .iter()
            .map(|action| InputMappingPreviewAction {
                action_id: action.action_id.clone(),
                value: match &action.value {
                    ActionValue::Button { phase } => phase.as_str().to_string(),
                    ActionValue::Axis1 { value } => format!("{}", value.value),
                    ActionValue::Axis2 { value } => format!("{},{}", value.x, value.y),
                    ActionValue::Pointer { position } => {
                        format!("{},{}", position.x, position.y)
                    }
                },
            })
            .collect::<Vec<_>>();
        let diagnostics = resolved
            .diagnostics
            .iter()
            .map(|diagnostic| ui_diagnostic(diagnostic, None))
            .collect::<Vec<_>>();
        InputMappingPreviewResult {
            schema_version: "input-mapping-preview.v1".to_string(),
            status: if diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == InputMappingDiagnosticSeverity::Error)
            {
                InputMappingPreviewStatus::Invalid
            } else if actions.is_empty() {
                InputMappingPreviewStatus::NoAction
            } else {
                InputMappingPreviewStatus::Resolved
            },
            device_path: device_path.to_string(),
            input_event_kind,
            matched_binding_ids,
            shadowed_binding_ids,
            actions,
            diagnostics,
        }
    }

    pub fn apply(
        mapping: &mut InputMappingAsset,
        command: InputMappingEditCommand,
    ) -> Result<(), String> {
        match command {
            InputMappingEditCommand::AddContext {
                context_id,
                priority,
            } => {
                if context_id.trim().is_empty() {
                    return Err("Input context id cannot be empty.".to_string());
                }
                if mapping
                    .contexts
                    .iter()
                    .any(|context| context.id == context_id)
                {
                    return Err(format!("Input context already exists: {context_id}"));
                }
                mapping
                    .contexts
                    .push(InputContextDefinition::new(context_id, priority));
            }
            InputMappingEditCommand::RemoveContext { context_id } => {
                let before = mapping.contexts.len();
                mapping.contexts.retain(|context| context.id != context_id);
                mapping
                    .bindings
                    .retain(|binding| binding.context_id != context_id);
                if mapping.contexts.len() == before {
                    return Err(format!("Input context does not exist: {context_id}"));
                }
            }
            InputMappingEditCommand::SetContextPriority {
                context_id,
                priority,
            } => {
                let context = mapping
                    .contexts
                    .iter_mut()
                    .find(|context| context.id == context_id)
                    .ok_or_else(|| format!("Input context does not exist: {context_id}"))?;
                context.priority = priority;
            }
            InputMappingEditCommand::SetContextConsumeInput {
                context_id,
                consume_input,
            } => {
                let context = mapping
                    .contexts
                    .iter_mut()
                    .find(|context| context.id == context_id)
                    .ok_or_else(|| format!("Input context does not exist: {context_id}"))?;
                context.consume_input = consume_input;
            }
            InputMappingEditCommand::AddAction {
                action_id,
                value_type,
            } => {
                if action_id.trim().is_empty() {
                    return Err("Input action id cannot be empty.".to_string());
                }
                if mapping.actions.iter().any(|action| action.id == action_id) {
                    return Err(format!("Input action already exists: {action_id}"));
                }
                mapping.actions.push(InputActionDefinition::new(
                    action_id,
                    input_value_type_from_ui(value_type),
                ));
            }
            InputMappingEditCommand::RemoveAction { action_id } => {
                let before = mapping.actions.len();
                mapping.actions.retain(|action| action.id != action_id);
                mapping
                    .bindings
                    .retain(|binding| binding.action_id != action_id);
                if mapping.actions.len() == before {
                    return Err(format!("Input action does not exist: {action_id}"));
                }
            }
            InputMappingEditCommand::SetActionValueType {
                action_id,
                value_type,
            } => {
                let action = mapping
                    .actions
                    .iter_mut()
                    .find(|action| action.id == action_id)
                    .ok_or_else(|| format!("Input action does not exist: {action_id}"))?;
                action.value_type = input_value_type_from_ui(value_type);
            }
            InputMappingEditCommand::AddBinding {
                context_id,
                action_id,
                device_path,
            } => {
                if !mapping
                    .contexts
                    .iter()
                    .any(|context| context.id == context_id)
                {
                    mapping
                        .contexts
                        .push(InputContextDefinition::new(context_id.clone(), 0));
                }
                mapping.bindings.push(InputBindingDefinition::new(
                    context_id,
                    action_id,
                    device_path,
                ));
            }
            InputMappingEditCommand::RemoveBinding { binding_index } => {
                if binding_index >= mapping.bindings.len() {
                    return Err(format!("Input binding index out of range: {binding_index}"));
                }
                mapping.bindings.remove(binding_index);
            }
            InputMappingEditCommand::SetBindingDevicePath {
                binding_index,
                device_path,
            } => {
                let Some(binding) = mapping.bindings.get_mut(binding_index) else {
                    return Err(format!("Input binding index out of range: {binding_index}"));
                };
                binding.device_path = device_path;
            }
            InputMappingEditCommand::SetBindingProcessorByIndex {
                binding_index,
                processor,
            } => {
                let Some(binding) = mapping.bindings.get_mut(binding_index) else {
                    return Err(format!("Input binding index out of range: {binding_index}"));
                };
                binding.processor = input_processor_from_ui(processor);
            }
            InputMappingEditCommand::RemoveBindingById { binding_id } => {
                let before = mapping.bindings.len();
                mapping
                    .bindings
                    .retain(|binding| binding.binding_id != binding_id);
                if mapping.bindings.len() == before {
                    return Err(format!("Input binding does not exist: {binding_id}"));
                }
            }
            InputMappingEditCommand::SetBindingDevicePathById {
                binding_id,
                device_path,
            } => {
                binding_by_id_mut(mapping, &binding_id)?.device_path = device_path;
            }
            InputMappingEditCommand::SetBindingTrigger {
                binding_id,
                trigger,
            } => {
                binding_by_id_mut(mapping, &binding_id)?.trigger = input_trigger_from_ui(trigger);
            }
            InputMappingEditCommand::SetBindingProcessor {
                binding_id,
                processor,
            } => {
                binding_by_id_mut(mapping, &binding_id)?.processor =
                    input_processor_from_ui(processor);
            }
        }
        mapping.normalize();
        Ok(())
    }

    pub fn build_model(
        project_root: Option<&Path>,
        selected_path: Option<String>,
        mapping: Option<&InputMappingAsset>,
        mapping_count: usize,
    ) -> InputMappingAuthoringModel {
        Self::build_model_with_editor_state(
            project_root,
            selected_path,
            mapping,
            mapping_count,
            None,
        )
    }

    pub fn build_model_with_editor_state(
        project_root: Option<&Path>,
        selected_path: Option<String>,
        mapping: Option<&InputMappingAsset>,
        mapping_count: usize,
        editor_state: Option<&InputMappingEditorState>,
    ) -> InputMappingAuthoringModel {
        let Some(mapping) = mapping else {
            return InputMappingAuthoringModel {
                project_root: project_root.map(|path| path.display().to_string()),
                selected_path,
                report: InputMappingAuthoringReport {
                    schema_version: INPUT_MAPPING_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
                    mapping_count,
                    ..InputMappingAuthoringReport::default()
                },
                commands: authoring_commands(project_root.is_some(), false),
                control_catalog: control_catalog_model(),
                ..InputMappingAuthoringModel::empty()
            };
        };
        let report = report_for_mapping(mapping, mapping_count, selected_path.clone());
        InputMappingAuthoringModel {
            project_root: project_root.map(|path| path.display().to_string()),
            selected_path,
            mapping_id: Some(mapping.asset_id.clone()),
            selected_context_id: editor_state.and_then(|state| state.selected_context_id.clone()),
            selected_action_id: editor_state.and_then(|state| state.selected_action_id.clone()),
            selected_binding_id: editor_state.and_then(|state| state.selected_binding_id.clone()),
            source_hash: editor_state.map(|state| state.source_hash.clone()),
            dirty: editor_state.is_some_and(|state| state.dirty),
            capture_binding_id: editor_state.and_then(|state| state.capture_binding_id.clone()),
            capture_accepts_pointer_position: editor_state.is_some_and(|state| {
                let Some(binding_id) = state.capture_binding_id.as_deref() else {
                    return false;
                };
                let Some(binding) = state
                    .draft_mapping
                    .bindings
                    .iter()
                    .find(|binding| binding.binding_id == binding_id)
                else {
                    return false;
                };
                state
                    .draft_mapping
                    .actions
                    .iter()
                    .find(|action| action.id == binding.action_id)
                    .is_some_and(|action| action.value_type == InputActionValueType::Pointer)
            }),
            preview: editor_state.and_then(|state| state.preview.clone()),
            report_level: editor_state
                .map(|state| state.report_level)
                .unwrap_or(InputMappingReportLevel::Summary),
            actions: action_summaries(mapping),
            contexts: context_summaries(mapping),
            bindings: binding_summaries(mapping),
            control_catalog: control_catalog_model(),
            report,
            commands: authoring_commands(project_root.is_some(), true),
            empty_message: String::new(),
        }
    }
}

fn binding_by_id_mut<'a>(
    mapping: &'a mut InputMappingAsset,
    binding_id: &str,
) -> Result<&'a mut InputBindingDefinition, String> {
    mapping
        .bindings
        .iter_mut()
        .find(|binding| binding.binding_id == binding_id)
        .ok_or_else(|| format!("Input binding does not exist: {binding_id}"))
}

fn input_trigger_from_ui(trigger: InputTriggerKind) -> InputTriggerPreset {
    match trigger {
        InputTriggerKind::Down => InputTriggerPreset::Down,
        InputTriggerKind::Pressed => InputTriggerPreset::Pressed,
        InputTriggerKind::Released => InputTriggerPreset::Released,
        InputTriggerKind::Hold { seconds } => InputTriggerPreset::Hold { seconds },
        InputTriggerKind::Tap { max_seconds } => InputTriggerPreset::Tap { max_seconds },
    }
}

fn input_processor_from_ui(processor: InputProcessorKind) -> InputProcessorPreset {
    match processor {
        InputProcessorKind::None => InputProcessorPreset::None,
        InputProcessorKind::Deadzone { threshold } => InputProcessorPreset::Deadzone { threshold },
        InputProcessorKind::Normalize => InputProcessorPreset::Normalize,
        InputProcessorKind::Scale { factor } => InputProcessorPreset::Scale { factor },
        InputProcessorKind::Invert => InputProcessorPreset::Invert,
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    format!("fnv1a64:{hash:016x}")
}

pub fn scan_input_mapping_paths(project_root: &Path) -> Vec<String> {
    let input_dir = project_root.join("Input");
    let Ok(read_dir) = fs::read_dir(input_dir) else {
        return Vec::new();
    };
    let mut paths = read_dir
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| is_input_mapping_path(path))
        .map(|path| {
            path.strip_prefix(project_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub fn load_first_input_mapping(project_root: &Path) -> Option<(String, InputMappingAsset)> {
    scan_input_mapping_paths(project_root)
        .into_iter()
        .find_map(|path| {
            InputMappingAuthoringService::load(project_root, &path)
                .ok()
                .map(|mapping| (path, mapping))
        })
}

pub fn scan_input_action_references(project_root: &Path, action_id: &str) -> Vec<String> {
    let mut references = Vec::new();
    scan_action_reference_directory(project_root, project_root, action_id, &mut references);
    references.sort();
    references.dedup();
    references
}

fn scan_action_reference_directory(
    project_root: &Path,
    directory: &Path,
    action_id: &str,
    references: &mut Vec<String>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if matches!(
                name.to_ascii_lowercase().as_str(),
                ".git" | ".aife-candidates" | "build" | "input" | "library" | "target"
            ) {
                continue;
            }
            scan_action_reference_directory(project_root, &path, action_id, references);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if json_contains_string(&value, action_id) {
            references.push(
                path.strip_prefix(project_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

fn json_contains_string(value: &serde_json::Value, expected: &str) -> bool {
    match value {
        serde_json::Value::String(value) => value == expected,
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| json_contains_string(value, expected)),
        serde_json::Value::Object(values) => values
            .values()
            .any(|value| json_contains_string(value, expected)),
        _ => false,
    }
}

fn report_for_mapping(
    mapping: &InputMappingAsset,
    mapping_count: usize,
    path: Option<String>,
) -> InputMappingAuthoringReport {
    let validation = mapping.validate();
    InputMappingAuthoringReport {
        schema_version: INPUT_MAPPING_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
        mapping_count,
        action_count: mapping.actions.len(),
        context_count: mapping.contexts.len(),
        binding_count: mapping.bindings.len(),
        validation_status: validation_status(&validation),
        diagnostics: validation
            .diagnostics
            .iter()
            .map(|diagnostic| ui_diagnostic(diagnostic, path.clone()))
            .collect(),
    }
}

fn ui_diagnostic(
    diagnostic: &InputMappingDiagnostic,
    path: Option<String>,
) -> InputMappingAuthoringDiagnostic {
    InputMappingAuthoringDiagnostic {
        severity: match diagnostic.severity {
            InputDiagnosticSeverity::Warning => InputMappingDiagnosticSeverity::Warning,
            InputDiagnosticSeverity::Error => InputMappingDiagnosticSeverity::Error,
        },
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        path,
        context_id: diagnostic.context_id.clone(),
        action_id: diagnostic.action_id.clone(),
        binding_id: diagnostic.binding_id.clone(),
        field_path: diagnostic.field_path.clone(),
        suggested_fix: diagnostic.suggested_fix.clone(),
    }
}

fn synthetic_input_event(
    device_path: &str,
    trigger: Option<&InputTriggerPreset>,
) -> Option<RuntimeInputEvent> {
    let (device, control) = device_path.split_once('/')?;
    let released = matches!(trigger, Some(InputTriggerPreset::Released));
    let held = matches!(trigger, Some(InputTriggerPreset::Hold { .. }));
    match device.to_ascii_lowercase().as_str() {
        "keyboard" => Some(if released {
            RuntimeInputEvent::KeyUp {
                key: control.to_string(),
            }
        } else if held {
            RuntimeInputEvent::KeyHeld {
                key: control.to_string(),
            }
        } else {
            RuntimeInputEvent::KeyDown {
                key: control.to_string(),
            }
        }),
        "mouse" => match control.to_ascii_lowercase().as_str() {
            "left" | "right" | "middle" => {
                let button = match control.to_ascii_lowercase().as_str() {
                    "right" => RuntimePointerButton::Secondary,
                    "middle" => RuntimePointerButton::Middle,
                    _ => RuntimePointerButton::Primary,
                };
                Some(if released {
                    RuntimeInputEvent::PointerUp {
                        x: 64.0,
                        y: 32.0,
                        button,
                    }
                } else if held {
                    RuntimeInputEvent::PointerHeld {
                        x: 64.0,
                        y: 32.0,
                        button,
                    }
                } else {
                    RuntimeInputEvent::PointerDown {
                        x: 64.0,
                        y: 32.0,
                        button,
                    }
                })
            }
            "position" => Some(RuntimeInputEvent::PointerMove { x: 64.0, y: 32.0 }),
            "wheel" => Some(RuntimeInputEvent::MouseWheel { delta: 1.0 }),
            _ => None,
        },
        "gamepad" => match control.to_ascii_lowercase().as_str() {
            "south" => Some(if released {
                RuntimeInputEvent::GamepadButtonUp {
                    gamepad_id: 0,
                    button: "South".to_string(),
                }
            } else if held {
                RuntimeInputEvent::GamepadButtonHeld {
                    gamepad_id: 0,
                    button: "South".to_string(),
                }
            } else {
                RuntimeInputEvent::GamepadButtonDown {
                    gamepad_id: 0,
                    button: "South".to_string(),
                }
            }),
            "leftstick" => Some(RuntimeInputEvent::GamepadAxis2d {
                gamepad_id: 0,
                axis: "LeftStick".to_string(),
                x: 0.75,
                y: -0.25,
            }),
            _ => None,
        },
        _ => None,
    }
}

fn validation_status(report: &InputMappingReport) -> InputMappingValidationStatus {
    if report.has_errors() {
        InputMappingValidationStatus::Error
    } else if report.diagnostics.is_empty() {
        InputMappingValidationStatus::Ok
    } else {
        InputMappingValidationStatus::Warning
    }
}

fn action_summaries(mapping: &InputMappingAsset) -> Vec<InputMappingActionSummary> {
    mapping
        .actions
        .iter()
        .map(|action| InputMappingActionSummary {
            action_id: action.id.clone(),
            value_type: ui_value_type_from_input(action.value_type),
            binding_count: mapping
                .bindings
                .iter()
                .filter(|binding| binding.action_id == action.id)
                .count(),
        })
        .collect()
}

fn context_summaries(mapping: &InputMappingAsset) -> Vec<InputMappingContextSummary> {
    mapping
        .contexts
        .iter()
        .map(|context| InputMappingContextSummary {
            context_id: context.id.clone(),
            priority: context.priority,
            consume_input: context.consume_input,
            enabled_by_default: context.enabled_by_default,
        })
        .collect()
}

fn binding_summaries(mapping: &InputMappingAsset) -> Vec<InputMappingBindingSummary> {
    mapping
        .bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| InputMappingBindingSummary {
            binding_id: binding.binding_id.clone(),
            binding_index: index,
            context_id: binding.context_id.clone(),
            action_id: binding.action_id.clone(),
            device_path: binding.device_path.clone(),
            processor: format!("{:?}", binding.processor),
            trigger: format!("{:?}", binding.trigger),
        })
        .collect()
}

fn control_catalog_model() -> InputControlCatalogModel {
    let catalog = InputControlCatalog::supported();
    InputControlCatalogModel {
        schema_version: catalog.schema_version,
        controls: catalog
            .controls
            .into_iter()
            .map(|control| InputControlCatalogEntryModel {
                device_path: control.device_path,
                label: control.label,
                device_kind: match control.device_kind {
                    InputControlDeviceKind::Keyboard => InputControlDeviceKindModel::Keyboard,
                    InputControlDeviceKind::Mouse => InputControlDeviceKindModel::Mouse,
                    InputControlDeviceKind::Gamepad => InputControlDeviceKindModel::Gamepad,
                },
                compatible_value_types: control
                    .compatible_value_types
                    .into_iter()
                    .map(ui_value_type_from_input)
                    .collect(),
                selectable: control.selectable,
                capture_supported: control.capture_supported,
            })
            .collect(),
    }
}

fn authoring_commands(
    project_open: bool,
    mapping_loaded: bool,
) -> Vec<InputMappingAuthoringCommand> {
    vec![
        InputMappingAuthoringCommand::new(
            "create_default_input_mapping",
            "Create Default",
            project_open,
            (!project_open).then(|| "Open a project first.".to_string()),
        ),
        InputMappingAuthoringCommand::new(
            "save_input_mapping",
            "Save",
            mapping_loaded,
            (!mapping_loaded).then(|| "Select an InputMapping asset first.".to_string()),
        ),
        InputMappingAuthoringCommand::new(
            "validate_input_mapping",
            "Validate",
            mapping_loaded,
            (!mapping_loaded).then(|| "Select an InputMapping asset first.".to_string()),
        ),
    ]
}

fn is_input_mapping_path(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".json") && (lower.contains("input") || lower.ends_with(".input-mapping.json"))
}

fn normalize_project_relative_path(path: &str) -> PathBuf {
    PathBuf::from(path.replace('\\', "/"))
}

fn input_value_type_from_ui(value_type: InputActionValueKind) -> InputActionValueType {
    match value_type {
        InputActionValueKind::Button => InputActionValueType::Button,
        InputActionValueKind::Axis1 => InputActionValueType::Axis1,
        InputActionValueKind::Axis2 => InputActionValueType::Axis2,
        InputActionValueKind::Pointer => InputActionValueType::Pointer,
    }
}

fn ui_value_type_from_input(value_type: InputActionValueType) -> InputActionValueKind {
    match value_type {
        InputActionValueType::Button => InputActionValueKind::Button,
        InputActionValueType::Axis1 => InputActionValueKind::Axis1,
        InputActionValueType::Axis2 => InputActionValueKind::Axis2,
        InputActionValueType::Pointer => InputActionValueKind::Pointer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_input::{InputResolver, RuntimeInputEvent, RuntimeInputFrame};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn input_mapping_authoring_creates_default_model() {
        let mapping = InputMappingAuthoringService::create_default();
        let model = InputMappingAuthoringService::build_model(
            Some(Path::new("D:/Project")),
            Some("Input/input.default.json".to_string()),
            Some(&mapping),
            1,
        );

        assert_eq!(model.mapping_id.as_deref(), Some("input.default"));
        assert_eq!(
            model.report.validation_status,
            InputMappingValidationStatus::Ok
        );
        assert!(model.actions.is_empty());
        assert!(model.bindings.is_empty());
    }

    #[test]
    fn input_mapping_authoring_adds_action_and_binding_that_resolves() {
        let mut mapping = InputMappingAuthoringService::create_default();
        InputMappingAuthoringService::apply(
            &mut mapping,
            InputMappingEditCommand::AddAction {
                action_id: "action.test".to_string(),
                value_type: InputActionValueKind::Button,
            },
        )
        .unwrap();
        InputMappingAuthoringService::apply(
            &mut mapping,
            InputMappingEditCommand::AddBinding {
                context_id: "gameplay".to_string(),
                action_id: "action.test".to_string(),
                device_path: "keyboard/T".to_string(),
            },
        )
        .unwrap();

        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame.events.push(RuntimeInputEvent::KeyDown {
            key: "T".to_string(),
        });
        let result = InputResolver::resolve(&frame, &mapping);

        assert!(result
            .action_snapshot
            .actions
            .iter()
            .any(|action| action.action_id == "action.test"));
    }

    #[test]
    fn input_mapping_authoring_saves_and_reloads_mapping() {
        let root = temp_project();
        let mapping = InputMappingAuthoringService::create_default();

        InputMappingAuthoringService::save(&root, "Input/input.default.json", &mapping).unwrap();
        let loaded = InputMappingAuthoringService::load(&root, "Input/input.default.json").unwrap();

        assert_eq!(loaded.asset_id, "input.default");
        assert_eq!(
            scan_input_mapping_paths(&root),
            vec!["Input/input.default.json".to_string()]
        );
    }

    #[test]
    fn input_mapping_working_copy_saves_only_on_explicit_commit() {
        let root = temp_project();
        let path = "Input/input.default.json";
        InputMappingAuthoringService::save(
            &root,
            path,
            &InputMappingAuthoringService::create_default(),
        )
        .unwrap();
        let mut state = InputMappingAuthoringService::open_editor_state(&root, path).unwrap();

        InputMappingAuthoringService::apply(
            &mut state.draft_mapping,
            InputMappingEditCommand::AddAction {
                action_id: "action.draft".to_string(),
                value_type: InputActionValueKind::Button,
            },
        )
        .unwrap();
        state.dirty = true;

        let before_save = InputMappingAuthoringService::load(&root, path).unwrap();
        assert!(!before_save
            .actions
            .iter()
            .any(|action| action.id == "action.draft"));

        InputMappingAuthoringService::save_editor_state(&root, &mut state).unwrap();
        let after_save = InputMappingAuthoringService::load(&root, path).unwrap();
        assert!(after_save
            .actions
            .iter()
            .any(|action| action.id == "action.draft"));
        assert!(!state.dirty);
    }

    #[test]
    fn input_mapping_working_copy_detects_external_source_change() {
        let root = temp_project();
        let path = "Input/input.default.json";
        InputMappingAuthoringService::save(
            &root,
            path,
            &InputMappingAuthoringService::create_default(),
        )
        .unwrap();
        let mut state = InputMappingAuthoringService::open_editor_state(&root, path).unwrap();
        state.dirty = true;
        fs::write(root.join(path), "{\"externally_modified\":true}").unwrap();

        let error = InputMappingAuthoringService::save_editor_state(&root, &mut state)
            .expect_err("stale source must be rejected");

        assert!(error.starts_with("input_mapping.stale_source_hash"));
        assert_eq!(
            fs::read_to_string(root.join(path)).unwrap(),
            "{\"externally_modified\":true}"
        );
    }

    #[test]
    fn input_mapping_full_field_commands_use_stable_binding_identity() {
        let mut mapping = InputMappingAuthoringService::create_default();
        InputMappingAuthoringService::apply(
            &mut mapping,
            InputMappingEditCommand::AddContext {
                context_id: "menu".to_string(),
                priority: 10,
            },
        )
        .unwrap();
        InputMappingAuthoringService::apply(
            &mut mapping,
            InputMappingEditCommand::SetContextConsumeInput {
                context_id: "menu".to_string(),
                consume_input: true,
            },
        )
        .unwrap();
        InputMappingAuthoringService::apply(
            &mut mapping,
            InputMappingEditCommand::AddBinding {
                context_id: "menu".to_string(),
                action_id: "action.fire".to_string(),
                device_path: "keyboard/Enter".to_string(),
            },
        )
        .unwrap();
        let binding_id = mapping.bindings.last().unwrap().binding_id.clone();
        InputMappingAuthoringService::apply(
            &mut mapping,
            InputMappingEditCommand::SetBindingTrigger {
                binding_id: binding_id.clone(),
                trigger: InputTriggerKind::Released,
            },
        )
        .unwrap();
        InputMappingAuthoringService::apply(
            &mut mapping,
            InputMappingEditCommand::SetBindingProcessor {
                binding_id: binding_id.clone(),
                processor: InputProcessorKind::Invert,
            },
        )
        .unwrap();

        let menu = mapping
            .contexts
            .iter()
            .find(|context| context.id == "menu")
            .unwrap();
        let binding = mapping
            .bindings
            .iter()
            .find(|binding| binding.binding_id == binding_id)
            .unwrap();
        assert_eq!(menu.priority, 10);
        assert!(menu.consume_input);
        assert_eq!(binding.trigger, InputTriggerPreset::Released);
        assert_eq!(binding.processor, InputProcessorPreset::Invert);
    }

    #[test]
    fn input_mapping_preview_reuses_runtime_resolver_and_reports_shadowing() {
        let mapping = InputMappingAsset::new(
            "input.preview",
            vec![
                InputActionDefinition::new("action.high", InputActionValueType::Button),
                InputActionDefinition::new("action.low", InputActionValueType::Button),
            ],
            vec![
                InputContextDefinition::new("high", 10).with_consume_input(true),
                InputContextDefinition::new("low", 0),
            ],
            vec![
                InputBindingDefinition::new("high", "action.high", "keyboard/Space"),
                InputBindingDefinition::new("low", "action.low", "keyboard/Space"),
            ],
        );

        let preview = InputMappingAuthoringService::preview(&mapping, "keyboard/Space");

        assert_eq!(preview.status, InputMappingPreviewStatus::Resolved);
        assert_eq!(preview.actions.len(), 1);
        assert_eq!(preview.actions[0].action_id, "action.high");
        assert_eq!(
            preview.matched_binding_ids,
            vec![mapping.bindings[0].binding_id.clone()]
        );
        assert_eq!(
            preview.shadowed_binding_ids,
            vec![mapping.bindings[1].binding_id.clone()]
        );
    }

    #[test]
    fn input_mapping_remove_impact_scans_structured_project_assets() {
        let root = temp_project();
        fs::create_dir_all(root.join("Scenes")).unwrap();
        fs::create_dir_all(root.join("Input")).unwrap();
        fs::write(
            root.join("Scenes/Main.scene.json"),
            r#"{"component":{"action_id":"action.fire"}}"#,
        )
        .unwrap();
        fs::write(
            root.join("Input/input.default.json"),
            r#"{"action_id":"action.fire"}"#,
        )
        .unwrap();

        let references = scan_input_action_references(&root, "action.fire");

        assert_eq!(references, vec!["Scenes/Main.scene.json".to_string()]);
    }

    #[test]
    fn input_mapping_remove_impact_ignores_generated_project_metadata() {
        let root = temp_project();
        fs::create_dir_all(root.join("Library/AiCapability")).unwrap();
        fs::create_dir_all(root.join("Build/reports")).unwrap();
        fs::write(
            root.join("Library/AiCapability/tool-kernel-journal.json"),
            r#"{"receipt":{"action_id":"action.fire"}}"#,
        )
        .unwrap();
        fs::write(
            root.join("Build/reports/result.json"),
            r#"{"observedAction":"action.fire"}"#,
        )
        .unwrap();

        assert!(scan_input_action_references(&root, "action.fire").is_empty());
    }

    #[test]
    fn input_mapping_authoring_reports_invalid_binding() {
        let mapping = InputMappingAsset::new(
            "input.invalid",
            vec![InputActionDefinition::new(
                "action.valid",
                InputActionValueType::Button,
            )],
            vec![InputContextDefinition::new("gameplay", 0)],
            vec![InputBindingDefinition::new(
                "gameplay",
                "action.missing",
                "device/Unknown",
            )],
        );
        let model = InputMappingAuthoringService::build_model(
            None,
            Some("Input/input.invalid.json".to_string()),
            Some(&mapping),
            1,
        );

        assert_eq!(
            model.report.validation_status,
            InputMappingValidationStatus::Error
        );
        assert!(model
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "input_mapping.unknown_action"));
        assert!(model
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "input_mapping.unsupported_device_path"));
    }

    fn temp_project() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("input-mapping-authoring-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
