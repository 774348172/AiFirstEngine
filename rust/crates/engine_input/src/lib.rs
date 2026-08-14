pub mod device_state;
pub mod input_action;
pub mod input_mapping;

pub use device_state::{
    InputDeviceKind, InputDeviceState, InputDeviceStateReport, RawInputEvent, RawInputEventKind,
    RawInputValue,
};
pub use input_action::{
    ActionPhase, ActionSnapshot, ActionValue, Axis1, Axis2, InputActionState, InputTraceSummary,
    PointerPosition,
};
pub use input_mapping::{
    InputActionDefinition, InputActionValueType, InputBindingDefinition, InputContextDefinition,
    InputControlCatalog, InputControlCatalogEntry, InputControlDeviceKind, InputDiagnosticSeverity,
    InputMappingAsset, InputMappingDiagnostic, InputMappingReport, InputProcessorPreset,
    InputResolveResult, InputResolver, InputTriggerPreset, PlatformInputOverride,
    RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton, INPUT_MAPPING_SCHEMA_VERSION,
    LEGACY_INPUT_MAPPING_SCHEMA_VERSION,
};

pub const ENGINE_INPUT_NAME: &str = "engine_input";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_input_smoke_test_passes() {
        assert_eq!(ENGINE_INPUT_NAME, "engine_input");
        let mapping = InputMappingAsset::gameplay_default();
        assert_eq!(mapping.asset_id, "input.default");
    }
}
