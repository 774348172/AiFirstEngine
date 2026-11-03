//! Semantic bindings share the existing resource identity and lifetime owner.
use super::{ContractError, ContractErrorCode, ProducerStage};
use crate::render_resource::{
    RenderResourceHandle, RenderResourceKind, RenderResourceManager, RenderResourceState,
};
use serde::{Deserialize, Serialize};

#[cfg(test)]
thread_local! {
    static SLOT_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

macro_rules! typed_handle {
    ($name:ident, $kind:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(try_from = "RenderResourceHandle", into = "RenderResourceHandle")]
        pub struct $name(RenderResourceHandle);
        impl TryFrom<RenderResourceHandle> for $name {
            type Error = ContractError;
            fn try_from(value: RenderResourceHandle) -> Result<Self, Self::Error> {
                if value.kind != RenderResourceKind::$kind {
                    return Err(binding_error(ContractErrorCode::ResourceTypeMismatch));
                }
                Ok(Self(value))
            }
        }
        impl From<$name> for RenderResourceHandle {
            fn from(value: $name) -> Self {
                value.0
            }
        }
        impl $name {
            pub fn raw(self) -> RenderResourceHandle {
                self.0
            }
        }
    };
}
typed_handle!(GeometryHandle, MeshBuffer);
typed_handle!(MaterialHandle, MaterialParams);
typed_handle!(TextureHandle, Texture);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderBindingKind {
    Texture,
    Material,
    Mesh,
    FontAtlas,
    RenderTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    tag = "kind",
    content = "handle",
    deny_unknown_fields
)]
pub enum BindingResource {
    Texture(TextureHandle),
    Geometry(GeometryHandle),
    Material(MaterialHandle),
    RenderTarget(RenderResourceHandle),
    Pipeline(RenderResourceHandle),
}

impl BindingResource {
    pub fn raw(&self) -> RenderResourceHandle {
        match self {
            Self::Texture(v) => v.raw(),
            Self::Geometry(v) => v.raw(),
            Self::Material(v) => v.raw(),
            Self::RenderTarget(v) | Self::Pipeline(v) => *v,
        }
    }
    fn from_raw(raw: RenderResourceHandle) -> Self {
        match raw.kind {
            RenderResourceKind::Texture => Self::Texture(TextureHandle(raw)),
            RenderResourceKind::MeshBuffer => Self::Geometry(GeometryHandle(raw)),
            RenderResourceKind::MaterialParams => Self::Material(MaterialHandle(raw)),
            RenderResourceKind::SurfaceFrameTexture => Self::RenderTarget(raw),
            RenderResourceKind::ShaderPipeline => Self::Pipeline(raw),
        }
    }
    pub fn expected_kind(&self) -> RenderResourceKind {
        match self {
            Self::Texture(_) => RenderResourceKind::Texture,
            Self::Geometry(_) => RenderResourceKind::MeshBuffer,
            Self::Material(_) => RenderResourceKind::MaterialParams,
            Self::RenderTarget(_) => RenderResourceKind::SurfaceFrameTexture,
            Self::Pipeline(_) => RenderResourceKind::ShaderPipeline,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamedRenderBinding {
    pub name: String,
    pub resource: BindingResource,
}

impl NamedRenderBinding {
    pub fn new(name: impl Into<String>, handle: RenderResourceHandle) -> Self {
        Self {
            name: name.into(),
            resource: BindingResource::from_raw(handle),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBindingSet {
    pub binding_id: String,
    pub binding_kind: RenderBindingKind,
    pub slots: Vec<NamedRenderBinding>,
    pub sampler: String,
    pub fallback_used: bool,
    pub debug_label: String,
}

impl RenderBindingKind {
    fn primary(self) -> (&'static str, RenderResourceKind) {
        match self {
            Self::Texture => ("baseColorTexture", RenderResourceKind::Texture),
            Self::Material => ("materialParams", RenderResourceKind::MaterialParams),
            Self::Mesh => ("geometry", RenderResourceKind::MeshBuffer),
            Self::FontAtlas => ("fontAtlas", RenderResourceKind::Texture),
            Self::RenderTarget => ("renderTarget", RenderResourceKind::SurfaceFrameTexture),
        }
    }
}

impl RenderBindingSet {
    #[cfg(test)]
    pub(crate) fn take_validation_count_for_test() -> usize {
        SLOT_VALIDATIONS.with(|n| n.replace(0))
    }

    pub fn single(
        binding_id: String,
        binding_kind: RenderBindingKind,
        handle: RenderResourceHandle,
        sampler: String,
        fallback_used: bool,
        debug_label: String,
    ) -> Result<Self, ContractError> {
        let value = Self {
            binding_id,
            binding_kind,
            slots: vec![NamedRenderBinding {
                name: binding_kind.primary().0.into(),
                resource: BindingResource::from_raw(handle),
            }],
            sampler,
            fallback_used,
            debug_label,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn resources(&self) -> impl Iterator<Item = RenderResourceHandle> + '_ {
        self.slots.iter().map(|slot| slot.resource.raw())
    }

    pub fn texture(&self, name: &str) -> Option<TextureHandle> {
        self.validate().ok()?;
        match &self.slots.iter().find(|slot| slot.name == name)?.resource {
            BindingResource::Texture(handle) => Some(*handle),
            _ => None,
        }
    }

    /// Read several slots from the same immutable binding, preserving the
    /// single-slot API's complete fail-closed validation semantics.
    pub(crate) fn texture_slots<const N: usize>(
        &self,
        names: [&str; N],
    ) -> [Option<TextureHandle>; N] {
        if self.validate().is_err() {
            return [None; N];
        }
        names.map(
            |name| match &self.slots.iter().find(|slot| slot.name == name)?.resource {
                BindingResource::Texture(handle) => Some(*handle),
                _ => None,
            },
        )
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        #[cfg(test)]
        SLOT_VALIDATIONS.with(|n| n.set(n.get() + 1));
        let (primary, kind) = self.binding_kind.primary();
        if self.binding_id.trim().is_empty()
            || !self
                .slots
                .iter()
                .any(|slot| slot.name == primary && slot.resource.raw().kind == kind)
        {
            return Err(binding_error(ContractErrorCode::InvalidBinding));
        }
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.name.trim().is_empty() || self.slots[..i].iter().any(|v| v.name == slot.name) {
                return Err(binding_error(ContractErrorCode::InvalidBinding));
            }
            if slot.resource.raw().kind != slot.resource.expected_kind() {
                return Err(binding_error(ContractErrorCode::ResourceTypeMismatch));
            }
            let required = match slot.name.as_str() {
                "baseColorTexture"
                | "normalTexture"
                | "metallicRoughnessTexture"
                | "occlusionTexture"
                | "emissiveTexture"
                | "fontAtlas" => Some(RenderResourceKind::Texture),
                "geometry" => Some(RenderResourceKind::MeshBuffer),
                "materialParams" => Some(RenderResourceKind::MaterialParams),
                "renderTarget" => Some(RenderResourceKind::SurfaceFrameTexture),
                _ => None,
            };
            if required.is_some_and(|kind| slot.resource.raw().kind != kind) {
                return Err(binding_error(ContractErrorCode::ResourceTypeMismatch));
            }
        }
        Ok(())
    }

    /// Only use for handles owned by this manager. External texture registries keep
    /// their own upload readiness; decoding a handle is never residency proof.
    pub fn validate_residency(&self, manager: &RenderResourceManager) -> Result<(), ContractError> {
        self.validate()?;
        for handle in self.resources() {
            validate_resident(handle, manager)?;
        }
        Ok(())
    }
}

pub fn validate_resident(
    handle: RenderResourceHandle,
    manager: &RenderResourceManager,
) -> Result<(), ContractError> {
    let record = manager
        .validate_handle(handle)
        .map_err(|_| binding_error(ContractErrorCode::StaleResourceHandle))?;
    match record.state {
        RenderResourceState::Resident => Ok(()),
        RenderResourceState::Stale
        | RenderResourceState::PendingRelease
        | RenderResourceState::Released => {
            Err(binding_error(ContractErrorCode::StaleResourceHandle))
        }
        _ => Err(binding_error(ContractErrorCode::ResourceNotReady)),
    }
}

// Decode legacy vectors once, then retain only semantic bindings in memory.
// Mixed new/legacy facts are rejected instead of silently selecting a winner.
impl<'de> Deserialize<'de> for RenderBindingSet {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            binding_id: String,
            binding_kind: RenderBindingKind,
            slots: Option<Vec<NamedRenderBinding>>,
            resources: Option<Vec<RenderResourceHandle>>,
            material_handle: Option<RenderResourceHandle>,
            sampler: String,
            fallback_used: bool,
            debug_label: String,
        }
        let wire = Wire::deserialize(deserializer)?;
        let slots = if let Some(slots) = wire.slots {
            if wire.resources.is_some() || wire.material_handle.is_some() {
                return Err(serde::de::Error::custom(
                    "mixed named and legacy binding facts",
                ));
            }
            slots
        } else {
            let mut primary_bound = false;
            let mut slots = wire
                .resources
                .ok_or_else(|| serde::de::Error::custom("missing binding slots/resources"))?
                .into_iter()
                .enumerate()
                .map(|(i, handle)| {
                    let name = if !primary_bound && handle.kind == wire.binding_kind.primary().1 {
                        primary_bound = true;
                        wire.binding_kind.primary().0.into()
                    } else {
                        format!("legacyResource{i}")
                    };
                    NamedRenderBinding {
                        name,
                        resource: BindingResource::from_raw(handle),
                    }
                })
                .collect::<Vec<_>>();
            if let Some(handle) = wire.material_handle {
                slots.push(NamedRenderBinding {
                    name: "legacyMaterial".into(),
                    resource: BindingResource::Material(
                        MaterialHandle::try_from(handle).map_err(serde::de::Error::custom)?,
                    ),
                });
            }
            slots
        };
        let value = Self {
            binding_id: wire.binding_id,
            binding_kind: wire.binding_kind,
            slots,
            sampler: wire.sampler,
            fallback_used: wire.fallback_used,
            debug_label: wire.debug_label,
        };
        value.validate().map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}

fn binding_error(code: ContractErrorCode) -> ContractError {
    ContractError::new(code, "binding.slots").at_stage(ProducerStage::AssetBinding)
}

#[cfg(test)]
mod slot_tests {
    use super::*;

    #[test]
    fn five_slots_share_complete_validation_without_accepting_malformed_bindings() {
        let names = super::super::STANDARD_LIT_TEXTURE_SLOTS;
        let mut binding = RenderBindingSet::single(
            "material".into(),
            RenderBindingKind::Material,
            RenderResourceHandle {
                kind: RenderResourceKind::MaterialParams,
                index: 1,
                generation: 3,
            },
            "linearRepeat".into(),
            false,
            "slot-test".into(),
        )
        .unwrap();
        for (i, name) in names.iter().enumerate() {
            binding.slots.push(NamedRenderBinding::new(
                *name,
                RenderResourceHandle {
                    kind: RenderResourceKind::Texture,
                    index: 2 + i as u64,
                    generation: 7,
                },
            ));
        }
        let expected = names.map(|name| binding.texture(name));
        assert!(expected.iter().all(Option::is_some));
        SLOT_VALIDATIONS.with(|n| n.set(0));
        assert_eq!(binding.texture_slots(names), expected);
        assert_eq!(
            SLOT_VALIDATIONS.with(|n| n.get()),
            1,
            "five texture queries revalidated the entire binding five times"
        );
        let mut missing = binding.clone();
        missing.slots.pop();
        let resolved = missing.texture_slots(names);
        assert_eq!(&resolved[..4], &expected[..4]);
        assert_eq!(resolved[4], None);
        let mut invalid = binding.clone();
        invalid.slots.push(binding.slots[0].clone());
        assert_eq!(
            invalid.texture_slots(names),
            [None; 5],
            "unqueried duplicate material slot bypassed validation"
        );
        invalid = binding.clone();
        invalid.slots[0].resource = BindingResource::RenderTarget(RenderResourceHandle {
            kind: RenderResourceKind::Texture,
            index: 8,
            generation: 1,
        });
        assert_eq!(
            invalid.texture_slots(names),
            [None; 5],
            "unqueried resource-kind mismatch bypassed validation"
        );
        assert_eq!(
            binding.texture_slots(["materialParams", "absent"]),
            [None; 2]
        );
    }
}
