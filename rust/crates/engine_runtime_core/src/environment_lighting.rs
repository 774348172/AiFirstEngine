//! Immutable cooked cubemap IBL. Asset bytes and decoded backing stay in the existing loader.
use crate::render_contract::{ContractError, ContractErrorCode};
use crate::render_graph::OrderedF32;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const ENVIRONMENT_MAGIC: u32 = 0x314c4249;
pub const ENVIRONMENT_HEADER_WORDS: usize = 32;
pub const ENVIRONMENT_LUT_SIZE: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvironmentMap {
    #[serde(skip)]
    content_validation: crate::render_geometry::ContentValidation<Self>,
    words: Vec<u32>,
    identity: String,
}
impl EnvironmentMap {
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, String> {
        if bytes.len() < ENVIRONMENT_HEADER_WORDS * 4
            || bytes.len() > 512 * 1024
            || bytes.len() % 4 != 0
        {
            return Err("environment.byte_capacity".into());
        }
        let words: Vec<_> = bytes
            .chunks_exact(4)
            .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
            .collect();
        Self::validate_words(&words)?;
        Ok(Arc::new(Self {
            content_validation: Default::default(),
            words,
            identity: crate::canonical_digest::sha256_prefixed(bytes),
        }))
    }
    fn validate_words(words: &[u32]) -> Result<(), String> {
        if words.len() < 32
            || words[0] != ENVIRONMENT_MAGIC
            || !(2..=64).contains(&words[1])
            || !words[1].is_power_of_two()
            || words[2] != words[1].ilog2() + 1
            || words[3] != ENVIRONMENT_LUT_SIZE as u32
            || words[31] != 0
        {
            return Err("environment.layout".into());
        }
        let mut size = words[1] as usize;
        let mut count = ENVIRONMENT_HEADER_WORDS + ENVIRONMENT_LUT_SIZE * ENVIRONMENT_LUT_SIZE * 2;
        while size > 0 {
            count += 6 * size * size * 3;
            size /= 2;
        }
        if words.len() != count
            || words[4..31].iter().any(|w| !f32::from_bits(*w).is_finite())
            || words[32..].iter().any(|w| {
                let v = f32::from_bits(*w);
                !v.is_finite() || !(0.0..=65504.0).contains(&v)
            })
        {
            return Err("environment.payload".into());
        }
        let lut = &words[count - ENVIRONMENT_LUT_SIZE * ENVIRONMENT_LUT_SIZE * 2..];
        if lut.iter().any(|w| f32::from_bits(*w) > 1.001) {
            return Err("environment.brdf_lut".into());
        }
        Ok(())
    }
    pub fn words(&self) -> &[u32] {
        &self.words
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub(crate) fn validate_shared(self: &Arc<Self>) -> Result<(), ContractError> {
        self.content_validation.validate(self, || self.validate())
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        Self::validate_words(&self.words).map_err(|_| {
            ContractError::new(ContractErrorCode::InvalidBinding, "environment.payload")
        })?;
        // Wire input cannot supply an arbitrary identity for different immutable bytes.
        #[cfg(test)]
        CONTENT_HASH_WORDS.fetch_add(self.words.len(), std::sync::atomic::Ordering::Relaxed);
        let bytes: Vec<_> = self.words.iter().flat_map(|v| v.to_le_bytes()).collect();
        if crate::canonical_digest::sha256_prefixed(&bytes) != self.identity {
            return Err(ContractError::new(
                ContractErrorCode::AssetDigestMismatch,
                "environment.identity",
            ));
        }
        Ok(())
    }
    pub(crate) fn observe_memory(
        &self,
        s: &mut crate::resource_memory::ResourceMemorySample,
        role: &str,
    ) {
        s.cpu_vec(&self.words, "assets", &[role]);
        s.cpu_string(&self.identity, "assets", &[role]);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvironmentLighting {
    pub map: Arc<EnvironmentMap>,
    pub intensity: OrderedF32,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn environment_rejects_wrong_layout_nonfinite_and_truncated_backing() {
        assert!(EnvironmentMap::from_bytes(&[]).is_err());
        let mut w = vec![0u32; 32 + 6 * (4 + 1) * 3 + 32 * 32 * 2];
        w[..4].copy_from_slice(&[ENVIRONMENT_MAGIC, 2, 2, 32]);
        let bytes = |w: &[u32]| w.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>();
        EnvironmentMap::from_bytes(&bytes(&w))
            .unwrap()
            .validate()
            .unwrap();
        w[32] = f32::NAN.to_bits();
        assert!(EnvironmentMap::from_bytes(&bytes(&w)).is_err());
        w[32] = (-1f32).to_bits();
        assert!(EnvironmentMap::from_bytes(&bytes(&w)).is_err());
        w[32] = 0;
        w[1] = 3;
        assert!(EnvironmentMap::from_bytes(&bytes(&w)).is_err());
        w[1] = 2;
        w.pop();
        assert!(EnvironmentMap::from_bytes(&bytes(&w)).is_err());
    }
}

#[cfg(test)]
static CONTENT_HASH_WORDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
mod shared_content_tests {
    use super::*;
    fn map() -> Arc<EnvironmentMap> {
        let mut words = vec![0u32; 32 + 6 * (4 + 1) * 3 + 32 * 32 * 2];
        words[..4].copy_from_slice(&[ENVIRONMENT_MAGIC, 2, 2, 32]);
        EnvironmentMap::from_bytes(
            &words
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }
    #[test]
    fn shared_environment_content_hashes_once_across_tasks_at_multiple_scales() {
        for count in [4, 32, 256] {
            let map = map();
            let old = serde_json::to_value(&map).unwrap();
            CONTENT_HASH_WORDS.store(0, std::sync::atomic::Ordering::Relaxed);
            std::thread::scope(|scope| {
                for _ in 0..4 {
                    let map = &map;
                    scope.spawn(move || {
                        for _ in 0..count {
                            map.validate_shared().unwrap();
                        }
                    });
                }
            });
            let actual = CONTENT_HASH_WORDS.load(std::sync::atomic::Ordering::Relaxed);
            eprintln!("shared environment scale={count} actual hashed words={actual} one_content_words={}",map.words.len());
            assert_eq!(
                actual,
                map.words.len(),
                "same immutable environment was hashed in every consumer"
            );
            assert_eq!(
                serde_json::to_value(&map).unwrap(),
                old,
                "private success leaked onto wire"
            );
        }
    }
    #[test]
    fn shared_environment_keeps_public_strict_cow_wire_and_weak_dissociation_checks() {
        let map = map();
        map.validate_shared().unwrap();
        CONTENT_HASH_WORDS.store(0, std::sync::atomic::Ordering::Relaxed);
        map.validate().unwrap();
        assert_eq!(
            CONTENT_HASH_WORDS.load(std::sync::atomic::Ordering::Relaxed),
            map.words.len()
        );
        let mut changed = map.clone();
        Arc::make_mut(&mut changed).words[32] = 1f32.to_bits();
        assert_eq!(
            changed.validate_shared().unwrap_err(),
            changed.validate().unwrap_err()
        );
        map.validate_shared().unwrap();
        let mut wire = serde_json::to_value(&map).unwrap();
        wire["identity"] = "bad digest".into();
        let decoded: Arc<EnvironmentMap> = Arc::new(serde_json::from_value(wire).unwrap());
        for _ in 0..2 {
            assert_eq!(
                decoded.validate_shared().unwrap_err(),
                decoded.validate().unwrap_err()
            );
        }
        let mut moved = super::shared_content_tests::map().as_ref().clone();
        let mut owner = Arc::new(moved.clone());
        owner.validate_shared().unwrap();
        // Sole strong owner plus retained Weak must dissociate on make_mut.
        Arc::make_mut(&mut owner).words[32] = f32::NAN.to_bits();
        assert_eq!(
            owner.validate_shared().unwrap_err(),
            owner.validate().unwrap_err()
        );
        moved.identity = "also invalid".into();
        assert!(Arc::new(moved).validate_shared().is_err());
    }
}
