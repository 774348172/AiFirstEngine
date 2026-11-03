//! Immutable decoded-pixel identity, computed where the payload is first owned.
use super::RuntimeTexturePayload;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex, Weak};

type DerivedMipMemo = Arc<Mutex<[Option<(Weak<Vec<u8>>, [u8; 32])>; 2]>>;

#[derive(Debug, Clone)]
pub struct VerifiedTexturePayload {
    payload: Arc<RuntimeTexturePayload>,
    pixel_digest: [u8; 32],
    // At most two derived chains (color/data); weak references never pin retirement.
    mips: DerivedMipMemo,
}

impl PartialEq for VerifiedTexturePayload {
    fn eq(&self, other: &Self) -> bool {
        self.payload == other.payload && self.pixel_digest == other.pixel_digest
    }
}
impl Eq for VerifiedTexturePayload {}

impl VerifiedTexturePayload {
    pub(crate) fn new(payload: RuntimeTexturePayload) -> Self {
        let pixel_digest = Sha256::digest(payload.rgba8.as_slice()).into();
        Self {
            payload: Arc::new(payload),
            pixel_digest,
            mips: Arc::new(Mutex::new([None, None])),
        }
    }

    pub(crate) fn payload(&self) -> &Arc<RuntimeTexturePayload> {
        &self.payload
    }

    pub(crate) fn without_pixels(&self) -> Self {
        let mut descriptor = self.clone();
        let payload = Arc::make_mut(&mut descriptor.payload);
        payload.rgba8 = Arc::new(Vec::new());
        descriptor
    }

    pub(crate) fn with_sampling(
        self,
        sampler: String,
    ) -> Result<Self, crate::render_contract::ContractError> {
        self.with_material_sampling(sampler, true)
    }

    pub(crate) fn with_material_sampling(
        mut self,
        sampler: String,
        srgb: bool,
    ) -> Result<Self, crate::render_contract::ContractError> {
        if self.payload.format.starts_with("bc3") {
            let expected = if srgb {
                "bc3RgbaUnormSrgb"
            } else {
                "bc3RgbaUnorm"
            };
            if self.payload.format != expected
                || self.payload.mip_count
                    != 32 - self.payload.width.max(self.payload.height).leading_zeros()
            {
                return Err(crate::render_contract::ContractError::new(
                    crate::render_contract::ContractErrorCode::InvalidAssetPayload,
                    "texture.compressed_color_space_or_mips",
                ));
            }
            Arc::make_mut(&mut self.payload).sampler = sampler;
            return Ok(self);
        }
        let format = if srgb { "rgba8UnormSrgb" } else { "rgba8Unorm" };
        let color_space = if srgb { "srgb" } else { "linear" };
        let space_changed =
            self.payload.format != format || self.payload.color_space != color_space;
        let generate = super::TextureSampler::parse(&sampler)
            .is_some_and(|s| s.min_filter.uses_mips())
            && (self.payload.mip_count == 1 || space_changed);
        if self.payload.sampler != sampler || space_changed {
            let payload = Arc::make_mut(&mut self.payload);
            payload.sampler = sampler;
            payload.format = format.into();
            payload.color_space = color_space.into();
        }
        if generate {
            let mut cache = self.mips.lock().expect("derived mip memo poisoned");
            let slot = &mut cache[usize::from(srgb)];
            let found = slot
                .as_ref()
                .and_then(|(pixels, digest)| pixels.upgrade().map(|pixels| (pixels, *digest)));
            let (pixels, digest) = if let Some(found) = found {
                found
            } else {
                let base_len =
                    super::rgba8_mip_byte_len(self.payload.width, self.payload.height, 1)
                        .ok_or_else(|| {
                            crate::render_contract::ContractError::new(
                                crate::render_contract::ContractErrorCode::InvalidAssetPayload,
                                "texture.mips",
                            )
                        })?;
                let base = self.payload.rgba8.get(..base_len).ok_or_else(|| {
                    crate::render_contract::ContractError::new(
                        crate::render_contract::ContractErrorCode::InvalidAssetPayload,
                        "texture.mips",
                    )
                })?;
                let pixels = Arc::new(
                    super::generate_rgba8_mip_chain(
                        self.payload.width,
                        self.payload.height,
                        base,
                        srgb,
                    )
                    .map_err(|_| {
                        crate::render_contract::ContractError::new(
                            crate::render_contract::ContractErrorCode::InvalidAssetPayload,
                            "texture.mips",
                        )
                    })?,
                );
                let digest = Sha256::digest(pixels.as_slice()).into();
                *slot = Some((Arc::downgrade(&pixels), digest));
                (pixels, digest)
            };
            self.pixel_digest = digest;
            let payload = Arc::make_mut(&mut self.payload);
            payload.rgba8 = pixels;
            payload.mip_count = 32 - payload.width.max(payload.height).leading_zeros();
        }
        Ok(self)
    }

    // An external caller can replace or mutate a PreparedTextureAsset's public
    // Arc. Trust the receipt only while it still points to these exact bytes.
    pub(crate) fn identity_for(&self, payload: &Arc<RuntimeTexturePayload>) -> String {
        let pixel_digest = if Arc::ptr_eq(&self.payload, payload) {
            self.pixel_digest
        } else {
            Sha256::digest(payload.rgba8.as_slice()).into()
        };
        let metadata = serde_json::to_vec(&(
            &payload.asset_id,
            &payload.cooked_asset_id,
            payload.width,
            payload.height,
            &payload.format,
            &payload.color_space,
            payload.mip_count,
            &payload.sampler,
            &payload.source_hash,
        ))
        .expect("typed texture metadata");
        let mut hasher = Sha256::new();
        hasher.update(Sha256::digest(metadata));
        hasher.update(pixel_digest);
        format!("sha256:{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn payload() -> RuntimeTexturePayload {
        RuntimeTexturePayload {
            asset_id: "texture".into(),
            cooked_asset_id: "cooked".into(),
            width: 1,
            height: 1,
            format: "rgba8UnormSrgb".into(),
            color_space: "srgb".into(),
            mip_count: 1,
            rgba8: vec![1, 2, 3, 255].into(),
            sampler: "linearClamp".into(),
            source_hash: "same-source".into(),
        }
    }
    #[test]
    fn bc3_sampling_never_reinterprets_encoded_mips_or_multiplies_storage() {
        let mut p = payload();
        p.width = 4;
        p.height = 4;
        p.mip_count = 3;
        p.format = "bc3RgbaUnormSrgb".into();
        p.rgba8 = vec![0; 48].into();
        let original = VerifiedTexturePayload::new(p);
        let a = original
            .clone()
            .with_material_sampling("nearestClamp".into(), true)
            .unwrap();
        assert!(Arc::ptr_eq(&original.payload().rgba8, &a.payload().rgba8));
        assert!(original
            .clone()
            .with_material_sampling("nearestClamp".into(), false)
            .is_err());
        let mut short = original.payload().as_ref().clone();
        short.mip_count = 1;
        short.rgba8 = vec![0; 16].into();
        assert!(VerifiedTexturePayload::new(short)
            .with_material_sampling("nearestClamp".into(), true)
            .is_err());
    }
    #[test]
    fn immutable_receipt_reuses_storage_but_replacement_pixels_cannot_reuse_identity() {
        let verified = VerifiedTexturePayload::new(payload());
        let original = verified.identity_for(verified.payload());
        let copy = verified.clone();
        assert!(Arc::ptr_eq(verified.payload(), copy.payload()));
        let mut changed = verified.payload().clone();
        Arc::make_mut(&mut Arc::make_mut(&mut changed).rgba8)[0] ^= 1;
        assert_ne!(original, verified.identity_for(&changed));
        assert_eq!(
            verified.identity_for(&changed),
            VerifiedTexturePayload::new(changed.as_ref().clone()).identity_for(&changed)
        );
        Arc::make_mut(&mut changed).sampler = "nearestRepeat".into();
        assert_ne!(original, verified.identity_for(&changed));
        let sampling = copy.with_sampling("nearestRepeat".into()).unwrap();
        assert_eq!(verified.payload().rgba8, sampling.payload().rgba8);
        assert_ne!(original, sampling.identity_for(sampling.payload()));
    }

    #[test]
    fn material_sampler_variants_share_pixels_without_aliasing_metadata() {
        let original = VerifiedTexturePayload::new(payload());
        let variants: Vec<_> = (0..32)
            .map(|_| {
                original
                    .clone()
                    .with_sampling("nearestRepeat".into())
                    .unwrap()
            })
            .collect();
        for variant in &variants {
            assert_eq!(
                original.payload().rgba8.as_ptr(),
                variant.payload().rgba8.as_ptr(),
                "material binding multiplied decoded pixel storage"
            );
            assert_eq!(variant.payload().sampler, "nearestRepeat");
            assert_eq!(original.payload().sampler, "linearClamp");
            assert_ne!(
                original.identity_for(original.payload()),
                variant.identity_for(variant.payload())
            );
        }
        let encoded = serde_json::to_value(original.payload().as_ref()).unwrap();
        assert_eq!(encoded["rgba8"], serde_json::json!([1, 2, 3, 255]));
        let decoded: RuntimeTexturePayload = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, **original.payload());
    }
    #[test]
    fn material_mip_variants_share_actual_pixels_without_pin_or_color_space_alias() {
        let mut p = payload();
        p.width = 2;
        p.height = 1;
        p.rgba8 = vec![0, 0, 0, 255, 255, 255, 255, 255].into();
        let original = VerifiedTexturePayload::new(p);
        let sampler = "nearestMipmapNearest:nearest:clampToEdge:clampToEdge".to_owned();
        let a = original
            .clone()
            .with_material_sampling(sampler.clone(), true)
            .unwrap();
        let b = original
            .clone()
            .with_material_sampling(sampler.clone(), true)
            .unwrap();
        assert!(
            Arc::ptr_eq(&a.payload().rgba8, &b.payload().rgba8),
            "shared material slots duplicate full mip backing"
        );
        let linear = original
            .clone()
            .with_material_sampling(sampler, false)
            .unwrap();
        assert_ne!(a.payload().rgba8, linear.payload().rgba8);
        assert_eq!(&a.payload().rgba8[8..], [188, 188, 188, 255]);
        assert_eq!(&linear.payload().rgba8[8..], [128, 128, 128, 255]);
        let weak = Arc::downgrade(&a.payload().rgba8);
        drop(a);
        drop(b);
        assert!(
            weak.upgrade().is_none(),
            "derived cache must not pin retired mip pixels"
        );
        assert_eq!(original.payload().mip_count, 1);
    }
}
