//! Narrow official-state seam. The engine control owner supplies bounded phases.
use crate::{Asset, Error, Instance, Mask, Opaque, native_result, text};
use std::os::raw::c_char;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct EntrySample {
    pub id: u64,
    pub phase: f32,
    pub mix: f32,
}
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct BoneTransform {
    pub x: f32,
    pub y: f32,
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
}
unsafe extern "C" {
    fn s374_tag(instance: *mut Opaque, track: u32, id: u64, alpha: f32) -> i32;
    fn s374_sample(instance: *mut Opaque, samples: *const EntrySample, count: u32) -> i32;
    fn s374_prepare_sample(instance: *mut Opaque, samples: *const EntrySample, count: u32) -> i32;
    fn s374_entries(instance: *mut Opaque, ids: *mut u64) -> u32;
    fn s374_clear(instance: *mut Opaque, track: u32) -> i32;
    fn s374_additive(asset: *mut Opaque, name: *const c_char, mask: *mut Opaque) -> i32;
    fn s374_bone_transform(instance: *mut Opaque, bone: u32, out: *mut BoneTransform) -> i32;
}
impl Asset {
    pub fn validate_additive(&self, name: &str, mask: Option<&Mask>) -> Result<(), Error> {
        let name = text(name)?;
        if mask.is_some_and(|m| !std::rc::Rc::ptr_eq(&self.0, &m.0.asset.0) || m.0.source != name) {
            return Err(Error::WrongAsset);
        }
        // SAFETY: asset-bound immutable mask and call-scoped string.
        native_result(unsafe {
            s374_additive(
                self.ptr(),
                name.as_ptr(),
                mask.map_or(std::ptr::null_mut(), |m| m.0.pointer.as_ptr()),
            )
        })
    }
}
impl Instance {
    pub fn bone_transform(&self, bone: u32) -> Result<BoneTransform, Error> {
        self.ready()?;
        let mut out = BoneTransform::default();
        // SAFETY: native bounds-checks the index and fills call-scoped storage.
        native_result(unsafe { s374_bone_transform(self.pointer.as_ptr(), bone, &mut out) })?;
        Ok(out)
    }
    pub fn tag_current(&mut self, track: u32, id: u64, alpha: f32) -> Result<(), Error> {
        self.ready()?;
        if track >= 4 || id == 0 || !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(Error::InvalidInput);
        }
        // SAFETY: unique live instance; native checks the current entry exists.
        let code = unsafe { s374_tag(self.pointer.as_ptr(), track, id, alpha) };
        self.mutation(code)
    }
    pub fn sample(&mut self, samples: &[EntrySample]) -> Result<(), Error> {
        self.validate_samples(samples)?;
        // SAFETY: complete bounded sample array, uniquely borrowed live instance.
        let code = unsafe {
            s374_sample(
                self.pointer.as_ptr(),
                samples.as_ptr(),
                samples.len() as u32,
            )
        };
        self.mutation(code)
    }
    /// Supplies clocks before an interruption, without a second pose application
    /// or event callback. setCurrent reads the outgoing entry's mix percentage.
    pub fn prepare_sample(&mut self, samples: &[EntrySample]) -> Result<(), Error> {
        self.validate_samples(samples)?;
        // SAFETY: complete validated sample array and uniquely borrowed instance.
        let code = unsafe {
            s374_prepare_sample(
                self.pointer.as_ptr(),
                samples.as_ptr(),
                samples.len() as u32,
            )
        };
        self.mutation(code)
    }
    fn validate_samples(&self, samples: &[EntrySample]) -> Result<(), Error> {
        self.ready()?;
        if samples.len() > 16
            || samples.iter().any(|s| {
                s.id == 0
                    || !s.phase.is_finite()
                    || !(0.0..=3600.0).contains(&s.phase)
                    || !s.mix.is_finite()
                    || !(0.0..=60.0).contains(&s.mix)
            })
        {
            return Err(Error::InvalidInput);
        }
        let ids = self.live_entries()?;
        if ids
            .iter()
            .any(|id| samples.iter().filter(|s| s.id == *id).count() != 1)
        {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
    pub fn live_entries(&self) -> Result<Vec<u64>, Error> {
        self.ready()?;
        let mut ids = [0; 16];
        // SAFETY: native writes at most 16 entries, returning 17 on overflow.
        let count = unsafe { s374_entries(self.pointer.as_ptr(), ids.as_mut_ptr()) };
        if count > 16 {
            return Err(Error::Capacity);
        }
        Ok(ids[..count as usize].to_vec())
    }
    pub fn clear_track(&mut self, track: u32) -> Result<(), Error> {
        self.ready()?;
        if track >= 4 {
            return Err(Error::InvalidInput);
        }
        // SAFETY: unique live instance and bounded track index.
        let code = unsafe { s374_clear(self.pointer.as_ptr(), track) };
        self.mutation(code)
    }
}
