//! Typed sampling settings carried through the existing texture/binding owner.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextureFilter {
    Nearest,
    Linear,
    NearestMipmapNearest,
    LinearMipmapNearest,
    NearestMipmapLinear,
    LinearMipmapLinear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextureWrap {
    Repeat,
    ClampToEdge,
    MirroredRepeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextureSampler {
    pub min_filter: TextureFilter,
    pub mag_filter: TextureFilter,
    pub wrap_s: TextureWrap,
    pub wrap_t: TextureWrap,
}

impl TextureFilter {
    pub fn uses_mips(self) -> bool {
        !matches!(self, Self::Nearest | Self::Linear)
    }
    pub fn base_filter(self) -> Self {
        match self {
            Self::Nearest | Self::NearestMipmapNearest | Self::NearestMipmapLinear => Self::Nearest,
            _ => Self::Linear,
        }
    }
    pub fn mip_filter(self) -> Self {
        match self {
            Self::NearestMipmapLinear | Self::LinearMipmapLinear => Self::Linear,
            _ => Self::Nearest,
        }
    }
}
impl TextureSampler {
    /// Preserve the two legacy keys; other combinations use an unambiguous key.
    pub fn key(self) -> String {
        if !self.min_filter.uses_mips()
            && self.wrap_s == TextureWrap::ClampToEdge
            && self.wrap_t == TextureWrap::ClampToEdge
            && self.min_filter == self.mag_filter
        {
            return match self.min_filter {
                TextureFilter::Nearest => "nearestClamp",
                TextureFilter::Linear => "linearClamp",
                _ => unreachable!("non-mip filter"),
            }
            .into();
        }
        let filter = |v| match v {
            TextureFilter::Nearest => "nearest",
            TextureFilter::Linear => "linear",
            TextureFilter::NearestMipmapNearest => "nearestMipmapNearest",
            TextureFilter::LinearMipmapNearest => "linearMipmapNearest",
            TextureFilter::NearestMipmapLinear => "nearestMipmapLinear",
            TextureFilter::LinearMipmapLinear => "linearMipmapLinear",
        };
        let wrap = |v| match v {
            TextureWrap::Repeat => "repeat",
            TextureWrap::ClampToEdge => "clampToEdge",
            TextureWrap::MirroredRepeat => "mirroredRepeat",
        };
        format!(
            "{}:{}:{}:{}",
            filter(self.min_filter),
            filter(self.mag_filter),
            wrap(self.wrap_s),
            wrap(self.wrap_t)
        )
    }

    pub fn parse(key: &str) -> Option<Self> {
        let legacy = match key {
            "nearestClamp" => Some(TextureFilter::Nearest),
            "linearClamp" => Some(TextureFilter::Linear),
            _ => None,
        };
        if let Some(filter) = legacy {
            return Some(Self {
                min_filter: filter,
                mag_filter: filter,
                wrap_s: TextureWrap::ClampToEdge,
                wrap_t: TextureWrap::ClampToEdge,
            });
        }
        let filter = |v| match v {
            "nearest" => Some(TextureFilter::Nearest),
            "linear" => Some(TextureFilter::Linear),
            "nearestMipmapNearest" => Some(TextureFilter::NearestMipmapNearest),
            "linearMipmapNearest" => Some(TextureFilter::LinearMipmapNearest),
            "nearestMipmapLinear" => Some(TextureFilter::NearestMipmapLinear),
            "linearMipmapLinear" => Some(TextureFilter::LinearMipmapLinear),
            _ => None,
        };
        let wrap = |v| match v {
            "repeat" => Some(TextureWrap::Repeat),
            "clampToEdge" => Some(TextureWrap::ClampToEdge),
            "mirroredRepeat" => Some(TextureWrap::MirroredRepeat),
            _ => None,
        };
        let mut parts = key.split(':');
        let sampler = Self {
            min_filter: filter(parts.next()?)?,
            mag_filter: filter(parts.next()?)?,
            wrap_s: wrap(parts.next()?)?,
            wrap_t: wrap(parts.next()?)?,
        };
        (parts.next().is_none() && !sampler.mag_filter.uses_mips() && sampler.key() == key)
            .then_some(sampler)
    }
}
