use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::OnceLock;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

pub const EDITOR_LOCALIZATION_CATALOG_SCHEMA_VERSION: &str = "editor-localization-catalog.v1";
pub const EDITOR_LOCALE_ZH_CN: &str = "zh-CN";
pub const EDITOR_LOCALE_EN_US: &str = "en-US";

const ZH_CN_CATALOG: &str = include_str!("../resources/localization/zh-CN.editor.json");
const EN_US_CATALOG: &str = include_str!("../resources/localization/en-US.editor.json");

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EditorLocaleId(String);

impl EditorLocaleId {
    pub fn parse(value: impl Into<String>) -> Result<Self, EditorCatalogDiagnostic> {
        let value = normalize_locale(value.into().as_str());
        if !matches!(value.as_str(), EDITOR_LOCALE_ZH_CN | EDITOR_LOCALE_EN_US) {
            return Err(EditorCatalogDiagnostic::new(
                EditorCatalogDiagnosticCode::LocaleUnsupported,
                format!("unsupported Editor locale `{value}`"),
            ));
        }
        Ok(Self(value))
    }

    pub fn zh_cn() -> Self {
        Self(EDITOR_LOCALE_ZH_CN.to_string())
    }

    pub fn en_us() -> Self {
        Self(EDITOR_LOCALE_EN_US.to_string())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Default for EditorLocaleId {
    fn default() -> Self {
        Self::zh_cn()
    }
}

fn normalize_locale(value: &str) -> String {
    match value.trim() {
        "zh_CN" | "zh-Hans" | "zh_Hans" => EDITOR_LOCALE_ZH_CN.to_string(),
        "en_US" => EDITOR_LOCALE_EN_US.to_string(),
        value => value.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EditorMessageKey(String);

impl EditorMessageKey {
    pub fn parse(value: impl Into<String>) -> Result<Self, EditorCatalogDiagnostic> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.starts_with("editor.")
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'_'
            })
            && !value.contains("..")
            && !value.ends_with('.');
        if !valid {
            return Err(EditorCatalogDiagnostic::new(
                EditorCatalogDiagnosticCode::MessageKeyInvalid,
                format!("invalid Editor message key `{value}`"),
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorMessageArgType {
    StringInvariant,
    I64,
    U64,
    F64,
    Bool,
    Path,
    StableId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum EditorMessageValue {
    StringInvariant(String),
    I64(i64),
    U64(u64),
    F64(f64),
    Bool(bool),
    Path(String),
    StableId(String),
}

impl EditorMessageValue {
    fn arg_type(&self) -> EditorMessageArgType {
        match self {
            Self::StringInvariant(_) => EditorMessageArgType::StringInvariant,
            Self::I64(_) => EditorMessageArgType::I64,
            Self::U64(_) => EditorMessageArgType::U64,
            Self::F64(_) => EditorMessageArgType::F64,
            Self::Bool(_) => EditorMessageArgType::Bool,
            Self::Path(_) => EditorMessageArgType::Path,
            Self::StableId(_) => EditorMessageArgType::StableId,
        }
    }
}

impl fmt::Display for EditorMessageValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StringInvariant(value) | Self::Path(value) | Self::StableId(value) => {
                formatter.write_str(value)
            }
            Self::I64(value) => value.fmt(formatter),
            Self::U64(value) => value.fmt(formatter),
            Self::F64(value) => value.fmt(formatter),
            Self::Bool(value) => value.fmt(formatter),
        }
    }
}

pub type EditorMessageArgs = BTreeMap<String, EditorMessageValue>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorInvariantText(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EditorTextRef {
    Message {
        key: EditorMessageKey,
        #[serde(default)]
        args: EditorMessageArgs,
    },
    Invariant(EditorInvariantText),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorCatalogDiagnosticCode {
    PreferenceMalformed,
    LocaleUnsupported,
    CatalogMissing,
    CatalogSchemaInvalid,
    CatalogDuplicateKey,
    MessageKeyInvalid,
    MessageMissing,
    ArgumentContractMismatch,
    ArgumentValueMissing,
    TemplateInvalid,
    PreferenceWriteFailed,
    SwitchRejected,
}

impl EditorCatalogDiagnosticCode {
    pub fn stable_code(&self) -> &'static str {
        match self {
            Self::PreferenceMalformed => "editor.localization.preference_malformed",
            Self::LocaleUnsupported => "editor.localization.locale_unsupported",
            Self::CatalogMissing => "editor.localization.catalog_missing",
            Self::CatalogSchemaInvalid => "editor.localization.catalog_schema_invalid",
            Self::CatalogDuplicateKey => "editor.localization.catalog_duplicate_key",
            Self::MessageKeyInvalid => "editor.localization.message_key_invalid",
            Self::MessageMissing => "editor.localization.message_missing",
            Self::ArgumentContractMismatch => "editor.localization.argument_contract_mismatch",
            Self::ArgumentValueMissing => "editor.localization.argument_value_missing",
            Self::TemplateInvalid => "editor.localization.template_invalid",
            Self::PreferenceWriteFailed => "editor.localization.preference_write_failed",
            Self::SwitchRejected => "editor.localization.switch_rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorCatalogDiagnostic {
    pub code: EditorCatalogDiagnosticCode,
    pub message: String,
    pub message_key: Option<String>,
}

impl EditorCatalogDiagnostic {
    pub fn new(code: EditorCatalogDiagnosticCode, message: String) -> Self {
        Self {
            code,
            message,
            message_key: None,
        }
    }

    fn for_key(mut self, key: &str) -> Self {
        self.message_key = Some(key.to_string());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorLocalizationMessage {
    pub text: String,
    #[serde(default)]
    pub arguments: BTreeMap<String, EditorMessageArgType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorLocalizationCatalog {
    pub schema_version: String,
    pub domain: String,
    pub locale: EditorLocaleId,
    #[serde(deserialize_with = "deserialize_messages_no_duplicates")]
    pub messages: BTreeMap<String, EditorLocalizationMessage>,
}

fn deserialize_messages_no_duplicates<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, EditorLocalizationMessage>, D::Error>
where
    D: Deserializer<'de>,
{
    struct MessagesVisitor;

    impl<'de> Visitor<'de> for MessagesVisitor {
        type Value = BTreeMap<String, EditorLocalizationMessage>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a map of unique Editor message keys")
        }

        fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut messages = BTreeMap::new();
            while let Some((key, message)) =
                access.next_entry::<String, EditorLocalizationMessage>()?
            {
                if messages.insert(key.clone(), message).is_some() {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate Editor message key `{key}`"
                    )));
                }
            }
            Ok(messages)
        }
    }

    deserializer.deserialize_map(MessagesVisitor)
}

impl EditorLocalizationCatalog {
    pub fn parse(json: &str) -> Result<Self, EditorCatalogDiagnostic> {
        let catalog: Self = serde_json::from_str(json).map_err(|error| {
            let code = if error.to_string().contains("duplicate Editor message key") {
                EditorCatalogDiagnosticCode::CatalogDuplicateKey
            } else {
                EditorCatalogDiagnosticCode::CatalogSchemaInvalid
            };
            EditorCatalogDiagnostic::new(code, format!("invalid Editor Catalog: {error}"))
        })?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), EditorCatalogDiagnostic> {
        if self.schema_version != EDITOR_LOCALIZATION_CATALOG_SCHEMA_VERSION {
            return Err(EditorCatalogDiagnostic::new(
                EditorCatalogDiagnosticCode::CatalogSchemaInvalid,
                format!("unsupported Catalog schema `{}`", self.schema_version),
            ));
        }
        if self.domain != "editor" {
            return Err(EditorCatalogDiagnostic::new(
                EditorCatalogDiagnosticCode::CatalogSchemaInvalid,
                format!("unexpected Catalog domain `{}`", self.domain),
            ));
        }
        for (key, message) in &self.messages {
            EditorMessageKey::parse(key.clone())?;
            if message.text.trim().is_empty() {
                return Err(EditorCatalogDiagnostic::new(
                    EditorCatalogDiagnosticCode::TemplateInvalid,
                    "Editor message text must not be empty".to_string(),
                )
                .for_key(key));
            }
            let placeholders = template_placeholders(message.text.as_str()).map_err(|error| {
                EditorCatalogDiagnostic::new(EditorCatalogDiagnosticCode::TemplateInvalid, error)
                    .for_key(key)
            })?;
            let arguments = message.arguments.keys().cloned().collect::<BTreeSet<_>>();
            if placeholders != arguments {
                return Err(EditorCatalogDiagnostic::new(
                    EditorCatalogDiagnosticCode::ArgumentContractMismatch,
                    format!(
                        "placeholder/argument mismatch: placeholders={placeholders:?}, arguments={arguments:?}"
                    ),
                )
                .for_key(key));
            }
        }
        Ok(())
    }
}

fn template_placeholders(text: &str) -> Result<BTreeSet<String>, String> {
    let mut placeholders = BTreeSet::new();
    let mut remainder = text;
    while let Some(open) = remainder.find('{') {
        let before = &remainder[..open];
        if before.contains('}') {
            return Err("unmatched closing brace".to_string());
        }
        let after_open = &remainder[open + 1..];
        let Some(close) = after_open.find('}') else {
            return Err("unmatched opening brace".to_string());
        };
        let name = &after_open[..close];
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(format!("invalid placeholder `{name}`"));
        }
        placeholders.insert(name.to_string());
        remainder = &after_open[close + 1..];
    }
    if remainder.contains('}') {
        return Err("unmatched closing brace".to_string());
    }
    Ok(placeholders)
}

#[derive(Debug, Clone)]
pub struct EditorLocalizationBundle {
    catalogs: BTreeMap<EditorLocaleId, EditorLocalizationCatalog>,
    native_keys: BTreeMap<String, EditorMessageKey>,
}

impl EditorLocalizationBundle {
    pub fn from_catalogs(
        catalogs: impl IntoIterator<Item = EditorLocalizationCatalog>,
    ) -> Result<Self, EditorCatalogDiagnostic> {
        let mut by_locale = BTreeMap::new();
        for catalog in catalogs {
            let locale = catalog.locale.clone();
            if by_locale.insert(locale.clone(), catalog).is_some() {
                return Err(EditorCatalogDiagnostic::new(
                    EditorCatalogDiagnosticCode::CatalogSchemaInvalid,
                    format!("duplicate Catalog locale `{}`", locale.as_str()),
                ));
            }
        }
        let fallback = by_locale.get(&EditorLocaleId::en_us()).ok_or_else(|| {
            EditorCatalogDiagnostic::new(
                EditorCatalogDiagnosticCode::CatalogMissing,
                "missing required en-US Editor Catalog".to_string(),
            )
        })?;
        let default = by_locale.get(&EditorLocaleId::zh_cn()).ok_or_else(|| {
            EditorCatalogDiagnostic::new(
                EditorCatalogDiagnosticCode::CatalogMissing,
                "missing required zh-CN Editor Catalog".to_string(),
            )
        })?;
        if fallback.messages.keys().ne(default.messages.keys()) {
            return Err(EditorCatalogDiagnostic::new(
                EditorCatalogDiagnosticCode::ArgumentContractMismatch,
                "zh-CN/en-US Editor Catalog key sets differ".to_string(),
            ));
        }
        for (key, fallback_message) in &fallback.messages {
            let default_message = &default.messages[key];
            if fallback_message.arguments != default_message.arguments {
                return Err(EditorCatalogDiagnostic::new(
                    EditorCatalogDiagnosticCode::ArgumentContractMismatch,
                    "zh-CN/en-US Editor Catalog argument contracts differ".to_string(),
                )
                .for_key(key));
            }
        }
        let mut native_keys = BTreeMap::new();
        for (key, message) in &fallback.messages {
            if message.arguments.is_empty() {
                let parsed = EditorMessageKey::parse(key.clone())?;
                native_keys.entry(message.text.clone()).or_insert(parsed);
            }
        }
        Ok(Self {
            catalogs: by_locale,
            native_keys,
        })
    }

    pub fn available_locales(&self) -> Vec<EditorLocaleDescriptor> {
        self.catalogs
            .keys()
            .map(|locale| EditorLocaleDescriptor {
                locale: locale.clone(),
                self_name: match locale.as_str() {
                    EDITOR_LOCALE_ZH_CN => "简体中文（zh-CN）",
                    _ => "English (en-US)",
                }
                .to_string(),
            })
            .collect()
    }

    pub fn snapshot(
        &self,
        locale: EditorLocaleId,
        revision: u64,
    ) -> Result<EditorLocalizationSnapshot, EditorCatalogDiagnostic> {
        if !self.catalogs.contains_key(&locale) {
            return Err(EditorCatalogDiagnostic::new(
                EditorCatalogDiagnosticCode::LocaleUnsupported,
                format!("unsupported Editor locale `{}`", locale.as_str()),
            ));
        }
        Ok(EditorLocalizationSnapshot { locale, revision })
    }

    fn resolve(
        &self,
        snapshot: &EditorLocalizationSnapshot,
        key: &EditorMessageKey,
        args: &EditorMessageArgs,
    ) -> Result<String, EditorCatalogDiagnostic> {
        let active = self.catalogs.get(&snapshot.locale);
        let fallback = self.catalogs.get(&EditorLocaleId::en_us());
        let message = active
            .and_then(|catalog| catalog.messages.get(key.as_str()))
            .or_else(|| fallback.and_then(|catalog| catalog.messages.get(key.as_str())))
            .ok_or_else(|| {
                EditorCatalogDiagnostic::new(
                    EditorCatalogDiagnosticCode::MessageMissing,
                    format!("missing Editor message `{}`", key.as_str()),
                )
                .for_key(key.as_str())
            })?;
        if args.len() != message.arguments.len() {
            return Err(EditorCatalogDiagnostic::new(
                EditorCatalogDiagnosticCode::ArgumentValueMissing,
                format!("wrong argument count for `{}`", key.as_str()),
            )
            .for_key(key.as_str()));
        }
        for (name, expected_type) in &message.arguments {
            let value = args.get(name).ok_or_else(|| {
                EditorCatalogDiagnostic::new(
                    EditorCatalogDiagnosticCode::ArgumentValueMissing,
                    format!("missing argument `{name}`"),
                )
                .for_key(key.as_str())
            })?;
            if value.arg_type() != *expected_type {
                return Err(EditorCatalogDiagnostic::new(
                    EditorCatalogDiagnosticCode::ArgumentContractMismatch,
                    format!("argument `{name}` has the wrong type"),
                )
                .for_key(key.as_str()));
            }
        }
        let mut resolved = message.text.clone();
        for (name, value) in args {
            resolved = resolved.replace(format!("{{{name}}}").as_str(), value.to_string().as_str());
        }
        Ok(resolved)
    }

    pub fn resolve_native_exact(
        &self,
        snapshot: &EditorLocalizationSnapshot,
        native_text: &str,
    ) -> Option<String> {
        let key = self.native_keys.get(native_text)?;
        self.resolve(snapshot, key, &EditorMessageArgs::new()).ok()
    }
}

pub fn trusted_editor_localization_bundle() -> &'static EditorLocalizationBundle {
    static BUNDLE: OnceLock<EditorLocalizationBundle> = OnceLock::new();
    BUNDLE.get_or_init(|| {
        let zh = EditorLocalizationCatalog::parse(ZH_CN_CATALOG)
            .expect("packaged zh-CN Editor Catalog must be valid");
        let en = EditorLocalizationCatalog::parse(EN_US_CATALOG)
            .expect("packaged en-US Editor Catalog must be valid");
        EditorLocalizationBundle::from_catalogs([zh, en])
            .expect("packaged Editor Catalogs must have matching contracts")
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorLocaleDescriptor {
    pub locale: EditorLocaleId,
    pub self_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorLocalizationSnapshot {
    pub locale: EditorLocaleId,
    pub revision: u64,
}

impl Default for EditorLocalizationSnapshot {
    fn default() -> Self {
        Self {
            locale: EditorLocaleId::zh_cn(),
            revision: 0,
        }
    }
}

impl EditorLocalizationSnapshot {
    pub fn resolve(
        &self,
        key: &EditorMessageKey,
        args: &EditorMessageArgs,
    ) -> Result<String, EditorCatalogDiagnostic> {
        trusted_editor_localization_bundle().resolve(self, key, args)
    }

    pub fn text(&self, key: &str) -> String {
        let Ok(key) = EditorMessageKey::parse(key.to_string()) else {
            return "Unavailable text".to_string();
        };
        self.resolve(&key, &EditorMessageArgs::new())
            .unwrap_or_else(|_| "Unavailable text".to_string())
    }

    pub fn localize_native_exact(&self, native_text: &str) -> Option<String> {
        trusted_editor_localization_bundle().resolve_native_exact(self, native_text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorLocaleChangeResult {
    pub changed: bool,
    pub snapshot: EditorLocalizationSnapshot,
    pub diagnostic: Option<EditorCatalogDiagnostic>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localization_defaults_to_simplified_chinese() {
        let snapshot = EditorLocalizationSnapshot::default();
        assert_eq!(snapshot.locale.as_str(), EDITOR_LOCALE_ZH_CN);
        assert_eq!(snapshot.text("editor.launcher.open_project"), "打开项目");
    }

    #[test]
    fn localization_resolves_english_and_typed_named_arguments() {
        let snapshot = trusted_editor_localization_bundle()
            .snapshot(EditorLocaleId::en_us(), 4)
            .unwrap();
        let key = EditorMessageKey::parse("editor.assets.selected_count").unwrap();
        let args = BTreeMap::from([("count".to_string(), EditorMessageValue::U64(3))]);
        assert_eq!(snapshot.resolve(&key, &args).unwrap(), "3 assets selected");
        assert_eq!(snapshot.revision, 4);
    }

    #[test]
    fn localization_rejects_argument_type_mismatch() {
        let snapshot = EditorLocalizationSnapshot::default();
        let key = EditorMessageKey::parse("editor.assets.selected_count").unwrap();
        let args = BTreeMap::from([(
            "count".to_string(),
            EditorMessageValue::StringInvariant("three".to_string()),
        )]);
        let diagnostic = snapshot.resolve(&key, &args).unwrap_err();
        assert_eq!(
            diagnostic.code,
            EditorCatalogDiagnosticCode::ArgumentContractMismatch
        );
    }

    #[test]
    fn localization_rejects_duplicate_message_keys() {
        let json = r#"{
            "schemaVersion":"editor-localization-catalog.v1",
            "domain":"editor",
            "locale":"zh-CN",
            "messages":{
                "editor.test.key":{"text":"一","arguments":{}},
                "editor.test.key":{"text":"二","arguments":{}}
            }
        }"#;
        let diagnostic = EditorLocalizationCatalog::parse(json).unwrap_err();
        assert_eq!(
            diagnostic.code,
            EditorCatalogDiagnosticCode::CatalogDuplicateKey
        );
    }

    #[test]
    fn localization_rejects_catalog_contract_drift() {
        let zh = EditorLocalizationCatalog::parse(ZH_CN_CATALOG).unwrap();
        let mut en = EditorLocalizationCatalog::parse(EN_US_CATALOG).unwrap();
        en.messages.remove("editor.launcher.open_project");
        let diagnostic = EditorLocalizationBundle::from_catalogs([zh, en]).unwrap_err();
        assert_eq!(
            diagnostic.code,
            EditorCatalogDiagnosticCode::ArgumentContractMismatch
        );
    }

    #[test]
    fn localization_normalizes_supported_locale_aliases() {
        assert_eq!(
            EditorLocaleId::parse("zh_Hans").unwrap().as_str(),
            EDITOR_LOCALE_ZH_CN
        );
        assert!(EditorLocaleId::parse("fr-FR").is_err());
    }

    #[test]
    fn production_native_text_inventory_is_cataloged_or_explicitly_invariant() {
        let localized = [
            "Projects",
            "Open Project",
            "Create Project",
            "Create with AI",
            "New project",
            "Search...",
            "What would you like to make?",
            "Add to draft",
            "No recent projects. Open or create a project to begin.",
            "NAME",
            "MODIFIED",
            "ENGINE VERSION",
            "Window",
            "Language",
            "Reset Layout",
            "Workspace",
            "Close Tab",
            "Build Export",
            "No export report yet.",
            "Export",
            "Build & Run",
            "Build Release",
            "Pick Icon",
            "Save Profile",
            "Output",
            "Report",
            "AI Panel",
            "Describe an editor change...",
            "Display 1    16:9 Landscape    Scale 1x    Play Focused    Stats    Gizmos",
            "Display 1",
            "Contain",
            "Stretch",
        ];
        let snapshot = EditorLocalizationSnapshot::default();
        for native_text in localized {
            assert!(
                snapshot.localize_native_exact(native_text).is_some(),
                "production Editor text is missing from the Catalog: {native_text}"
            );
        }

        let invariants = ["AI First Engine", "简体中文（zh-CN）", "English (en-US)"];
        for invariant in invariants {
            assert!(snapshot.localize_native_exact(invariant).is_none());
        }
    }
}
