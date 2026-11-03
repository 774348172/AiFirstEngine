//! Prepare-owned typed material facts. Mutation invalidates facts before exposing values.
use super::*;
use std::ops::{Deref, DerefMut};
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub(super) struct PreparedMaterial {
    source: CookedMaterial,
    values: OnceLock<Result<MaterialValues, ContractError>>,
}

#[derive(Debug, Clone)]
pub(super) struct MaterialValues {
    pub base_color: [OrderedF32; 4],
    pub alpha_cutoff: Option<OrderedF32>,
    pub standard_lit: Option<StandardLitParameters>,
}

impl From<CookedMaterial> for PreparedMaterial {
    fn from(source: CookedMaterial) -> Self {
        Self {
            source,
            values: OnceLock::new(),
        }
    }
}
impl Deref for PreparedMaterial {
    type Target = CookedMaterial;
    fn deref(&self) -> &Self::Target {
        &self.source
    }
}
impl DerefMut for PreparedMaterial {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.values.take();
        &mut self.source
    }
}
impl PreparedMaterial {
    pub(super) fn values(&self) -> Result<&MaterialValues, ContractError> {
        self.values
            .get_or_init(|| {
                #[cfg(test)]
                build_inputs::MATERIAL_INTERPRETATIONS.with(|n| n.set(n.get() + 1));
                let Some(MaterialParameter::Vector4(base_color)) =
                    self.source.parameters.get("baseColor")
                else {
                    return Err(invalid("material.baseColor"));
                };
                Ok(MaterialValues {
                    base_color: base_color.map(Into::into),
                    alpha_cutoff: self
                        .source
                        .parameters
                        .get("alphaCutoff")
                        .and_then(|v| match v {
                            MaterialParameter::Float(v) => Some((*v).into()),
                            _ => None,
                        }),
                    standard_lit: self.source.standard_lit(),
                })
            })
            .as_ref()
            .map_err(|e| *e)
    }
}
