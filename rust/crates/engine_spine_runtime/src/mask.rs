//! Explicit source-name selection. Validation happens before any native clone.
use crate::{
    Asset, Error, Mask, MaskOwner, Opaque,
    import::{Diagnostic, ValidatedSource},
    native_result, text,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ptr::NonNull,
    rc::Rc,
};

#[derive(Debug, Clone)]
pub struct MaskPlan {
    identity: [u8; 32],
    animation: String,
    bits: [u64; 4],
}
unsafe extern "C" {
    fn s374_mask_create256(
        asset: *mut Opaque,
        animation: *const std::os::raw::c_char,
        bits: *const u64,
        out: *mut *mut Opaque,
    ) -> i32;
}
fn conflict(path: &str, message: &str) -> Diagnostic {
    Diagnostic {
        code: "animator.mask.dependency_conflict",
        path: path.into(),
        message: message.into(),
    }
}
fn entries(v: &Value) -> impl Iterator<Item = (&String, &Value)> {
    v.as_object().into_iter().flatten()
}
fn items(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}
impl ValidatedSource {
    pub fn mask_plan(&self, animation: &str, names: &[&str]) -> Result<MaskPlan, Diagnostic> {
        let root: Value =
            serde_json::from_str(self.cooked_json()).expect("validated canonical source");
        let clip = &root["animations"][animation];
        if clip.is_null() {
            return Err(Diagnostic {
                code: "animator.binding.unknown_action",
                path: animation.into(),
                message: "unknown source clip".into(),
            });
        }
        let bones: BTreeMap<&str, usize> = items(&root["bones"])
            .enumerate()
            .map(|(i, n)| (n["name"].as_str().unwrap(), i))
            .collect();
        let mut selected = BTreeSet::new();
        for name in names {
            let i = bones
                .get(name)
                .ok_or_else(|| conflict(name, "unknown explicit bone"))?;
            selected.insert(*i);
        }
        if selected.is_empty() {
            return Err(Diagnostic {
                code: "animator.mask.empty_selection",
                path: animation.into(),
                message: "empty explicit bone set".into(),
            });
        }
        let chosen = |name: &str| bones.get(name).is_some_and(|i| selected.contains(i));
        let bone_index = |name: &str| *bones.get(name).expect("validated bone reference");
        let slot_bones: BTreeMap<&str, &str> = items(&root["slots"])
            .map(|s| (s["name"].as_str().unwrap(), s["bone"].as_str().unwrap()))
            .collect();
        let parents: BTreeMap<&str, &str> = items(&root["bones"])
            .filter_map(|b| Some((b["name"].as_str()?, b["parent"].as_str()?)))
            .collect();
        // An excluded animated descendant still inherits a selected parent's pose.
        for (name, _) in entries(&clip["bones"]) {
            if chosen(name) {
                continue;
            }
            let mut ancestor = name.as_str();
            while let Some(parent) = parents.get(ancestor) {
                if chosen(parent) {
                    return Err(conflict(
                        name,
                        "selected ancestor drives excluded animated descendant",
                    ));
                }
                ancestor = parent;
            }
        }
        let bone_names: Vec<_> = items(&root["bones"])
            .map(|b| b["name"].as_str().unwrap())
            .collect();
        let check_set = |set: &[usize], path: &str| -> Result<(), Diagnostic> {
            let count = set.iter().filter(|i| selected.contains(i)).count();
            if count != 0 && count != set.len() {
                Err(conflict(path, "dependency crosses explicit bone selection"))
            } else if count == 0
                && set.len() > 1
                && set.iter().any(|i| {
                    let mut name = bone_names[*i];
                    while let Some(parent) = parents.get(name) {
                        if chosen(parent) && clip["bones"].get(*parent).is_some() {
                            return true;
                        }
                        name = parent;
                    }
                    false
                })
            {
                Err(conflict(
                    path,
                    "selected animated ancestor drives an excluded dependency",
                ))
            } else {
                Ok(())
            }
        };
        // Constraints read targets and write constrained bones even if their mix
        // has no keys in this clip. A partial dependency closure is unsafe.
        for kind in ["ik", "transform", "path"] {
            for c in items(&root[kind]) {
                let mut set: Vec<usize> = items(&c["bones"])
                    .map(|n| bone_index(n.as_str().unwrap()))
                    .collect();
                let target = c["target"].as_str().unwrap();
                set.push(bone_index(if kind == "path" {
                    slot_bones[target]
                } else {
                    target
                }));
                check_set(&set, kind)?;
            }
        }
        if !clip["drawOrder"].is_null() && slot_bones.values().any(|b| !chosen(b)) {
            return Err(conflict(
                "drawOrder",
                "global draw order requires every slot bone",
            ));
        }
        let slots: Vec<&str> = items(&root["slots"])
            .map(|s| s["name"].as_str().unwrap())
            .collect();
        for skin in items(&root["skins"]) {
            for (slot, attachments) in entries(&skin["attachments"]) {
                for (name, attachment) in entries(attachments) {
                    let path = format!("{}/{slot}/{name}", skin["name"].as_str().unwrap());
                    let mut set = vec![bone_index(slot_bones[slot.as_str()])];
                    let vertices = attachment["vertices"].as_array();
                    let expected = if attachment["type"] == "mesh" {
                        attachment["uvs"].as_array().map(Vec::len)
                    } else {
                        attachment["vertexCount"].as_u64().map(|v| v as usize * 2)
                    };
                    if let (Some(v), Some(expected)) = (vertices, expected)
                        && v.len() != expected
                    {
                        let mut cursor = 0;
                        while cursor < v.len() {
                            let count = v[cursor].as_u64().unwrap() as usize;
                            cursor += 1;
                            for _ in 0..count {
                                set.push(v[cursor].as_u64().unwrap() as usize);
                                cursor += 4;
                            }
                        }
                    }
                    if attachment["type"] == "linkedmesh" {
                        // Parent slot defaults to this slot in the pinned parser.
                        let parent_slot = attachment["slot"].as_str().unwrap_or(slot);
                        set.push(bone_index(slot_bones[parent_slot]));
                    }
                    if attachment["type"] == "clipping" {
                        let start = slots.iter().position(|s| *s == slot).unwrap();
                        let end = attachment["end"]
                            .as_str()
                            .and_then(|s| slots.iter().position(|x| *x == s))
                            .unwrap_or(slots.len() - 1);
                        for s in &slots[start..=end] {
                            set.push(bone_index(slot_bones[s]));
                        }
                    }
                    check_set(&set, &path)?;
                }
            }
        }
        let mut bits = [0; 4];
        for i in selected {
            bits[i / 64] |= 1 << (i % 64);
        }
        Ok(MaskPlan {
            identity: Sha256::digest(self.cooked_json().as_bytes()).into(),
            animation: animation.into(),
            bits,
        })
    }
}
impl Asset {
    pub fn bind_mask(&self, plan: &MaskPlan) -> Result<Mask, Error> {
        if self.0.1 != plan.identity {
            return Err(Error::WrongAsset);
        }
        let animation = text(&plan.animation)?;
        let mut pointer = std::ptr::null_mut();
        // SAFETY: source-bound validated dependency plan, live strings and bitset.
        native_result(unsafe {
            s374_mask_create256(
                self.ptr(),
                animation.as_ptr(),
                plan.bits.as_ptr(),
                &mut pointer,
            )
        })?;
        Ok(Mask(Rc::new(MaskOwner {
            pointer: NonNull::new(pointer).ok_or(Error::InvalidInput)?,
            asset: self.clone(),
            source: animation,
        })))
    }
}
