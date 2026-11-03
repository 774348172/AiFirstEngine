//! Strict, no-I/O admission before the pinned C++ parser. No unchecked JSON is
//! accepted by the safe API. Physics, two-color, sequence and unknown types fail
//! explicitly. Setup/timeline constraint checks live in `import/constraints.rs`.
use crate::Asset;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
mod constraints;
mod metadata;
mod texture;
pub use metadata::{
    DeclaredSource, SourceAnimation, SourceEvent, source_event_id, validate_declared,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}
type Result<T> = std::result::Result<T, Diagnostic>;
fn fail<T>(path: &str, message: &str) -> Result<T> {
    Err(Diagnostic {
        code: "spine.import.invalid_structure",
        path: path.into(),
        message: message.into(),
    })
}
fn unsupported<T>(path: &str) -> Result<T> {
    Err(Diagnostic {
        code: "spine.import.unsupported_feature",
        path: path.into(),
        message: "feature not qualified in this backend admission profile".into(),
    })
}

// serde_json::Value normally overwrites duplicate object keys. Reject them at
// every depth, including event payloads and animation/attachment names.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("finite JSON without duplicate keys")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Unique, E> {
                if i32::try_from(v).is_err() {
                    return Err(E::custom("number exceeds native parser integer domain"));
                }
                Ok(Unique(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Unique, E> {
                if i32::try_from(v).is_err() {
                    return Err(E::custom("number exceeds native parser integer domain"));
                }
                Ok(Unique(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Unique, E> {
                // Official Json::parseNumber casts EVERY numeric token to int,
                // including values later consumed as floats. Avoid C++ UB.
                if !(i32::MIN as f64..=i32::MAX as f64).contains(&v) {
                    return Err(E::custom("number exceeds native parser integer domain"));
                }
                serde_json::Number::from_f64(v)
                    .map(|n| Unique(Value::Number(n)))
                    .ok_or_else(|| E::custom("nonfinite number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Unique, E> {
                if v.contains('\0') {
                    return Err(E::custom("embedded NUL"));
                }
                Ok(Unique(v.into()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Unique, E> {
                self.visit_str(&v)
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> std::result::Result<Unique, E> {
                self.visit_unit()
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut v = Vec::new();
                while let Some(Unique(x)) = a.next_element()? {
                    v.push(x);
                }
                Ok(Unique(Value::Array(v)))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut m = Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if k.contains('\0') || m.contains_key(&k) {
                        return Err(de::Error::custom("duplicate key or embedded NUL"));
                    }
                    let Unique(v) = a.next_value()?;
                    m.insert(k, v);
                }
                Ok(Unique(Value::Object(m)))
            }
        }
        d.deserialize_any(V)
    }
}
fn object<'a>(v: &'a Value, p: &str, allowed: &[&str]) -> Result<&'a Map<String, Value>> {
    let m = v.as_object().ok_or_else(|| Diagnostic {
        code: "spine.import.invalid_structure",
        path: p.into(),
        message: "expected object".into(),
    })?;
    for k in m.keys() {
        if !allowed.contains(&k.as_str()) {
            return fail(&format!("{p}/{k}"), "unknown field");
        }
    }
    Ok(m)
}
fn dict<'a>(v: &'a Value, p: &str) -> Result<&'a Map<String, Value>> {
    v.as_object().ok_or_else(|| Diagnostic {
        code: "spine.import.invalid_structure",
        path: p.into(),
        message: "expected named object".into(),
    })
}
fn array<'a>(v: &'a Value, p: &str, max: usize) -> Result<&'a Vec<Value>> {
    match v.as_array() {
        Some(a) if a.len() <= max => Ok(a),
        _ => fail(p, "expected bounded array"),
    }
}
fn string<'a>(v: &'a Value, p: &str) -> Result<&'a str> {
    match v.as_str() {
        Some(s) if !s.is_empty() && s.len() <= 1024 => Ok(s),
        _ => fail(p, "expected nonempty name <=1024 bytes"),
    }
}
fn number(v: &Value, p: &str) -> Result<f64> {
    match v.as_f64() {
        Some(x) if (x as f32).is_finite() => Ok(x),
        _ => fail(p, "expected finite f32-compatible number"),
    }
}
fn integer(v: &Value, p: &str, max: usize) -> Result<usize> {
    match v.as_u64() {
        Some(x) if x <= max as u64 => Ok(x as usize),
        _ => fail(p, "expected bounded unsigned integer"),
    }
}
fn required<'a>(m: &'a Map<String, Value>, k: &str, p: &str) -> Result<&'a Value> {
    m.get(k).ok_or_else(|| Diagnostic {
        code: "spine.import.invalid_structure",
        path: format!("{p}/{k}"),
        message: "missing required field".into(),
    })
}
fn name<'a>(m: &'a Map<String, Value>, k: &str, p: &str) -> Result<&'a str> {
    string(required(m, k, p)?, p)
}
fn numeric_fields(m: &Map<String, Value>, keys: &[&str], p: &str) -> Result<()> {
    for k in keys {
        if let Some(v) = m.get(*k) {
            number(v, &format!("{p}/{k}"))?;
        }
    }
    Ok(())
}
fn bool_fields(m: &Map<String, Value>, keys: &[&str], p: &str) -> Result<()> {
    for k in keys {
        if let Some(v) = m.get(*k)
            && !v.is_boolean()
        {
            return fail(p, "expected boolean");
        }
    }
    Ok(())
}
fn color(m: &Map<String, Value>, key: &str, p: &str, digits: usize) -> Result<()> {
    if let Some(v) = m.get(key) {
        let s = string(v, p)?;
        if s.len() != digits || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return fail(p, "invalid hex color");
        }
    }
    Ok(())
}
fn member(names: &BTreeSet<String>, v: &Value, p: &str) -> Result<()> {
    if !names.contains(string(v, p)?) {
        return fail(p, "unknown reference");
    }
    Ok(())
}
fn choice(m: &Map<String, Value>, key: &str, choices: &[&str], p: &str) -> Result<()> {
    if let Some(v) = m.get(key)
        && !choices.contains(&string(v, p)?)
    {
        return fail(p, "unknown enum value");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub name: String,
    pub texture_asset_id: String,
    pub pma: bool,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone)]
struct Atlas {
    pages: Vec<Page>,
    regions: BTreeSet<String>,
    canonical: String,
}
fn safe_page(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 1024
        && !s.contains(['\\', ':', '\0'])
        && !s.starts_with('/')
        && s.split('/').all(|c| !c.is_empty() && c != "." && c != "..")
}
fn ints(s: &str, n: usize, p: &str) -> Result<Vec<i32>> {
    let v = s
        .split(',')
        .map(|x| x.trim().parse::<i32>())
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| Diagnostic {
            code: "spine.import.invalid_structure",
            path: p.into(),
            message: "invalid atlas integer tuple".into(),
        })?;
    if v.len() != n {
        return fail(p, "wrong tuple length");
    }
    Ok(v)
}
fn atlas(source: &str, bindings: &BTreeMap<String, String>) -> Result<Atlas> {
    if source.len() > 1024 * 1024 || source.contains('\0') {
        return fail("/atlas", "invalid size or NUL");
    }
    let lines: Vec<_> = source.lines().map(str::trim).collect();
    let mut i = 0;
    let mut pages = Vec::new();
    let mut regions = BTreeSet::new();
    let mut canonical = String::new();
    let mut decoded = 0u64;
    while i < lines.len() {
        if lines[i].is_empty() {
            i += 1;
            continue;
        }
        let page_name = lines[i];
        i += 1;
        if !safe_page(page_name)
            || pages.iter().any(|p: &Page| p.name == page_name)
            || pages.len() == 8
        {
            return fail("/atlas/page", "unsafe, duplicate or excessive page");
        }
        let mut fields = BTreeMap::new();
        while i < lines.len() && lines[i].contains(':') {
            let (k, v) = lines[i].split_once(':').unwrap();
            i += 1;
            if !["size", "format", "filter", "repeat", "pma"].contains(&k)
                || fields.insert(k, v.trim()).is_some()
            {
                return fail("/atlas/page", "unknown or duplicate page field");
            }
        }
        let size = ints(fields.get("size").copied().unwrap_or(""), 2, "/atlas/size")?;
        if size.iter().any(|n| *n < 1 || *n > 4096) {
            return fail("/atlas/size", "page outside 1..4096");
        }
        decoded += size[0] as u64 * size[1] as u64 * 4;
        if decoded > 256 * 1024 * 1024 {
            return fail("/atlas", "decoded texture capacity");
        }
        if fields.get("format").is_some_and(|s| *s != "RGBA8888")
            || fields.get("repeat").is_some_and(|s| *s != "none")
        {
            return unsupported("/atlas/page");
        }
        let filter = fields.get("filter").copied().unwrap_or("Nearest, Nearest");
        if !["Nearest, Nearest", "Linear, Linear"].contains(&filter) {
            return unsupported("/atlas/filter");
        }
        let pma = match fields.get("pma").copied().unwrap_or("false") {
            "true" => true,
            "false" => false,
            _ => return fail("/atlas/pma", "expected boolean"),
        };
        let Some(id) = bindings.get(page_name) else {
            return Err(Diagnostic {
                code: "spine.import.atlas_page_missing",
                path: format!("/pages/{page_name}"),
                message: "page requires an AssetRef".into(),
            });
        };
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        {
            return fail("/pages", "invalid AssetRef identity");
        }
        canonical += &format!(
            "{page_name}\nsize: {}, {}\nformat: RGBA8888\nfilter: {filter}\nrepeat: none\npma: {pma}\n",
            size[0], size[1]
        );
        pages.push(Page {
            name: page_name.into(),
            texture_asset_id: id.clone(),
            pma,
            width: size[0] as u32,
            height: size[1] as u32,
        });
        while i < lines.len() && !lines[i].is_empty() {
            let region = lines[i];
            i += 1;
            if region.contains(':') || region.len() > 1024 || !regions.insert(region.to_string()) {
                return fail("/atlas/region", "invalid or duplicate region");
            }
            let mut f = BTreeMap::new();
            while i < lines.len() && lines[i].contains(':') {
                let (k, v) = lines[i].split_once(':').unwrap();
                i += 1;
                if !["bounds", "offsets", "rotate", "index"].contains(&k)
                    || f.insert(k, v.trim()).is_some()
                {
                    return fail(
                        "/atlas/region",
                        "unsupported/duplicate atlas field (use 4.2 bounds/offsets)",
                    );
                }
            }
            let b = ints(f.get("bounds").copied().unwrap_or(""), 4, "/atlas/bounds")?;
            let o = ints(f.get("offsets").copied().unwrap_or(""), 4, "/atlas/offsets")?;
            let rotation = match f.get("rotate").copied().unwrap_or("false") {
                "false" | "0" => 0,
                "true" | "90" => 90,
                _ => return unsupported("/atlas/rotate"),
            };
            let (w, h) = if rotation == 90 {
                (b[3], b[2])
            } else {
                (b[2], b[3])
            };
            if b[0] < 0
                || b[1] < 0
                || w <= 0
                || h <= 0
                || i64::from(b[0]) + i64::from(w) > i64::from(size[0])
                || i64::from(b[1]) + i64::from(h) > i64::from(size[1])
                || o[0] < 0
                || o[1] < 0
                || o[2] <= 0
                || o[3] <= 0
                || i64::from(o[0]) + i64::from(b[2]) > i64::from(o[2])
                || i64::from(o[1]) + i64::from(b[3]) > i64::from(o[3])
            {
                return fail("/atlas/region", "invalid bounds or trim offsets");
            }
            let idx = f
                .get("index")
                .copied()
                .unwrap_or("-1")
                .parse::<i32>()
                .map_err(|_| Diagnostic {
                    code: "spine.import.invalid_structure",
                    path: "/atlas/index".into(),
                    message: "invalid index".into(),
                })?;
            canonical += &format!(
                "{region}\nbounds: {}, {}, {}, {}\noffsets: {}, {}, {}, {}\nrotate: {rotation}\nindex: {idx}\n",
                b[0], b[1], b[2], b[3], o[0], o[1], o[2], o[3]
            );
        }
        canonical.push('\n');
    }
    if pages.is_empty() || bindings.len() != pages.len() {
        return fail(
            "/pages",
            "empty atlas or bindings do not match page closure",
        );
    }
    Ok(Atlas {
        pages,
        regions,
        canonical,
    })
}

#[derive(Debug, Clone)]
struct Attachment {
    kind: String,
    vertices: usize,
    deform: usize,
    indices: usize,
    end: Option<String>,
    parent: Option<(String, String)>,
}
type Attachments = BTreeMap<(String, String, String), Attachment>;
fn vertices(m: &Map<String, Value>, n: usize, bones: usize, p: &str) -> Result<usize> {
    if n == 0 || n > 8192 {
        return fail(p, "vertex capacity");
    }
    let a = array(required(m, "vertices", p)?, p, 8192 * 17)?;
    for v in a {
        number(v, p)?;
    }
    if a.len() == n * 2 {
        return Ok(n * 2);
    }
    let mut i = 0;
    let mut influences = 0;
    for _ in 0..n {
        let count = integer(a.get(i).unwrap_or(&Value::Null), p, 4)?;
        i += 1;
        if count == 0 || i + count * 4 > a.len() {
            return fail(p, "invalid influence stream");
        }
        let mut sum = 0.0;
        for _ in 0..count {
            integer(&a[i], p, bones - 1)?;
            let weight = number(&a[i + 3], p)?;
            if !(0.0..=1.0).contains(&weight) {
                return fail(p, "invalid influence weight");
            }
            sum += weight;
            i += 4;
            influences += 1;
        }
        if (sum - 1.0).abs() > 1e-4 {
            return fail(p, "influence weights must sum to one");
        }
    }
    if i != a.len() {
        return fail(p, "trailing weighted vertex data");
    }
    Ok(influences * 2)
}

/// Admission result carries only immutable, prechecked source. The C++ parser
/// receives canonical JSON/atlas; asset identifiers never become filesystem paths.
pub struct ValidatedSource {
    json: String,
    atlas: String,
    pub pages: Vec<Page>,
    pub bones: Vec<String>,
    pub slots: Vec<String>,
    pub skins: Vec<String>,
    pub animations: Vec<String>,
    pub animation_data: Vec<SourceAnimation>,
    pub source_digest: String,
}
impl ValidatedSource {
    pub fn cooked_json(&self) -> &str {
        &self.json
    }
    pub fn cooked_atlas(&self) -> &str {
        &self.atlas
    }
    pub fn load(&self) -> Result<Asset> {
        // SAFETY: construction is private and validates every admitted field and
        // structural reference against the pinned SkeletonJson/Atlas parser.
        unsafe { Asset::from_validated_json(&self.json, &self.atlas) }.map_err(|e| Diagnostic {
            code: "spine.import.backend_failure",
            path: "/".into(),
            message: format!("{e:?}"),
        })
    }
}

pub fn validate(
    json: &str,
    atlas_source: &str,
    page_refs: &BTreeMap<String, String>,
) -> Result<ValidatedSource> {
    if json.len() > 32 * 1024 * 1024 {
        return fail("/", "source exceeds 32MiB admission budget");
    }
    let Unique(root) = serde_json::from_str::<Unique>(json).map_err(|e| Diagnostic {
        code: "spine.import.invalid_structure",
        path: "/".into(),
        message: e.to_string(),
    })?;
    let r = object(
        &root,
        "/",
        &[
            "skeleton",
            "bones",
            "slots",
            "skins",
            "animations",
            "events",
            "ik",
            "transform",
            "path",
            "physics",
        ],
    )?;
    if r.contains_key("physics") {
        return unsupported("/physics");
    }
    let sk = object(
        required(r, "skeleton", "/")?,
        "/skeleton",
        &[
            "hash",
            "spine",
            "x",
            "y",
            "width",
            "height",
            "referenceScale",
            "fps",
            "images",
            "audio",
        ],
    )?;
    let version = name(sk, "spine", "/skeleton")?;
    if version != "4.2"
        && !(version.starts_with("4.2.")
            && version[4..].bytes().all(|b| b.is_ascii_digit())
            && version.len() > 4)
    {
        return Err(Diagnostic {
            code: "spine.import.version_mismatch",
            path: "/skeleton/spine".into(),
            message: "only 4.2 JSON is admitted".into(),
        });
    }
    numeric_fields(
        sk,
        &["x", "y", "width", "height", "referenceScale", "fps"],
        "/skeleton",
    )?;
    for k in ["hash", "images", "audio"] {
        if let Some(v) = sk.get(k)
            && !v.is_string()
        {
            return fail("/skeleton", "expected string metadata");
        }
    }
    let atlas = atlas(atlas_source, page_refs)?;
    let mut bone_set = BTreeSet::new();
    let mut bone_names = Vec::new();
    let mut parents = BTreeMap::new();
    for v in array(required(r, "bones", "/")?, "/bones", 256)? {
        let m = object(
            v,
            "/bones",
            &[
                "name", "parent", "length", "x", "y", "rotation", "scaleX", "scaleY", "shearX",
                "shearY", "inherit", "skin", "color", "icon", "visible",
            ],
        )?;
        let n = name(m, "name", "/bones")?;
        if let Some(p) = m.get("parent") {
            member(&bone_set, p, "/bones/parent")?;
            parents.insert(n.to_string(), string(p, "/bones/parent")?.to_string());
        } else if !bone_names.is_empty() {
            return fail("/bones/parent", "only first bone may be root");
        }
        if !bone_set.insert(n.into()) {
            return fail("/bones/name", "duplicate bone");
        }
        bone_names.push(n.into());
        numeric_fields(
            m,
            &[
                "length", "x", "y", "rotation", "scaleX", "scaleY", "shearX", "shearY",
            ],
            "/bones",
        )?;
        bool_fields(m, &["skin", "visible"], "/bones")?;
        color(m, "color", "/bones", 8)?;
        if let Some(v) = m.get("icon") {
            string(v, "/bones/icon")?;
        }
        choice(
            m,
            "inherit",
            &[
                "normal",
                "onlyTranslation",
                "noRotationOrReflection",
                "noScale",
                "noScaleOrReflection",
            ],
            "/bones/inherit",
        )?;
    }
    if bone_names.is_empty() {
        return fail("/bones", "at least one bone required");
    }
    let mut slot_set = BTreeSet::new();
    let mut slot_names = Vec::new();
    let mut setup = BTreeMap::new();
    if let Some(v) = r.get("slots") {
        for v in array(v, "/slots", 256)? {
            let m = object(
                v,
                "/slots",
                &["name", "bone", "attachment", "color", "dark", "blend"],
            )?;
            if m.contains_key("dark") {
                return unsupported("/slots/dark");
            }
            let n = name(m, "name", "/slots")?;
            member(&bone_set, required(m, "bone", "/slots")?, "/slots/bone")?;
            if !slot_set.insert(n.into()) {
                return fail("/slots/name", "duplicate slot");
            }
            slot_names.push(n.into());
            color(m, "color", "/slots", 8)?;
            choice(
                m,
                "blend",
                &["normal", "additive", "multiply", "screen"],
                "/slots/blend",
            )?;
            if let Some(v) = m.get("attachment") {
                setup.insert(n.to_string(), string(v, "/slots/attachment")?.to_string());
            }
        }
    }
    let constraint_names = constraints::setup(r, &bone_set, &slot_set, &parents)?;
    let mut attachments = Attachments::new();
    let mut skins = BTreeSet::new();
    let mut attachment_count = 0;
    if let Some(v) = r.get("skins") {
        for v in array(v, "/skins", 16)? {
            let m = object(
                v,
                "/skins",
                &[
                    "name",
                    "attachments",
                    "bones",
                    "color",
                    "ik",
                    "transform",
                    "path",
                ],
            )?;
            constraints::skin(m, &constraint_names)?;
            let skin = name(m, "name", "/skins")?;
            if !skins.insert(skin.to_string()) {
                return fail("/skins/name", "duplicate skin");
            }
            color(m, "color", "/skins", 8)?;
            if let Some(b) = m.get("bones") {
                for v in array(b, "/skins/bones", 256)? {
                    member(&bone_set, v, "/skins/bones")?;
                }
            }
            if let Some(v) = m.get("attachments") {
                for (slot, v) in dict(v, "/skins/attachments")? {
                    if !slot_set.contains(slot) {
                        return fail("/skins/attachments", "unknown slot");
                    }
                    for (an, v) in dict(v, "/skins/attachments/slot")? {
                        if an.is_empty() || an.len() > 1024 {
                            return fail("/skins/attachments", "invalid attachment name");
                        }
                        attachment_count += 1;
                        if attachment_count > 512 {
                            return fail("/skins", "attachment capacity");
                        }
                        let m = object(
                            v,
                            "/attachment",
                            &[
                                "name",
                                "type",
                                "path",
                                "x",
                                "y",
                                "rotation",
                                "scaleX",
                                "scaleY",
                                "width",
                                "height",
                                "color",
                                "uvs",
                                "triangles",
                                "vertices",
                                "hull",
                                "edges",
                                "parent",
                                "skin",
                                "timelines",
                                "vertexCount",
                                "end",
                                "sequence",
                                "closed",
                                "constantSpeed",
                                "lengths",
                            ],
                        )?;
                        if m.contains_key("sequence") {
                            return unsupported("/attachment/sequence");
                        }
                        for k in ["name", "path", "parent", "skin", "end"] {
                            if let Some(v) = m.get(k) {
                                string(v, "/attachment")?;
                            }
                        }
                        numeric_fields(
                            m,
                            &["x", "y", "rotation", "scaleX", "scaleY", "width", "height"],
                            "/attachment",
                        )?;
                        color(m, "color", "/attachment", 8)?;
                        bool_fields(m, &["timelines", "closed", "constantSpeed"], "/attachment")?;
                        let kind = m
                            .get("type")
                            .map(|v| string(v, "/attachment/type"))
                            .transpose()?
                            .unwrap_or("region");
                        if !["region", "mesh", "linkedmesh", "clipping", "path"].contains(&kind) {
                            return unsupported("/attachment/type");
                        }
                        if kind != "clipping" && kind != "path" {
                            let path = m
                                .get("path")
                                .or_else(|| m.get("name"))
                                .and_then(Value::as_str)
                                .unwrap_or(an);
                            if !atlas.regions.contains(path) {
                                return fail("/attachment/path", "missing atlas region");
                            }
                        }
                        let mut a = Attachment {
                            kind: kind.into(),
                            vertices: 0,
                            deform: 0,
                            indices: 0,
                            end: None,
                            parent: None,
                        };
                        if let Some(parent) = m.get("parent") {
                            if kind != "mesh" && kind != "linkedmesh" {
                                return fail("/attachment/parent", "parent only allowed on mesh");
                            }
                            a.parent = Some((
                                m.get("skin")
                                    .and_then(Value::as_str)
                                    .unwrap_or("default")
                                    .into(),
                                string(parent, "/attachment/parent")?.into(),
                            ));
                        } else if kind == "linkedmesh" {
                            return fail("/attachment/parent", "linkedmesh requires parent");
                        } else if kind == "region" {
                            a.vertices = 4;
                            a.indices = 6;
                        } else {
                            a.vertices = if kind == "mesh" {
                                let uv = array(
                                    required(m, "uvs", "/attachment")?,
                                    "/attachment/uvs",
                                    8192 * 2,
                                )?;
                                if uv.len() % 2 != 0 {
                                    return fail("/attachment/uvs", "odd uv count");
                                }
                                for v in uv {
                                    number(v, "/attachment/uvs")?;
                                }
                                uv.len() / 2
                            } else {
                                integer(
                                    required(m, "vertexCount", "/attachment")?,
                                    "/attachment/vertexCount",
                                    if kind == "path" { 8192 } else { 16 },
                                )?
                            };
                            if kind == "clipping" && a.vertices < 3 {
                                return fail("/attachment/vertexCount", "clip needs >=3 vertices");
                            }
                            a.deform =
                                vertices(m, a.vertices, bone_names.len(), "/attachment/vertices")?;
                            if kind == "path" {
                                if a.vertices < 6 || !a.vertices.is_multiple_of(3) {
                                    return fail(
                                        "/attachment/path",
                                        "path requires >=6 vertices in cubic groups of three",
                                    );
                                }
                                let lengths = array(
                                    required(m, "lengths", "/attachment/path")?,
                                    "/attachment/path/lengths",
                                    a.vertices / 3,
                                )?;
                                if lengths.len() != a.vertices / 3 {
                                    return fail(
                                        "/attachment/path/lengths",
                                        "wrong path lengths count",
                                    );
                                }
                                let mut previous = 0f32;
                                for v in lengths {
                                    let length = number(v, "/attachment/path/lengths")? as f32;
                                    if length <= previous {
                                        return fail(
                                            "/attachment/path/lengths",
                                            "lengths must increase and be positive",
                                        );
                                    }
                                    previous = length;
                                }
                            }
                            if kind == "mesh" {
                                let tri = array(
                                    required(m, "triangles", "/attachment")?,
                                    "/attachment/triangles",
                                    49152,
                                )?;
                                if tri.is_empty() || tri.len() % 3 != 0 {
                                    return fail(
                                        "/attachment/triangles",
                                        "triangle triplets required",
                                    );
                                }
                                for v in tri {
                                    integer(v, "/attachment/triangles", a.vertices - 1)?;
                                }
                                a.indices = tri.len();
                                if let Some(v) = m.get("hull") {
                                    integer(v, "/attachment/hull", a.vertices)?;
                                }
                                if let Some(v) = m.get("edges") {
                                    for v in array(v, "/attachment/edges", 49152)? {
                                        integer(v, "/attachment/edges", a.vertices * 2 - 1)?;
                                    }
                                }
                            } else if kind == "clipping"
                                && let Some(v) = m.get("end")
                            {
                                member(&slot_set, v, "/attachment/end")?;
                                a.end = Some(string(v, "/attachment/end")?.into());
                            }
                        }
                        attachments.insert((skin.into(), slot.clone(), an.clone()), a);
                    }
                }
            }
        }
    }
    // Native resolves linked meshes in declaration order; no chain/cycle is
    // admitted here, and every parent is a concrete mesh in the same slot.
    let links: Vec<_> = attachments
        .iter()
        .filter_map(|(k, a)| a.parent.as_ref().map(|p| (k.clone(), p.clone())))
        .collect();
    for ((skin, slot, n), (ps, pn)) in links {
        let Some(parent) = attachments.get(&(ps, slot.clone(), pn)).cloned() else {
            return fail("/attachment/parent", "missing parent mesh");
        };
        if parent.kind != "mesh" || parent.parent.is_some() {
            return unsupported("/attachment/parent/chain");
        }
        let a = attachments.get_mut(&(skin, slot, n)).unwrap();
        a.vertices = parent.vertices;
        a.indices = parent.indices;
        a.deform = parent.deform;
    }
    for (slot, n) in setup {
        if !attachments.keys().any(|(_, s, a)| *s == slot && *a == n) {
            return fail("/slots/attachment", "unknown setup attachment");
        }
    }
    let mut raw_vertices = 0;
    let mut raw_indices = 0;
    let mut clips = 0;
    for slot in &slot_names {
        raw_vertices += attachments
            .iter()
            .filter(|((_, s, _), a)| s == slot && a.kind != "clipping" && a.kind != "path")
            .map(|(_, a)| a.vertices)
            .max()
            .unwrap_or(0);
        raw_indices += attachments
            .iter()
            .filter(|((_, s, _), a)| s == slot && a.kind != "clipping" && a.kind != "path")
            .map(|(_, a)| a.indices)
            .max()
            .unwrap_or(0);
        clips += usize::from(
            attachments
                .iter()
                .any(|((_, s, _), a)| s == slot && a.kind == "clipping"),
        );
    }
    if raw_vertices > 8192 || raw_indices > 49152 || clips > 8 {
        return fail("/skins", "aggregate geometry capacity");
    }
    validate_clip_order(&slot_names, &attachments)?;
    let mut event_names = BTreeSet::new();
    if let Some(v) = r.get("events") {
        for (n, v) in dict(v, "/events")? {
            event_names.insert(n.clone());
            let m = object(
                v,
                "/events",
                &["int", "float", "string", "audio", "volume", "balance"],
            )?;
            event_fields(m, "/events")?;
        }
    }
    let mut animations = Vec::new();
    let mut animation_data = Vec::new();
    let mut expanded_bytes = 0usize;
    if let Some(v) = r.get("animations") {
        let m = dict(v, "/animations")?;
        if m.len() > 128 {
            return fail("/animations", "clip capacity");
        }
        for (n, v) in m {
            if n.is_empty() {
                return fail("/animations", "empty animation name");
            }
            animation(
                v,
                &bone_set,
                &slot_names,
                &skins,
                &attachments,
                &event_names,
                &constraint_names,
            )?;
            metadata::check_expansion(v, &attachments, &mut expanded_bytes)?;
            animation_data.push(metadata::animation_metadata(n, v, r.get("events"))?);
            animations.push(n.clone());
        }
    }
    Ok(ValidatedSource {
        json: serde_json::to_string(&root).unwrap(),
        atlas: atlas.canonical,
        pages: atlas.pages,
        bones: bone_names,
        slots: slot_names,
        skins: skins.into_iter().collect(),
        animations,
        animation_data,
        source_digest: metadata::digest_parts(&[json.as_bytes(), atlas_source.as_bytes()]),
    })
}

fn validate_clip_order(order: &[String], attachments: &Attachments) -> Result<()> {
    let mut active_end = None;
    for (i, slot) in order.iter().enumerate() {
        let mut possible_ends = Vec::new();
        for (_, a) in attachments
            .iter()
            .filter(|((_, s, _), a)| s == slot && a.kind == "clipping")
        {
            if active_end.is_some_and(|end| i <= end) {
                return unsupported("/skins/clipping/nested_interval");
            }
            let end = match &a.end {
                Some(s) => order.iter().position(|x| x == s).unwrap(),
                None => order.len(),
            };
            if end < i {
                return fail("/attachment/end", "end slot precedes clipping start");
            }
            possible_ends.push(end);
        }
        if let Some(end) = possible_ends.into_iter().max() {
            active_end = Some(end);
        }
    }
    Ok(())
}
fn event_fields(m: &Map<String, Value>, p: &str) -> Result<()> {
    numeric_fields(m, &["float", "volume", "balance"], p)?;
    if let Some(v) = m.get("int")
        && v.as_i64().is_none_or(|n| i32::try_from(n).is_err())
    {
        return fail(p, "event int outside i32");
    }
    for k in ["string", "audio"] {
        if let Some(v) = m.get(k)
            && !v.is_string()
        {
            return fail(p, "expected payload string");
        }
    }
    Ok(())
}
fn frames<'a>(v: &'a Value, p: &str, fields: &[&str], components: usize) -> Result<&'a Vec<Value>> {
    let a = array(v, p, 65536)?;
    if a.is_empty() {
        return fail(p, "empty timeline");
    }
    let mut previous = -1.0;
    for v in a {
        let mut allowed = vec!["time", "curve"];
        allowed.extend_from_slice(fields);
        let m = object(v, p, &allowed)?;
        let time = m
            .get("time")
            .map(|v| number(v, p))
            .transpose()?
            .unwrap_or(0.0);
        let invalid_order = if p == "/animation/events" {
            time < previous
        } else {
            time as f32 <= previous as f32
        };
        if !(0.0..=3600.0).contains(&time) || invalid_order {
            return fail(
                p,
                "keys must increase within 0..3600s (equal time only for events)",
            );
        }
        previous = time;
        if let Some(v) = m.get("curve") {
            if v.as_str() == Some("stepped") {
            } else {
                let curve = array(v, p, components * 4)?;
                if curve.len() != components * 4 {
                    return fail(p, "wrong Bezier component count");
                }
                for v in curve {
                    number(v, p)?;
                }
            }
        }
    }
    Ok(a)
}
fn animation(
    v: &Value,
    bones: &BTreeSet<String>,
    slots: &[String],
    skins: &BTreeSet<String>,
    attachments: &Attachments,
    events: &BTreeSet<String>,
    constraint_names: &constraints::Names,
) -> Result<()> {
    let m = object(
        v,
        "/animation",
        &[
            "bones",
            "slots",
            "attachments",
            "drawOrder",
            "events",
            "ik",
            "transform",
            "path",
        ],
    )?;
    constraints::timelines(m, constraint_names)?;
    if let Some(v) = m.get("bones") {
        for (n, v) in dict(v, "/animation/bones")? {
            if !bones.contains(n) {
                return fail("/animation/bones", "unknown bone");
            }
            for (kind, v) in dict(v, "/animation/bones/timelines")? {
                let (keys, c) = match kind.as_str() {
                    "rotate" | "translatex" | "translatey" | "scalex" | "scaley" | "shearx"
                    | "sheary" => (vec!["value"], 1),
                    "translate" | "scale" | "shear" => (vec!["x", "y"], 2),
                    _ => return unsupported("/animation/bones/timeline"),
                };
                for f in frames(v, "/animation/bones/keys", &keys, c)? {
                    numeric_fields(f.as_object().unwrap(), &keys, "/animation/bones/keys")?;
                }
            }
        }
    }
    if let Some(v) = m.get("slots") {
        for (n, v) in dict(v, "/animation/slots")? {
            if !slots.contains(n) {
                return fail("/animation/slots", "unknown slot");
            }
            for (kind, v) in dict(v, "/animation/slots/timelines")? {
                let (keys, c) = match kind.as_str() {
                    "attachment" => (vec!["name"], 0),
                    "rgba" => (vec!["color"], 4),
                    "rgb" => (vec!["color"], 3),
                    "alpha" => (vec!["value"], 1),
                    _ => return unsupported("/animation/slots/timeline"),
                };
                for f in frames(v, "/animation/slots/keys", &keys, c)? {
                    let f = f.as_object().unwrap();
                    if kind == "attachment" {
                        if f.contains_key("curve") {
                            return fail("/animation/slots", "discrete curve");
                        }
                        if let Some(v) = f.get("name").filter(|v| !v.is_null()) {
                            let name = string(v, "/animation/slots/name")?;
                            if !attachments.keys().any(|(_, s, a)| s == n && a == name) {
                                return fail("/animation/slots/name", "unknown attachment");
                            }
                        }
                    } else if kind == "alpha" {
                        numeric_fields(f, &["value"], "/animation/slots/alpha")?;
                    } else {
                        color(
                            f,
                            "color",
                            "/animation/slots/color",
                            if kind == "rgb" { 6 } else { 8 },
                        )?;
                        if !f.contains_key("color") {
                            return fail("/animation/slots/color", "color required");
                        }
                    }
                }
            }
        }
    }
    if let Some(v) = m.get("attachments") {
        for (skin, v) in dict(v, "/animation/attachments")? {
            if !skins.contains(skin) {
                return fail("/animation/attachments", "unknown skin");
            }
            for (slot, v) in dict(v, "/animation/attachments/skin")? {
                for (an, v) in dict(v, "/animation/attachments/slot")? {
                    let Some(a) = attachments.get(&(skin.clone(), slot.clone(), an.clone())) else {
                        return fail("/animation/attachments", "unknown attachment");
                    };
                    if a.deform == 0 {
                        return fail(
                            "/animation/attachments",
                            "deform requires vertex attachment",
                        );
                    }
                    let t = object(v, "/animation/attachments/timeline", &["deform"])?;
                    if let Some(v) = t.get("deform") {
                        for f in frames(v, "/animation/deform", &["offset", "vertices"], 1)? {
                            let f = f.as_object().unwrap();
                            let offset = f
                                .get("offset")
                                .map(|v| integer(v, "/animation/deform/offset", a.deform))
                                .transpose()?
                                .unwrap_or(0);
                            if let Some(v) = f.get("vertices") {
                                let values = array(v, "/animation/deform/vertices", a.deform)?;
                                if offset + values.len() > a.deform {
                                    return fail("/animation/deform", "deform range overflow");
                                }
                                for v in values {
                                    number(v, "/animation/deform/vertices")?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(v) = m.get("drawOrder") {
        for f in frames(v, "/animation/drawOrder", &["offsets"], 0)? {
            let f = f.as_object().unwrap();
            if f.contains_key("curve") {
                return fail("/animation/drawOrder", "discrete curve");
            }
            let mut output = vec![None; slots.len()];
            let mut unchanged = Vec::new();
            let mut original = 0;
            if let Some(v) = f.get("offsets") {
                for v in array(v, "/animation/drawOrder/offsets", slots.len())? {
                    let o = object(v, "/animation/drawOrder/offset", &["slot", "offset"])?;
                    let slot = name(o, "slot", "/animation/drawOrder")?;
                    let Some(index) = slots.iter().position(|s| s == slot) else {
                        return fail("/animation/drawOrder", "unknown slot");
                    };
                    if index < original {
                        return fail(
                            "/animation/drawOrder",
                            "offset slots must be ordered and unique",
                        );
                    }
                    while original < index {
                        unchanged.push(original);
                        original += 1;
                    }
                    let offset = required(o, "offset", "/animation/drawOrder")?
                        .as_i64()
                        .ok_or_else(|| Diagnostic {
                            code: "spine.import.invalid_structure",
                            path: "/animation/drawOrder".into(),
                            message: "offset must be integer".into(),
                        })?;
                    let target = (index as i64)
                        .checked_add(offset)
                        .filter(|t| *t >= 0 && *t < slots.len() as i64)
                        .ok_or_else(|| Diagnostic {
                            code: "spine.import.invalid_structure",
                            path: "/animation/drawOrder".into(),
                            message: "offset out of range".into(),
                        })? as usize;
                    if output[target].replace(index).is_some() {
                        return fail("/animation/drawOrder", "colliding target slots");
                    }
                    original += 1;
                }
            }
            unchanged.extend(original..slots.len());
            let mut rest = unchanged.into_iter();
            let order: Vec<_> = output
                .into_iter()
                .map(|x| slots[x.unwrap_or_else(|| rest.next().unwrap())].clone())
                .collect();
            validate_clip_order(&order, attachments)?;
        }
    }
    if let Some(v) = m.get("events") {
        for f in frames(
            v,
            "/animation/events",
            &["name", "int", "float", "string", "volume", "balance"],
            0,
        )? {
            let f = f.as_object().unwrap();
            member(
                events,
                required(f, "name", "/animation/events")?,
                "/animation/events/name",
            )?;
            event_fields(f, "/animation/events")?;
            if f.contains_key("curve") {
                return fail("/animation/events", "discrete curve");
            }
        }
        if v.as_array().unwrap().len() > 4096 {
            return fail("/animation/events", "event capacity");
        }
    }
    Ok(())
}
