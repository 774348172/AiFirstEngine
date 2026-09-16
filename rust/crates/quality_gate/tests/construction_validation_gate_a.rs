use quality_gate::{
    parse_validation_catalog, ConstructionValidationModule, EmptyEvidenceStore, PrepareRequest,
    StaticValidationCatalogSource, ValidationCatalog,
};

#[test]
fn schema_prepare_request_rejects_unknown_fields() {
    let source = r#"{
        "schemaVersion":"construction-validation.prepare-request.v2",
        "requestId":"gate-a",
        "claim":"Development",
        "change":{"changedSubjects":[],"identities":{}},
        "declaredOwners":["quality_gate"],
        "declaredConsumers":[],
        "requiredCapabilities":[],
        "environment":{"platform":"windows-x86_64","profile":"debug","features":[],"composition":"source"},
        "authorizationCeiling":{"localCi":false,"realEditor":false,"productionReplacement":false,"realConfigurationMutation":false},
        "timeBudgetSeconds":300,
        "unexpected":true
    }"#;
    assert!(serde_json::from_str::<PrepareRequest>(source).is_err());
}

#[test]
fn schema_catalog_rejects_unknown_producer() {
    let source = r#"{
        "schemaVersion":"construction-validation.catalog.v1",
        "verifiers":[{
            "id":"bad",
            "ownerDomains":["quality_gate"],
            "consumerDomains":[],
            "proves":["development.owner"],
            "producerId":"arbitrary.shell",
            "environment":{},
            "subsumes":[],
            "costClass":"low",
            "defaultTimeoutSeconds":10,
            "historicalDurationSeconds":null,
            "cleanupReserveSeconds":0,
            "externalEffects":[],
            "requiredAuthorization":[],
            "consumedIdentityKinds":["product_source"]
        }]
    }"#;
    assert!(parse_validation_catalog(source.as_bytes()).is_err());
}

#[test]
fn plan_only_module_exposes_prepare() {
    let catalog: ValidationCatalog = parse_validation_catalog(
        br#"{"schemaVersion":"construction-validation.catalog.v1","verifiers":[]}"#,
    )
    .expect("empty catalog is structurally valid");
    let module = ConstructionValidationModule::new(
        StaticValidationCatalogSource::new(catalog),
        EmptyEvidenceStore,
    );
    let _prepare =
        ConstructionValidationModule::<StaticValidationCatalogSource, EmptyEvidenceStore>::prepare;
    drop(module);
}
