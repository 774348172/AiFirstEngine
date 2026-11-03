use super::*;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq)]
pub struct SourceEvent {
    pub time_us: u64,
    pub source_ordinal: u32,
    pub name: String,
    pub int_value: i32,
    pub float_value: f32,
    pub string_value: String,
    pub volume: f32,
    pub balance: f32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SourceAnimation {
    pub name: String,
    pub duration_us: u64,
    pub events: Vec<SourceEvent>,
}
/// Input for the EXISTING asset/Cook owner. No path access, package assembly or
/// GPU ownership lives in this module.
pub struct DeclaredSource {
    pub asset_id: String,
    pub pixels_per_unit: f32,
    pub source: ValidatedSource,
}
impl DeclaredSource {
    /// Cook identity includes the complete AssetRef texture closure, target
    /// capability, schema/cooker version, pinned backend and import scale.
    pub fn cook_key(
        &self,
        textures: &BTreeMap<String, String>,
        target_capability: &str,
    ) -> Result<String> {
        const BACKEND: &[u8] = b"spine-cpp:4.2:e7dc1435fa4a0083ab431f1b28e083c14a1f5c68";
        if target_capability != "animator.spine.4.2.v1" {
            return unsupported("/targetCapability");
        }
        let ids: BTreeSet<_> = self
            .source
            .pages
            .iter()
            .map(|p| p.texture_asset_id.as_str())
            .collect();
        if textures.len() != ids.len() || ids.iter().any(|id| !textures.contains_key(*id)) {
            return fail("/textures", "texture digest closure differs from AssetRefs");
        }
        let mut texture_identity = Vec::new();
        for page in &self.source.pages {
            for field in [&page.name, &page.texture_asset_id] {
                texture_identity.extend_from_slice(&(field.len() as u64).to_le_bytes());
                texture_identity.extend_from_slice(field.as_bytes());
            }
        }
        for (id, digest) in textures {
            if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
                return fail("/textures/digest", "expected SHA256");
            }
            texture_identity.extend_from_slice(&(id.len() as u64).to_le_bytes());
            texture_identity.extend_from_slice(id.as_bytes());
            texture_identity.extend_from_slice(digest.to_ascii_lowercase().as_bytes());
        }
        Ok(digest_parts(&[
            b"spine-skeleton-asset.v1:cooker374-b1",
            BACKEND,
            self.asset_id.as_bytes(),
            self.source.source_digest.as_bytes(),
            &self.pixels_per_unit.to_bits().to_le_bytes(),
            target_capability.as_bytes(),
            &texture_identity,
        ]))
    }
    /// Stable subasset identity, independent of payload edits and source names
    /// containing separators. Length-delimited fields prevent ambiguous keys.
    pub fn event_id(&self, animation: &str, source_ordinal: u32) -> String {
        source_event_id(&self.asset_id, animation, source_ordinal)
    }
}
/// Shared by Cook and the typed control table; payload edits preserve identity.
pub fn source_event_id(asset_id: &str, animation: &str, source_ordinal: u32) -> String {
    digest_parts(&[
        asset_id.as_bytes(),
        b"animation-event.v1",
        animation.as_bytes(),
        &source_ordinal.to_le_bytes(),
    ])
}
pub(super) fn digest_parts(parts: &[&[u8]]) -> String {
    let mut h = Sha256::new();
    for part in parts {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part);
    }
    format!("{:x}", h.finalize())
}
pub(super) fn check_expansion(
    v: &Value,
    attachments: &Attachments,
    expanded: &mut usize,
) -> Result<()> {
    // Native expands sparse/empty deform keys to the FULL vertex stream.
    if let Some(skins) = v.get("attachments").and_then(Value::as_object) {
        for (skin, slots) in skins {
            for (slot, items) in slots.as_object().unwrap() {
                for (an, timelines) in items.as_object().unwrap() {
                    if let Some(keys) = timelines.get("deform").and_then(Value::as_array) {
                        *expanded = expanded.saturating_add(
                            keys.len()
                                .saturating_mul(
                                    attachments[&(skin.clone(), slot.clone(), an.clone())].deform,
                                )
                                .saturating_mul(4),
                        );
                    }
                }
            }
        }
    }
    *expanded = expanded.saturating_add(timeline_storage(v));
    if *expanded > 64 * 1024 * 1024 {
        return fail(
            "/animations",
            "native expanded timeline admission budget exceeds 64MiB",
        );
    }
    Ok(())
}
fn timeline_storage(v: &Value) -> usize {
    match v {
        Value::Object(m) => m
            .values()
            .map(timeline_storage)
            .fold(0usize, usize::saturating_add),
        Value::Array(a) if a.first().is_some_and(Value::is_object) => a.len().saturating_mul(1024),
        _ => 0,
    }
}
fn max_key_time(v: &Value) -> f64 {
    match v {
        Value::Object(m) => m.values().map(max_key_time).fold(0.0, f64::max),
        Value::Array(a) if a.first().is_some_and(Value::is_object) => a
            .iter()
            .filter_map(|f| f.get("time").and_then(Value::as_f64))
            .fold(0.0, f64::max),
        _ => 0.0,
    }
}
pub(super) fn animation_metadata(
    n: &str,
    v: &Value,
    defaults: Option<&Value>,
) -> Result<SourceAnimation> {
    let duration_us = (max_key_time(v) * 1_000_000.0).round() as u64;
    let mut events = Vec::new();
    if let Some(keys) = v.get("events").and_then(Value::as_array) {
        if duration_us == 0 {
            return fail("/animation/events", "zero-duration animation with events");
        }
        for (ordinal, k) in keys.iter().enumerate() {
            let name = k["name"].as_str().unwrap();
            let default = defaults.and_then(|m| m.get(name));
            let value = |key: &str| k.get(key).or_else(|| default.and_then(|d| d.get(key)));
            events.push(SourceEvent {
                time_us: (k.get("time").and_then(Value::as_f64).unwrap_or(0.0) * 1_000_000.0)
                    .round() as u64,
                source_ordinal: ordinal as u32,
                name: name.into(),
                int_value: value("int").and_then(Value::as_i64).unwrap_or(0) as i32,
                float_value: value("float").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                string_value: value("string").and_then(Value::as_str).unwrap_or("").into(),
                volume: value("volume").and_then(Value::as_f64).unwrap_or(1.0) as f32,
                balance: value("balance").and_then(Value::as_f64).unwrap_or(0.0) as f32,
            });
        }
    }
    Ok(SourceAnimation {
        name: n.into(),
        duration_us,
        events,
    })
}
pub fn validate_declared(descriptor: &str, json: &str, atlas: &str) -> Result<DeclaredSource> {
    if descriptor.len() > 64 * 1024 {
        return fail("/descriptor", "descriptor exceeds 64KiB");
    }
    let Unique(v) = serde_json::from_str::<Unique>(descriptor).map_err(|e| Diagnostic {
        code: "spine.import.invalid_structure",
        path: "/descriptor".into(),
        message: e.to_string(),
    })?;
    let m = object(
        &v,
        "/descriptor",
        &[
            "schemaVersion",
            "assetId",
            "skeletonSource",
            "atlasSource",
            "pages",
            "format",
            "exportVersion",
            "pixelsPerUnit",
        ],
    )?;
    if name(m, "schemaVersion", "/descriptor")? != "spine-skeleton-asset.v1" {
        return fail("/descriptor/schemaVersion", "unknown schema");
    }
    if name(m, "format", "/descriptor")? != "json" {
        return Err(Diagnostic {
            code: "spine.import.unsupported_format",
            path: "/descriptor/format".into(),
            message: "only JSON is qualified".into(),
        });
    }
    let asset_id = name(m, "assetId", "/descriptor")?;
    if asset_id.len() > 128
        || !asset_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    {
        return fail("/descriptor/assetId", "invalid semantic asset identity");
    }
    for key in ["skeletonSource", "atlasSource"] {
        let path = name(m, key, "/descriptor")?;
        if path.len() > 256 || !safe_page(path) {
            return fail(
                &format!("/descriptor/{key}"),
                "unsafe relative project source",
            );
        }
    }
    let ppu = number(
        required(m, "pixelsPerUnit", "/descriptor")?,
        "/descriptor/pixelsPerUnit",
    )? as f32;
    if ppu <= 0.0 || ppu > 1_000_000.0 {
        return fail(
            "/descriptor/pixelsPerUnit",
            "pixelsPerUnit outside positive finite range",
        );
    }
    let page_map = dict(required(m, "pages", "/descriptor")?, "/descriptor/pages")?;
    let mut refs = BTreeMap::new();
    for (n, v) in page_map {
        let p = object(v, "/descriptor/pages/page", &["assetId"])?;
        refs.insert(
            n.clone(),
            name(p, "assetId", "/descriptor/pages/page")?.to_string(),
        );
    }
    let source = validate(json, atlas, &refs)?;
    let actual: Value = serde_json::from_str(source.cooked_json()).unwrap();
    if name(m, "exportVersion", "/descriptor")? != actual["skeleton"]["spine"].as_str().unwrap() {
        return Err(Diagnostic {
            code: "spine.import.version_mismatch",
            path: "/descriptor/exportVersion".into(),
            message: "declaration and source version differ".into(),
        });
    }
    Ok(DeclaredSource {
        asset_id: asset_id.into(),
        pixels_per_unit: ppu,
        source,
    })
}
