//! Field/reference checks mirrored from the pinned SkeletonJson reader, before
//! any native constraint constructor dereferences bone/target data.
use super::*;
pub(super) type Names = BTreeMap<String, BTreeSet<String>>;
const MIX: &[&str] = &[
    "mixRotate",
    "mixX",
    "mixY",
    "mixScaleX",
    "mixScaleY",
    "mixShearY",
];
fn mix_fields(m: &Map<String, Value>, keys: &[&str], p: &str) -> Result<()> {
    for k in keys {
        if let Some(v) = m.get(*k)
            && !(0.0..=1.0).contains(&number(v, p)?)
        {
            return fail(p, "constraint mix outside 0..1");
        }
    }
    Ok(())
}
pub(super) fn setup(
    r: &Map<String, Value>,
    bones: &BTreeSet<String>,
    slots: &BTreeSet<String>,
    parents: &BTreeMap<String, String>,
) -> Result<Names> {
    let mut result = Names::new();
    let mut orders = BTreeSet::new();
    let mut total = 0;
    for kind in ["ik", "transform", "path"] {
        let mut names = BTreeSet::new();
        if let Some(v) = r.get(kind) {
            for v in array(v, &format!("/{kind}"), 256)? {
                total += 1;
                if total > 256 {
                    return fail("/constraints", "constraint capacity");
                }
                let mut allowed = vec!["name", "order", "skin", "bones", "target"];
                allowed.extend_from_slice(match kind {
                    "ik" => &[
                        "mix",
                        "softness",
                        "bendPositive",
                        "compress",
                        "stretch",
                        "uniform",
                    ][..],
                    "transform" => &[
                        "local",
                        "relative",
                        "rotation",
                        "x",
                        "y",
                        "scaleX",
                        "scaleY",
                        "shearY",
                        "mixRotate",
                        "mixX",
                        "mixY",
                        "mixScaleX",
                        "mixScaleY",
                        "mixShearY",
                    ][..],
                    _ => &[
                        "positionMode",
                        "spacingMode",
                        "rotateMode",
                        "rotation",
                        "position",
                        "spacing",
                        "mixRotate",
                        "mixX",
                        "mixY",
                    ][..],
                });
                let m = object(v, "/constraint", &allowed)?;
                let n = name(m, "name", "/constraint")?;
                if !names.insert(n.to_string()) {
                    return fail("/constraint/name", "duplicate constraint");
                }
                let order = m
                    .get("order")
                    .map(|v| integer(v, "/constraint/order", 255))
                    .transpose()?
                    .unwrap_or(0);
                if !orders.insert(order) {
                    return fail(
                        "/constraint/order",
                        "constraint order must be globally unique",
                    );
                }
                bool_fields(
                    m,
                    &[
                        "skin",
                        "local",
                        "relative",
                        "bendPositive",
                        "compress",
                        "stretch",
                        "uniform",
                    ],
                    "/constraint",
                )?;
                let list = array(
                    required(m, "bones", "/constraint")?,
                    "/constraint/bones",
                    if kind == "ik" { 2 } else { 256 },
                )?;
                if list.is_empty() {
                    return fail("/constraint/bones", "at least one bone required");
                }
                let mut seen = BTreeSet::new();
                let mut bound = Vec::new();
                for v in list {
                    member(bones, v, "/constraint/bones")?;
                    let bone = string(v, "/constraint/bones")?;
                    if !seen.insert(bone) || !parents.contains_key(bone) {
                        return fail("/constraint/bones", "duplicate or root constrained bone");
                    }
                    bound.push(bone);
                }
                if kind == "ik" && bound.len() == 2 && parents[bound[1]] != bound[0] {
                    return fail("/ik/bones", "two-bone IK requires direct parent/child");
                }
                member(
                    if kind == "path" { slots } else { bones },
                    required(m, "target", "/constraint")?,
                    "/constraint/target",
                )?;
                if kind != "path" && seen.contains(name(m, "target", "/constraint")?) {
                    return fail("/constraint/target", "target cannot constrain itself");
                }
                numeric_fields(
                    m,
                    &[
                        "rotation", "x", "y", "scaleX", "scaleY", "shearY", "position", "spacing",
                        "softness",
                    ],
                    "/constraint",
                )?;
                if m.get("softness").is_some_and(|v| v.as_f64().unwrap() < 0.0) {
                    return fail("/ik/softness", "negative softness");
                }
                mix_fields(m, &["mix"], "/ik/mix")?;
                mix_fields(m, MIX, "/constraint/mix")?;
                if kind == "path" {
                    choice(
                        m,
                        "positionMode",
                        &["fixed", "percent"],
                        "/path/positionMode",
                    )?;
                    choice(
                        m,
                        "spacingMode",
                        &["length", "fixed", "percent"],
                        "/path/spacingMode",
                    )?;
                    choice(
                        m,
                        "rotateMode",
                        &["tangent", "chain", "chainScale"],
                        "/path/rotateMode",
                    )?;
                }
            }
        }
        result.insert(kind.into(), names);
    }
    if orders.iter().copied().ne(0..total) {
        return fail(
            "/constraint/order",
            "constraint order must cover 0..count-1",
        );
    }
    Ok(result)
}
pub(super) fn skin(m: &Map<String, Value>, names: &Names) -> Result<()> {
    for kind in ["ik", "transform", "path"] {
        if let Some(v) = m.get(kind) {
            let mut seen = BTreeSet::new();
            for v in array(v, "/skins/constraints", 256)? {
                member(&names[kind], v, "/skins/constraints")?;
                if !seen.insert(string(v, "/skins/constraints")?) {
                    return fail("/skins/constraints", "duplicate constraint reference");
                }
            }
        }
    }
    Ok(())
}
pub(super) fn timelines(m: &Map<String, Value>, names: &Names) -> Result<()> {
    for kind in ["ik", "transform", "path"] {
        if let Some(v) = m.get(kind) {
            for (n, v) in dict(v, "/animation/constraints")? {
                if !names[kind].contains(n) {
                    return fail("/animation/constraints", "unknown constraint");
                }
                if kind == "path" {
                    for (timeline, v) in dict(v, "/animation/path")? {
                        let (keys, components) = match timeline.as_str() {
                            "position" | "spacing" => (vec!["value"], 1),
                            "mix" => (vec!["mixRotate", "mixX", "mixY"], 3),
                            _ => return unsupported("/animation/path/timeline"),
                        };
                        for f in frames(v, "/animation/path/keys", &keys, components)? {
                            let f = f.as_object().unwrap();
                            numeric_fields(f, &keys, "/animation/path/keys")?;
                            if timeline == "mix" {
                                mix_fields(f, &keys, "/animation/path/keys")?;
                            }
                        }
                    }
                } else {
                    let (keys, components) = if kind == "ik" {
                        (
                            vec!["mix", "softness", "bendPositive", "compress", "stretch"],
                            2,
                        )
                    } else {
                        (MIX.to_vec(), 6)
                    };
                    for f in frames(v, "/animation/constraints/keys", &keys, components)? {
                        let f = f.as_object().unwrap();
                        if kind == "ik" {
                            mix_fields(f, &["mix"], "/animation/ik")?;
                            numeric_fields(f, &["softness"], "/animation/ik")?;
                            bool_fields(
                                f,
                                &["bendPositive", "compress", "stretch"],
                                "/animation/ik",
                            )?;
                            if f.get("softness").is_some_and(|v| v.as_f64().unwrap() < 0.0) {
                                return fail("/animation/ik", "negative softness");
                            }
                        } else {
                            mix_fields(f, MIX, "/animation/transform")?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
