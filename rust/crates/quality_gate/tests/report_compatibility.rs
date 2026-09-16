use quality_gate::report::{
    read_quality_gate_report, QualityGateReport, QUALITY_GATE_REPORT_SCHEMA_VERSION,
    QUALITY_GATE_REPORT_V1_SCHEMA_VERSION,
};

#[test]
fn v1_fixture_remains_readable() {
    let mut value = serde_json::to_value(QualityGateReport::default()).unwrap();
    value["schema_version"] =
        serde_json::Value::String(QUALITY_GATE_REPORT_V1_SCHEMA_VERSION.to_string());
    value.as_object_mut().unwrap().remove("architecture");
    value
        .as_object_mut()
        .unwrap()
        .remove("architecture_diagnostics");
    let bytes = serde_json::to_vec(&value).unwrap();
    let report = read_quality_gate_report(&bytes).unwrap();
    assert_eq!(report.schema_version, QUALITY_GATE_REPORT_V1_SCHEMA_VERSION);
    assert_eq!(report.architecture.final_decision, "");
}

#[test]
fn v2_writer_uses_honest_schema() {
    let report = QualityGateReport::default();
    assert_eq!(report.schema_version, QUALITY_GATE_REPORT_SCHEMA_VERSION);
    assert_eq!(report.schema_version, "quality-gate-report.v2");
}

#[test]
fn unknown_required_semantics_fail_closed() {
    let source = br#"{"schema_version":"quality-gate-report.v9"}"#;
    assert!(read_quality_gate_report(source).is_err());
}
