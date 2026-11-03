//! Pinned Spine 4.2 backend: safe resource admission, geometry and the narrow
//! official-state seam consumed by the engine's integer-tick control owner.
//!
//! Native data and instances are deliberately `!Send + !Sync`. Assets retain atlas
//! metadata; instances retain assets and all masks used by their track histories.
mod events;
mod geometry;
pub mod import;
mod mask;
pub use mask::MaskPlan;
mod controlled;
pub use controlled::{BoneTransform, EntrySample};
pub use events::{ContentEvent, EventCursor, EventIndex, EventKey, OncePlan, seconds_to_ticks};
pub use geometry::{BlendMode, DrawBatch, Geometry, GeometryInputKey, Vertex};

use std::{ffi::CString, os::raw::c_char, ptr::NonNull, rc::Rc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidInput,
    UnknownName,
    Unsupported,
    Capacity,
    WrongAsset,
    StaleHandle,
    Native(String),
    Isolated,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct BonePose {
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct NativeEventKey {
    seconds: f32,
    ordinal: u32,
}

// Opaque C++ objects, only dereferenced by the native owner.
type Opaque = std::ffi::c_void;
unsafe extern "C" {
    fn s374_asset_create(
        json: *const c_char,
        atlas: *const c_char,
        out: *mut *mut Opaque,
        error: *mut c_char,
    ) -> i32;
    fn s374_asset_destroy(asset: *mut Opaque);
    fn s374_atlas_pages(asset: *mut Opaque) -> u32;
    fn s374_duration(asset: *mut Opaque, animation: *const c_char) -> f32;
    fn s374_event_keys(
        asset: *mut Opaque,
        animation: *const c_char,
        out: *mut NativeEventKey,
        capacity: u32,
    ) -> u32;
    fn s374_mask_create(
        asset: *mut Opaque,
        animation: *const c_char,
        bits: u64,
        out: *mut *mut Opaque,
    ) -> i32;
    fn s374_mask_destroy(mask: *mut Opaque);
    fn s374_mask_timelines(mask: *mut Opaque) -> u32;
    fn s374_mask_property(
        asset: *mut Opaque,
        mask: *mut Opaque,
        animation: *const c_char,
        timeline: u32,
    ) -> i32;
    fn s374_instance_create(asset: *mut Opaque, out: *mut *mut Opaque) -> i32;
    fn s374_instance_destroy(instance: *mut Opaque);
    fn s374_play(
        instance: *mut Opaque,
        track: u32,
        animation: *const c_char,
        mask: *mut Opaque,
        mix: f32,
        looping: i32,
        additive: i32,
    ) -> i32;
    fn s374_empty(instance: *mut Opaque, track: u32, mix: f32) -> i32;
    fn s374_step(instance: *mut Opaque, dt: f32) -> i32;
    fn s374_bone(instance: *mut Opaque, bone: *const c_char, out: *mut BonePose) -> i32;
    fn s374_vertices(
        instance: *mut Opaque,
        slot: *const c_char,
        out: *mut f32,
        capacity: u32,
    ) -> i32;
    fn s374_clock(instance: *mut Opaque, track: u32) -> f32;
    fn s374_local_clock(instance: *mut Opaque, track: u32) -> f32;
    fn s374_callback_events(instance: *mut Opaque) -> u32;
    fn s374_live_allocations() -> u64;
    fn s374_skin(instance: *mut Opaque, name: *const c_char) -> i32;
    fn s374_mask_used(instance: *mut Opaque, mask: *mut Opaque) -> i32;
}

fn text(value: &str) -> Result<CString, Error> {
    CString::new(value).map_err(|_| Error::InvalidInput)
}

fn native_result(code: i32) -> Result<(), Error> {
    match code {
        0 => Ok(()),
        1 => Err(Error::UnknownName),
        2 => Err(Error::Unsupported),
        _ => Err(Error::Native(format!("native status {code}"))),
    }
}

struct AssetOwner(NonNull<Opaque>, [u8; 32]);
impl Drop for AssetOwner {
    fn drop(&mut self) {
        // SAFETY: exactly one owner; instances/masks retain the Rc until retired.
        unsafe { s374_asset_destroy(self.0.as_ptr()) };
    }
}

#[derive(Clone)]
pub struct Asset(Rc<AssetOwner>);
impl Asset {
    /// Parses already validated Spine 4.2 JSON and atlas metadata, without file I/O.
    ///
    /// # Safety
    /// The caller must validate all structural references, lengths, finite values,
    /// and timeline/attachment types according to the pinned official parser.
    /// That parser assumes valid exported data and is not a hostile-input sandbox.
    /// Gate A uses only reviewed, project-authored test fixtures. Gate B owns the
    /// production importer; this unsafe entry must never become an unchecked tool.
    pub unsafe fn from_validated_json(json: &str, atlas: &str) -> Result<Self, Error> {
        use sha2::{Digest, Sha256};
        let identity: [u8; 32] = Sha256::digest(json.as_bytes()).into();
        let (json, atlas) = (text(json)?, text(atlas)?);
        let mut pointer = std::ptr::null_mut();
        let mut error = [0 as c_char; 256];
        // SAFETY: input validity is the caller's obligation; strings and output
        // storage remain live during the call. Native copies all input data.
        let code = unsafe {
            s374_asset_create(
                json.as_ptr(),
                atlas.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
            )
        };
        if code != 0 {
            // SAFETY: native writes a null-terminated message into this buffer.
            let message = unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            return Err(Error::Native(message));
        }
        let pointer = NonNull::new(pointer).ok_or(Error::InvalidInput)?;
        Ok(Self(Rc::new(AssetOwner(pointer, identity))))
    }
    fn ptr(&self) -> *mut Opaque {
        self.0.0.as_ptr()
    }
    pub fn atlas_pages(&self) -> u32 {
        // SAFETY: Rc retains the asset, and native only reads its atlas.
        unsafe { s374_atlas_pages(self.ptr()) }
    }
    pub fn duration(&self, animation: &str) -> Result<f32, Error> {
        let animation = text(animation)?;
        // SAFETY: live asset and call-scoped C string.
        let duration = unsafe { s374_duration(self.ptr(), animation.as_ptr()) };
        if duration < 0.0 {
            Err(Error::UnknownName)
        } else {
            Ok(duration)
        }
    }
    pub fn event_index(&self, animation: &str) -> Result<EventIndex, Error> {
        let duration = self.duration(animation)?;
        let animation = text(animation)?;
        // SAFETY: zero capacity forbids writes to the null output pointer.
        let count =
            unsafe { s374_event_keys(self.ptr(), animation.as_ptr(), std::ptr::null_mut(), 0) };
        if count > 4096 {
            return Err(Error::Capacity);
        }
        let mut keys = vec![NativeEventKey::default(); count as usize];
        // SAFETY: array has exactly the immutable event count queried above.
        unsafe { s374_event_keys(self.ptr(), animation.as_ptr(), keys.as_mut_ptr(), count) };
        EventIndex::new(
            duration,
            keys.into_iter().map(|key| (key.seconds, key.ordinal)),
        )
    }
    /// Legacy 64-bit feasibility seam. Production uses `ValidatedSource::mask_plan`
    /// and `bind_mask`, which validate explicit 256-bone dependency selections.
    pub fn mask(&self, animation: &str, bone_bits: u64) -> Result<Mask, Error> {
        let animation = text(animation)?;
        let mut pointer = std::ptr::null_mut();
        // SAFETY: live asset; native creates independent owned timelines.
        native_result(unsafe {
            s374_mask_create(self.ptr(), animation.as_ptr(), bone_bits, &mut pointer)
        })?;
        Ok(Mask(Rc::new(MaskOwner {
            pointer: NonNull::new(pointer).ok_or(Error::InvalidInput)?,
            asset: self.clone(),
            source: animation,
        })))
    }
    pub fn instance(&self) -> Result<Instance, Error> {
        let mut pointer = std::ptr::null_mut();
        // SAFETY: native instance borrows asset, retained by the Rust instance.
        native_result(unsafe { s374_instance_create(self.ptr(), &mut pointer) })?;
        Ok(Instance {
            pointer: NonNull::new(pointer).ok_or(Error::InvalidInput)?,
            asset: self.clone(),
            masks: Vec::new(),
            isolated: false,
            geometry_capacity: Default::default(),
        })
    }
}

struct MaskOwner {
    pointer: NonNull<Opaque>,
    asset: Asset,
    source: CString,
}
impl Drop for MaskOwner {
    fn drop(&mut self) {
        // SAFETY: sole native mask owner; parent asset is still alive.
        unsafe { s374_mask_destroy(self.pointer.as_ptr()) };
    }
}
#[derive(Clone)]
pub struct Mask(Rc<MaskOwner>);
impl Mask {
    pub fn timeline_count(&self) -> u32 {
        // SAFETY: Rc retains the mask.
        unsafe { s374_mask_timelines(self.0.pointer.as_ptr()) }
    }
    pub fn contains_source_property(&self, animation: &str, timeline: u32) -> Result<bool, Error> {
        let animation = text(animation)?;
        // SAFETY: mask retains its asset; index is checked by native code.
        let result = unsafe {
            s374_mask_property(
                self.0.asset.ptr(),
                self.0.pointer.as_ptr(),
                animation.as_ptr(),
                timeline,
            )
        };
        match result {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::InvalidInput),
        }
    }
}

pub struct Instance {
    pointer: NonNull<Opaque>,
    asset: Asset,
    masks: Vec<Mask>,
    isolated: bool,
    // Allocation sizes only: no pose, skin, geometry or borrowed native data.
    geometry_capacity: std::cell::Cell<(usize, usize)>,
}
impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: state/history is destroyed before retained masks and asset.
        unsafe { s374_instance_destroy(self.pointer.as_ptr()) };
    }
}
impl Instance {
    fn retire_masks(&mut self) {
        let instance = self.pointer.as_ptr();
        // SAFETY: both the native state and retained masks are live; read only.
        self.masks
            .retain(|m| unsafe { s374_mask_used(instance, m.0.pointer.as_ptr()) } != 0);
    }
    /// Explicit skin change clears old slot/deform state, then reapplies the
    /// current official pose without advancing time. Unknown skins do not mutate.
    pub fn set_skin(&mut self, name: &str) -> Result<(), Error> {
        self.ready()?;
        let name = text(name)?;
        // SAFETY: live uniquely borrowed instance and call-scoped name.
        let code = unsafe { s374_skin(self.pointer.as_ptr(), name.as_ptr()) };
        self.mutation(code)
    }
    fn ready(&self) -> Result<(), Error> {
        if self.isolated {
            Err(Error::Isolated)
        } else {
            Ok(())
        }
    }
    fn mutation(&mut self, code: i32) -> Result<(), Error> {
        if code >= 3 {
            self.isolated = true;
        }
        native_result(code)
    }
    pub fn play(
        &mut self,
        track: u32,
        animation: &str,
        mask: Option<&Mask>,
        mix: f32,
        looping: bool,
        additive: bool,
    ) -> Result<(), Error> {
        self.ready()?;
        if track >= 8 || !mix.is_finite() || !(0.0..=60.0).contains(&mix) {
            return Err(Error::InvalidInput);
        }
        // Validate the name even when a mask is supplied.
        self.asset.duration(animation)?;
        let animation = text(animation)?;
        let mut native_mask = std::ptr::null_mut();
        self.retire_masks();
        if let Some(mask) = mask {
            if !Rc::ptr_eq(&self.asset.0, &mask.0.asset.0) {
                return Err(Error::WrongAsset);
            }
            if mask.0.source != animation {
                return Err(Error::InvalidInput);
            }
            if !self.masks.iter().any(|kept| Rc::ptr_eq(&kept.0, &mask.0)) {
                self.masks.push(mask.clone());
            }
            native_mask = mask.0.pointer.as_ptr();
        }
        // SAFETY: live instance/name/mask; all numeric inputs are bounded. Mask is
        // retained for the entire history, including interrupted mixing chains.
        let code = unsafe {
            s374_play(
                self.pointer.as_ptr(),
                track,
                animation.as_ptr(),
                native_mask,
                mix,
                looping.into(),
                additive.into(),
            )
        };
        let result = self.mutation(code);
        self.retire_masks();
        result
    }
    pub fn fade_out(&mut self, track: u32, mix: f32) -> Result<(), Error> {
        self.ready()?;
        if track >= 8 || !mix.is_finite() || !(0.0..=60.0).contains(&mix) {
            return Err(Error::InvalidInput);
        }
        // SAFETY: validated values, uniquely borrowed instance.
        let code = unsafe { s374_empty(self.pointer.as_ptr(), track, mix) };
        self.mutation(code)
    }
    /// Low-level feasibility delta, not the eventual authoritative tick clock.
    pub fn step(&mut self, dt: f32) -> Result<(), Error> {
        self.ready()?;
        if !dt.is_finite() || !(0.0..=60.0).contains(&dt) {
            return Err(Error::InvalidInput);
        }
        // SAFETY: live uniquely borrowed instance; no Rust callback can reenter it.
        let code = unsafe { s374_step(self.pointer.as_ptr(), dt) };
        self.mutation(code)
    }
    pub fn bone(&self, name: &str) -> Result<BonePose, Error> {
        self.ready()?;
        let name = text(name)?;
        let mut pose = BonePose::default();
        // SAFETY: live instance, name and writable pose; native checks the bone.
        native_result(unsafe { s374_bone(self.pointer.as_ptr(), name.as_ptr(), &mut pose) })?;
        Ok(pose)
    }
    pub fn vertices(&self, slot: &str) -> Result<Vec<f32>, Error> {
        self.ready()?;
        let slot = text(slot)?;
        let mut values = vec![0.0; 8192];
        // SAFETY: native bounds-checks count before writing to the array.
        let count = unsafe {
            s374_vertices(
                self.pointer.as_ptr(),
                slot.as_ptr(),
                values.as_mut_ptr(),
                values.len() as u32,
            )
        };
        if count < 0 {
            return Err(if count == -2 {
                Error::Capacity
            } else {
                Error::UnknownName
            });
        }
        values.truncate(count as usize);
        Ok(values)
    }
    pub fn clocks(&self, track: u32) -> Result<(f32, f32), Error> {
        self.ready()?;
        if track >= 8 {
            return Err(Error::InvalidInput);
        }
        // SAFETY: native getCurrent checks for missing tracks; live instance.
        let values = unsafe {
            (
                s374_clock(self.pointer.as_ptr(), track),
                s374_local_clock(self.pointer.as_ptr(), track),
            )
        };
        if values.0 < 0.0 {
            Err(Error::UnknownName)
        } else {
            Ok(values)
        }
    }
    /// Diagnostic comparison only. Never publish these as project content events.
    pub fn callback_event_count(&self) -> u32 {
        // SAFETY: live instance; native reads only a compact counter.
        unsafe { s374_callback_events(self.pointer.as_ptr()) }
    }
}

/// Allocation accounting covers official allocator calls, not STL/Rust heaps.
pub fn live_native_allocations() -> u64 {
    // SAFETY: process-wide atomic diagnostic; no borrowed pointer crosses FFI.
    unsafe { s374_live_allocations() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceHandle {
    registry: u64,
    index: usize,
    generation: u64,
}
pub struct Instances {
    identity: u64,
    slots: Vec<(u64, Option<Instance>)>,
}
impl Default for Instances {
    fn default() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let identity = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("registry identity exhausted");
        Self {
            identity,
            slots: Vec::new(),
        }
    }
}
impl Instances {
    pub fn insert(&mut self, instance: Instance) -> InstanceHandle {
        let index = self
            .slots
            .iter()
            .position(|slot| slot.1.is_none() && slot.0 != u64::MAX)
            .unwrap_or(self.slots.len());
        if index == self.slots.len() {
            self.slots.push((0, None));
        }
        self.slots[index].1 = Some(instance);
        InstanceHandle {
            registry: self.identity,
            index,
            generation: self.slots[index].0,
        }
    }
    pub fn get_mut(&mut self, handle: InstanceHandle) -> Result<&mut Instance, Error> {
        if handle.registry != self.identity {
            return Err(Error::StaleHandle);
        }
        let slot = self.slots.get_mut(handle.index).ok_or(Error::StaleHandle)?;
        if slot.0 != handle.generation {
            return Err(Error::StaleHandle);
        }
        slot.1.as_mut().ok_or(Error::StaleHandle)
    }
    pub fn remove(&mut self, handle: InstanceHandle) -> Result<(), Error> {
        self.get_mut(handle)?;
        let slot = &mut self.slots[handle.index];
        slot.1 = None;
        slot.0 += 1; // MAX slots are retired permanently and never reused.
        Ok(())
    }
}
