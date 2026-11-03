//! Ordered local geometry from the official runtime. These are backend output
//! values, awaiting the engine's existing RenderProjection consumer, not a GPU
//! renderer or a second scene/proxy registry.
use crate::{Asset, Error, Instance, Opaque};
use std::{
    hash::{Hash, Hasher},
    rc::Rc,
};

/// Opaque backend equality input, retained only during one projection call.
/// Asset ownership keeps attachment identities alive; hashes never replace Eq.
pub struct GeometryInputKey {
    asset: Asset,
    bytes: Vec<u8>,
}
impl PartialEq for GeometryInputKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.asset.0, &other.asset.0) && self.bytes == other.bytes
    }
}
impl Eq for GeometryInputKey {}
impl Hash for GeometryInputKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.asset.0).hash(state);
        self.bytes.hash(state);
    }
}
impl std::fmt::Debug for GeometryInputKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeometryInputKey")
            .field("bytes", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
    /// Straight tint. The eventual material consumer premultiplies exactly once.
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Additive,
    Multiply,
    Screen,
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct NativeBatch {
    page: u32,
    blend: u32,
    first_index: u32,
    index_count: u32,
}
#[repr(C)]
#[derive(Default)]
struct Counts {
    vertices: u32,
    indices: u32,
    batches: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawBatch {
    pub page: u32,
    pub blend: BlendMode,
    pub first_index: u32,
    pub index_count: u32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<DrawBatch>,
    /// [minX,minY,maxX,maxY] in engine local units, no entity/camera transform.
    pub bounds: Option<[f32; 4]>,
}
unsafe extern "C" {
    fn s374_geometry_input_matches(
        instance: *mut Opaque,
        ppu: f32,
        expected: *const u8,
        length: u32,
    ) -> i32;
    fn s374_geometry_input(
        instance: *mut Opaque,
        ppu: f32,
        out: *mut u8,
        capacity: u32,
        written: *mut u32,
    ) -> i32;
    fn s374_geometry(
        instance: *mut Opaque,
        ppu: f32,
        vertices: *mut Vertex,
        vertex_capacity: u32,
        indices: *mut u32,
        index_capacity: u32,
        batches: *mut NativeBatch,
        batch_capacity: u32,
        counts: *mut Counts,
    ) -> i32;
}
impl Instance {
    /// Compare the complete current input with a retained key from this call.
    /// Failed or unsupported comparisons return false and retain extraction.
    pub fn geometry_input_matches(&self, ppu: f32, key: &GeometryInputKey) -> bool {
        if !Rc::ptr_eq(&self.asset.0, &key.asset.0) || self.ready().is_err() {
            return false;
        }
        // SAFETY: both owners are live and thread confined. Native code reads
        // precisely this slice; it retains no pointer and does not change pose.
        unsafe {
            s374_geometry_input_matches(
                self.pointer.as_ptr(),
                ppu,
                key.bytes.as_ptr(),
                key.bytes.len() as u32,
            ) == 0
        }
    }
    /// A bounded optimization hint; unavailable inputs retain the original path.
    pub fn geometry_input_key(&self, ppu: f32) -> Option<GeometryInputKey> {
        self.geometry_input_key_bounded(ppu, 1024 * 1024)
    }
    fn geometry_input_key_bounded(&self, ppu: f32, limit: u32) -> Option<GeometryInputKey> {
        self.ready().ok()?;
        let mut count = 0;
        // SAFETY: the native owner is live and thread confined. Null requests
        // bounded sizing only; no output pointer is retained or pose modified.
        let status = unsafe {
            s374_geometry_input(
                self.pointer.as_ptr(),
                ppu,
                std::ptr::null_mut(),
                limit,
                &mut count,
            )
        };
        if status != 0 || count > limit {
            return None;
        }
        let mut bytes = vec![0; count as usize];
        let mut written = 0;
        // SAFETY: the buffer has precisely the queried capacity; the instance
        // cannot change between the two calls on this owner thread.
        let status = unsafe {
            s374_geometry_input(
                self.pointer.as_ptr(),
                ppu,
                bytes.as_mut_ptr(),
                count,
                &mut written,
            )
        };
        (status == 0 && written == count).then(|| GeometryInputKey {
            asset: self.asset.clone(),
            bytes,
        })
    }

    pub fn geometry(&self, pixels_per_unit: f32) -> Result<Geometry, Error> {
        let (vertices, indices) = self.geometry_capacity.get();
        let result =
            self.geometry_with_capacity(pixels_per_unit, vertices.max(64), indices.max(192));
        // The official extraction is read-only and transactional. A skin or
        // clipping expansion may outgrow the hint; retry once at the original
        // limits rather than rejecting a frame admitted by the old API.
        let geometry = match result {
            Err(Error::Capacity) if vertices < 32768 || indices < 98304 => {
                self.geometry_with_capacity(pixels_per_unit, 32768, 98304)?
            }
            other => other?,
        };
        self.geometry_capacity.set((
            geometry.vertices.len().next_power_of_two().clamp(64, 32768),
            geometry.indices.len().next_power_of_two().clamp(192, 98304),
        ));
        Ok(geometry)
    }
    /// Capacity can be lowered by a consumer. Native extraction is transactional:
    /// a rejected frame never exposes partially written output buffers.
    pub fn geometry_with_capacity(
        &self,
        ppu: f32,
        vertices: usize,
        indices: usize,
    ) -> Result<Geometry, Error> {
        self.ready()?;
        if !ppu.is_finite() || ppu <= 0.0 || vertices > 32768 || indices > 98304 {
            return Err(Error::InvalidInput);
        }
        let mut v = vec![Vertex::default(); vertices];
        let mut ix = vec![0u32; indices];
        let mut batches = vec![NativeBatch::default(); 256];
        let mut counts = Counts::default();
        // SAFETY: the live instance is thread confined; all output arrays are
        // initialized and exactly match the supplied capacities. Native does not
        // mutate the pose and never retains these output pointers.
        let code = unsafe {
            s374_geometry(
                self.pointer.as_ptr(),
                ppu,
                v.as_mut_ptr(),
                vertices as u32,
                ix.as_mut_ptr(),
                indices as u32,
                batches.as_mut_ptr(),
                256,
                &mut counts,
            )
        };
        match code {
            0 => {}
            2 => return Err(Error::Unsupported),
            4 => return Err(Error::Capacity),
            _ => return Err(Error::Native(format!("geometry status {code}"))),
        }
        if counts.vertices as usize > v.len()
            || counts.indices as usize > ix.len()
            || counts.batches as usize > batches.len()
        {
            return Err(Error::Native("invalid native geometry counts".into()));
        }
        v.truncate(counts.vertices as usize);
        ix.truncate(counts.indices as usize);
        batches.truncate(counts.batches as usize);
        let batches = batches
            .into_iter()
            .map(|b| {
                Ok(DrawBatch {
                    page: b.page,
                    blend: match b.blend {
                        0 => BlendMode::Normal,
                        1 => BlendMode::Additive,
                        2 => BlendMode::Multiply,
                        3 => BlendMode::Screen,
                        _ => return Err(Error::Unsupported),
                    },
                    first_index: b.first_index,
                    index_count: b.index_count,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let bounds = v.first().map(|first| {
            v.iter()
                .skip(1)
                .fold([first.x, first.y, first.x, first.y], |b, v| {
                    [b[0].min(v.x), b[1].min(v.y), b[2].max(v.x), b[3].max(v.y)]
                })
        });
        Ok(Geometry {
            vertices: v,
            indices: ix,
            batches,
            bounds,
        })
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use std::collections::BTreeMap;
    const SOURCE: &str = include_str!("../tests/fixtures/probe.json");
    const ATLAS: &str = include_str!("../tests/fixtures/probe.atlas");
    fn asset() -> Asset {
        crate::import::validate(
            SOURCE,
            ATLAS,
            &BTreeMap::from([("probe.png".into(), "texture".into())]),
        )
        .unwrap()
        .load()
        .unwrap()
    }
    #[test]
    fn spine374_f_geometry_input_equality_retains_asset_and_separates_pose_units() {
        let asset = asset();
        let mut a = asset.instance().unwrap();
        let mut b = asset.instance().unwrap();
        assert_eq!(a.geometry_input_key(100.), b.geometry_input_key(100.));
        assert_ne!(a.geometry_input_key(100.), a.geometry_input_key(200.));
        let other = super::input_tests::asset().instance().unwrap();
        assert_ne!(a.geometry_input_key(100.), other.geometry_input_key(100.));
        a.play(0, "motion", None, 0., true, false).unwrap();
        b.play(0, "motion", None, 0., true, false).unwrap();
        a.step(0.25).unwrap();
        b.step(0.25).unwrap();
        assert_eq!(a.geometry_input_key(100.), b.geometry_input_key(100.));
        assert!(b.geometry_input_matches(100., &a.geometry_input_key(100.).unwrap()));
        assert_eq!(a.geometry(100.).unwrap(), b.geometry(100.).unwrap());
        b.step(0.25).unwrap();
        assert_ne!(a.geometry_input_key(100.), b.geometry_input_key(100.));
        assert!(!b.geometry_input_matches(100., &a.geometry_input_key(100.).unwrap()));
        assert_ne!(a.geometry(100.).unwrap(), b.geometry(100.).unwrap());
        let key = a.geometry_input_key(100.).unwrap();
        drop(a);
        drop(b);
        drop(asset);
        // Full Eq and Hash remain valid after all instance owners disappear.
        assert!(std::collections::HashSet::from([key]).len() == 1);
    }
    #[test]
    fn spine374_f_geometry_input_comparison_checks_every_byte_length_asset_and_units() {
        let asset = asset();
        let instance = asset.instance().unwrap();
        let mut key = instance.geometry_input_key(100.).unwrap();
        let before = instance.geometry(100.).unwrap();
        assert!(instance.geometry_input_matches(100., &key));
        for index in 0..key.bytes.len() {
            key.bytes[index] ^= 1;
            assert!(!instance.geometry_input_matches(100., &key), "byte {index}");
            key.bytes[index] ^= 1;
        }
        let last = key.bytes.pop().unwrap();
        assert!(!instance.geometry_input_matches(100., &key));
        key.bytes.push(last);
        key.bytes.push(0);
        assert!(!instance.geometry_input_matches(100., &key));
        key.bytes.pop();
        assert!(!instance.geometry_input_matches(200., &key));
        assert!(!instance.geometry_input_matches(f32::NAN, &key));
        let foreign = super::input_tests::asset().instance().unwrap();
        assert!(!foreign.geometry_input_matches(100., &key));
        assert_eq!(instance.geometry(100.).unwrap(), before);
        key.bytes.clear();
        assert!(!instance.geometry_input_matches(100., &key));
    }
    #[test]
    fn spine374_f_geometry_input_capacity_falls_back_without_rejecting_geometry() {
        let instance = asset().instance().unwrap();
        assert!(instance.geometry_input_key_bounded(100., 16).is_none());
        assert!(instance.geometry_input_key(100.).is_some());
        assert!(instance.geometry_input_key(f32::NAN).is_none());
        assert!(!instance.geometry(100.).unwrap().vertices.is_empty());
    }
    #[test]
    fn spine374_f_geometry_input_covers_tint_attachment_deform_and_draw_order() {
        use serde_json::json;
        let mut value: serde_json::Value = serde_json::from_str(SOURCE).unwrap();
        let mut second = value["slots"][0].clone();
        second["name"] = json!("second");
        value["slots"].as_array_mut().unwrap().push(second);
        let attachment = value["skins"][0]["attachments"]["cloth"]["triangle"].clone();
        let mut skin = value["skins"][0].clone();
        skin["name"] = json!("alternate");
        value["skins"].as_array_mut().unwrap().push(skin);
        value["skins"][0]["attachments"]["second"] = json!({"triangle":attachment});
        value["skins"][0]["attachments"]["cloth"]["alternate"] = attachment;
        value["animations"]["tint"] = json!({"slots":{"cloth":{"rgba":[{"color":"80ff80ff"}]}}});
        value["animations"]["detach"] =
            json!({"slots":{"cloth":{"attachment":[{"name":"alternate"}]}}});
        value["animations"]["order"] =
            json!({"drawOrder":[{"offsets":[{"slot":"cloth","offset":1}]}]});
        value["animations"]["deformOnly"] = json!({"attachments":{"default":{"cloth":{"triangle":{"deform":[{"vertices":[1.,0.,0.,0.,0.,0.]}]}}}}});
        let asset = crate::import::validate(
            &value.to_string(),
            ATLAS,
            &BTreeMap::from([("probe.png".into(), "texture".into())]),
        )
        .unwrap()
        .load()
        .unwrap();
        let base = asset.instance().unwrap();
        let base_key = base.geometry_input_key(100.).unwrap();
        let mut skinned = asset.instance().unwrap();
        skinned.set_skin("alternate").unwrap();
        assert_ne!(Some(&base_key), skinned.geometry_input_key(100.).as_ref());
        for clip in ["tint", "detach", "order", "deformOnly"] {
            let mut i = asset.instance().unwrap();
            i.play(0, clip, None, 0., false, false).unwrap();
            i.step(0.).unwrap();
            assert!(!i.geometry_input_matches(100., &base_key), "{clip}");
            assert_ne!(
                Some(&base_key),
                i.geometry_input_key(100.).as_ref(),
                "{clip}"
            );
        }
    }

    #[test]
    fn spine374_f_geometry_input_hash_collision_still_requires_full_equality() {
        #[derive(Default)]
        struct Collision;
        impl std::hash::Hasher for Collision {
            fn finish(&self) -> u64 {
                0
            }
            fn write(&mut self, _: &[u8]) {}
        }
        let instance = asset().instance().unwrap();
        let key = instance.geometry_input_key(100.).unwrap();
        let mut keys =
            std::collections::HashSet::<_, std::hash::BuildHasherDefault<Collision>>::default();
        for index in 0..key.bytes.len() {
            let mut bytes = key.bytes.clone();
            bytes[index] ^= 1;
            assert!(keys.insert(GeometryInputKey {
                asset: key.asset.clone(),
                bytes
            }));
        }
        assert!(keys.insert(instance.geometry_input_key(100.).unwrap()));
        assert!(!keys.insert(instance.geometry_input_key(100.).unwrap()));
    }
}
