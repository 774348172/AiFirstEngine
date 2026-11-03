use super::*;
use base64::Engine;
use image::{ImageDecoder, ImageReader};
use std::io::Cursor;
use std::sync::Arc;

pub(super) fn container<'a>(bytes: &'a [u8], path: &str) -> Result<(&'a [u8], Option<&'a [u8]>)> {
    if !path.to_ascii_lowercase().ends_with(".glb") {
        if bytes.starts_with(b"glTF") {
            return Err(fail(
                path,
                "",
                "import_container_invalid",
                "GLB requires a .glb source.",
            ));
        }
        return Ok((bytes, None));
    }
    let invalid = || {
        fail(
            path,
            "",
            "import_container_invalid",
            "Expected a glTF 2 GLB with exact length and aligned JSON/BIN chunks.",
        )
    };
    if bytes.len() < 20
        || &bytes[..4] != b"glTF"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 2
        || u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize != bytes.len()
    {
        return Err(invalid());
    }
    // The library tolerates trailing bytes after BIN; the engine contract does not.
    let mut offset = 12usize;
    let mut chunks = Vec::new();
    while offset < bytes.len() {
        let header = bytes
            .get(offset..offset.checked_add(8).ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        let length = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(8)
            .and_then(|v| v.checked_add(length))
            .ok_or_else(invalid)?;
        if length % 4 != 0 || end > bytes.len() || chunks.len() >= 2 {
            return Err(invalid());
        }
        chunks.push((&header[4..], &bytes[offset + 8..end]));
        offset = end;
    }
    if chunks[0].0 != b"JSON" || (chunks.len() == 2 && chunks[1].0 != b"BIN\0") {
        return Err(invalid());
    }
    // Use the parser's container checks as well, retaining borrowed source slices.
    gltf::binary::Glb::from_slice(bytes)
        .map_err(|e| fail(path, "", "import_container_invalid", e.to_string()))?;
    Ok((chunks[0].1, chunks.get(1).map(|c| c.1)))
}

pub(super) fn version(json: &Value, source: &str) -> Result<()> {
    if json["asset"]["version"].as_str() != Some("2.0")
        || json["asset"]
            .get("minVersion")
            .is_some_and(|v| v.as_str() != Some("2.0"))
    {
        return Err(fail(
            source,
            "/asset",
            "import_version_unsupported",
            "Only glTF 2.0 with absent or 2.0 minVersion is supported.",
        ));
    }
    if let Some(required) = json.get("extensionsRequired") {
        let required = required.as_array().ok_or_else(|| {
            fail(
                source,
                "/extensionsRequired",
                "import_schema_invalid",
                "extensionsRequired must be an array.",
            )
        })?;
        for (index, extension) in required.iter().enumerate() {
            if extension.as_str() != Some("KHR_materials_unlit") {
                return Err(fail(
                    source,
                    &format!("/extensionsRequired/{index}"),
                    "import_required_extension_unsupported",
                    format!("Unsupported required extension: {extension}"),
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn extensions(
    value: &Value,
    path: &str,
    consumed: &BTreeSet<String>,
    source: &str,
    notices: &mut Vec<ImportDecodeNotice>,
) -> Result<()> {
    match value {
        Value::Object(fields) => {
            if let Some(extensions) = fields.get("extensions").and_then(Value::as_object) {
                for name in extensions.keys() {
                    let mut ancestor = path;
                    let mut selected = false;
                    while !ancestor.is_empty() {
                        if consumed.contains(ancestor) {
                            selected = true;
                            break;
                        }
                        ancestor = ancestor.rsplit_once('/').map_or("", |(parent, _)| parent);
                    }
                    // A root light definition alone is not consumed; the selected
                    // node's extension establishes whether the light is in scope.
                    selected |= path.is_empty() && name != "KHR_lights_punctual";
                    let at = format!("{path}/extensions/{name}");
                    if selected
                        && matches!(
                            name.as_str(),
                            "KHR_draco_mesh_compression"
                                | "EXT_meshopt_compression"
                                | "KHR_lights_punctual"
                                | "KHR_texture_transform"
                        )
                    {
                        return Err(fail(
                            source,
                            &at,
                            "import_extension_unsupported",
                            format!("Selected content uses unsupported {name}."),
                        ));
                    }
                    notices.push(ImportDecodeNotice {
                        source_path: source.into(),
                        field_path: at,
                        outcome: if !selected {
                            "unconsumed"
                        } else if name == "KHR_materials_unlit" {
                            "recognized"
                        } else {
                            "ignored"
                        }
                        .into(),
                        message: format!(
                            "Extension {name}; only recognized supported semantics are applied by static import."
                        ),
                    });
                }
            }
            for (key, child) in fields
                .iter()
                .filter(|(k, _)| k.as_str() != "extensions" && k.as_str() != "extras")
            {
                extensions(child, &format!("{path}/{key}"), consumed, source, notices)?;
            }
            if path.is_empty() {
                for (index, name) in fields
                    .get("extensionsUsed")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .enumerate()
                {
                    if !notices
                        .iter()
                        .any(|n| n.field_path.ends_with(&format!("/extensions/{name}")))
                    {
                        notices.push(ImportDecodeNotice {
                            source_path: source.into(),
                            field_path: format!("/extensionsUsed/{index}"),
                            outcome: "unconsumed".into(),
                            message: format!(
                                "Extension {name} is declared without a consumed payload."
                            ),
                        });
                    }
                }
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                extensions(child, &format!("{path}/{index}"), consumed, source, notices)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) struct SourceBytes<'a, 'd> {
    source: &'a CompilerSourceView,
    model_path: &'a str,
    bin: Option<&'a [u8]>,
    cache: BTreeMap<String, Arc<[u8]>>,
    dependencies: &'d mut Vec<ImportSourceDependency>,
}
impl<'a, 'd> SourceBytes<'a, 'd> {
    pub(super) fn new(
        source: &'a CompilerSourceView,
        model_path: &'a str,
        bin: Option<&'a [u8]>,
        dependencies: &'d mut Vec<ImportSourceDependency>,
    ) -> Self {
        Self {
            source,
            model_path,
            bin,
            cache: BTreeMap::new(),
            dependencies,
        }
    }

    fn uri(
        &mut self,
        uri: &str,
        locator: &str,
        image: bool,
        budget: &mut Budget<'_>,
    ) -> Result<Arc<[u8]>> {
        if let Some(data) = uri.strip_prefix("data:") {
            let (header, encoded) = data.split_once(',').ok_or_else(|| {
                fail(
                    self.model_path,
                    locator,
                    "import_uri_invalid",
                    "Data URI has no separator.",
                )
            })?;
            let allowed = if image {
                ["image/png;base64", "image/jpeg;base64"]
            } else {
                [
                    "application/octet-stream;base64",
                    "application/gltf-buffer;base64",
                ]
            };
            if !allowed.contains(&header) {
                return Err(fail(self.model_path, locator, "import_uri_unsupported", "Only base64 PNG/JPEG image or octet-stream/gltf-buffer data URIs are supported."));
            }
            let maximum = (encoded.len() as u64)
                .checked_add(3)
                .and_then(|n| n.checked_div(4))
                .and_then(|n| n.checked_mul(3))
                .ok_or_else(|| {
                    fail(
                        self.model_path,
                        locator,
                        "source_budget_exceeded",
                        "Data URI size overflows.",
                    )
                })?;
            if maximum > budget.limits.source_bytes {
                return Err(fail(
                    self.model_path,
                    locator,
                    "source_budget_exceeded",
                    "Data URI decoded length exceeds source budget before allocation.",
                ));
            }
            let key = format!("data:{}", sha256_prefixed(uri.as_bytes()));
            let bytes = if let Some(bytes) = self.cache.get(&key) {
                Arc::clone(bytes)
            } else {
                let bytes: Arc<[u8]> = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|e| {
                        fail(
                            self.model_path,
                            locator,
                            "import_uri_invalid",
                            e.to_string(),
                        )
                    })?
                    .into();
                self.cache.insert(key, Arc::clone(&bytes));
                bytes
            };
            self.dependencies.push(ImportSourceDependency {
                locator: locator.into(),
                source_path: self.model_path.into(),
                byte_length: bytes.len() as u64,
                digest: sha256_prefixed(&bytes),
            });
            return Ok(bytes);
        }
        // Decode once and then apply the same canonical path restrictions as project writes.
        let raw = uri.as_bytes();
        for (i, &c) in raw.iter().enumerate() {
            if c == b'%'
                && (i + 2 >= raw.len()
                    || !raw[i + 1].is_ascii_hexdigit()
                    || !raw[i + 2].is_ascii_hexdigit())
            {
                return Err(fail(
                    self.model_path,
                    locator,
                    "import_uri_invalid",
                    format!("Invalid percent encoding in URI {uri}."),
                ));
            }
        }
        let decoded = percent_encoding::percent_decode_str(uri)
            .decode_utf8()
            .map_err(|_| {
                fail(
                    self.model_path,
                    locator,
                    "import_uri_invalid",
                    "URI is not UTF-8.",
                )
            })?;
        if decoded.contains([':', '\\', '?', '#', '\0'])
            || decoded.chars().any(char::is_control)
            || decoded.split('/').any(|p| p == "." || p == "..")
        {
            return Err(fail(
                self.model_path,
                locator,
                "import_uri_unsafe",
                format!("Only canonical project-relative URI paths are allowed: {uri}"),
            ));
        }
        crate::ProjectRelativePath::parse(decoded.as_ref()).map_err(|_| {
            fail(
                self.model_path,
                locator,
                "import_uri_unsafe",
                format!("URI is not a safe relative path: {uri}"),
            )
        })?;
        let parent = self.model_path.rsplit_once('/').map_or("", |(p, _)| p);
        let path = if parent.is_empty() {
            decoded.into_owned()
        } else {
            format!("{parent}/{decoded}")
        };
        let safe = crate::ProjectRelativePath::parse(&path).map_err(|_| {
            fail(
                self.model_path,
                locator,
                "import_uri_unsafe",
                format!("Unsafe resolved dependency {path}."),
            )
        })?;
        let path = safe.as_str();
        let key = format!("file:{path}");
        let bytes = if let Some(bytes) = self.cache.get(&key) {
            Arc::clone(bytes)
        } else {
            let bytes = self.source.bytes(path).ok_or_else(|| {
                fail(
                    self.model_path,
                    locator,
                    "import_dependency_missing",
                    format!(
                        "Dependency {path} from URI {uri} is absent from the operation snapshot."
                    ),
                )
            })?;
            // The primary source is already charged, even if a URI refers back to it.
            if path != self.model_path {
                charge(
                    &mut budget.source_bytes,
                    bytes.len() as u64,
                    budget.limits.source_bytes,
                    self.model_path,
                    locator,
                    "source_budget_exceeded",
                )?;
            }
            let bytes: Arc<[u8]> = bytes.into();
            self.cache.insert(key, Arc::clone(&bytes));
            bytes
        };
        self.dependencies.push(ImportSourceDependency {
            locator: locator.into(),
            source_path: path.into(),
            byte_length: bytes.len() as u64,
            digest: sha256_prefixed(&bytes),
        });
        Ok(bytes)
    }

    pub(super) fn buffer(
        &mut self,
        buffer: &gltf::Buffer<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<Arc<[u8]>> {
        let path = format!("/buffers/{}", buffer.index());
        if buffer.length() as u64 > budget.limits.source_bytes {
            return Err(fail(
                self.model_path,
                &path,
                "source_budget_exceeded",
                "Declared buffer exceeds source budget.",
            ));
        }
        let bytes = match buffer.source() {
            gltf::buffer::Source::Uri(uri) => {
                self.uri(uri, &format!("{path}/uri"), false, budget)?
            }
            gltf::buffer::Source::Bin => {
                if buffer.index() != 0 {
                    return Err(fail(
                        self.model_path,
                        &path,
                        "import_buffer_invalid",
                        "GLB BIN must be buffer 0.",
                    ));
                }
                let bytes = self.bin.ok_or_else(|| {
                    fail(
                        self.model_path,
                        &path,
                        "import_buffer_missing",
                        "Missing GLB BIN chunk.",
                    )
                })?;
                if bytes.len() < buffer.length() || bytes.len() - buffer.length() > 3 {
                    return Err(fail(
                        self.model_path,
                        &path,
                        "import_buffer_invalid",
                        "GLB BIN length differs from declared length beyond allowed padding.",
                    ));
                }
                self.dependencies.push(ImportSourceDependency {
                    locator: path.clone(),
                    source_path: self.model_path.into(),
                    byte_length: buffer.length() as u64,
                    digest: sha256_prefixed(&bytes[..buffer.length()]),
                });
                Arc::from(&bytes[..buffer.length()])
            }
        };
        if bytes.len() < buffer.length() {
            return Err(fail(
                self.model_path,
                &path,
                "import_buffer_truncated",
                "Buffer is shorter than byteLength.",
            ));
        }
        Ok(bytes)
    }

    pub(super) fn image(
        &mut self,
        image: &gltf::Image<'_>,
        buffers: &BTreeMap<usize, Arc<[u8]>>,
        budget: &mut Budget<'_>,
    ) -> Result<Arc<[u8]>> {
        let path = format!("/images/{}", image.index());
        match image.source() {
            gltf::image::Source::Uri { uri, .. } => {
                self.uri(uri, &format!("{path}/uri"), true, budget)
            }
            gltf::image::Source::View { view, .. } => {
                let range = checked_view(&view, buffers, self.model_path)?;
                let bytes = &buffers[&view.buffer().index()][range];
                self.dependencies.push(ImportSourceDependency {
                    locator: path,
                    source_path: self.model_path.into(),
                    byte_length: bytes.len() as u64,
                    digest: sha256_prefixed(bytes),
                });
                Ok(Arc::from(bytes))
            }
        }
    }
}

pub(super) fn checked_view(
    view: &gltf::buffer::View<'_>,
    buffers: &BTreeMap<usize, Arc<[u8]>>,
    source: &str,
) -> Result<std::ops::Range<usize>> {
    let path = format!("/bufferViews/{}", view.index());
    let bytes = buffers.get(&view.buffer().index()).ok_or_else(|| {
        fail(
            source,
            &path,
            "import_buffer_missing",
            "View buffer has not been captured.",
        )
    })?;
    let end = view
        .offset()
        .checked_add(view.length())
        .filter(|end| *end <= view.buffer().length() && *end <= bytes.len())
        .ok_or_else(|| {
            fail(
                source,
                &path,
                "import_buffer_range_invalid",
                "bufferView exceeds declared or captured buffer length.",
            )
        })?;
    Ok(view.offset()..end)
}

pub(super) fn decode_image(
    bytes: &[u8],
    input: gltf::image::Source<'_>,
    path: &str,
    budget: &mut Budget<'_>,
) -> Result<DecodedImage> {
    let format = image::guess_format(bytes)
        .map_err(|e| fail(budget.source, path, "import_image_invalid", e.to_string()))?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err(fail(
            budget.source,
            path,
            "import_image_unsupported",
            "Only PNG and JPEG images are supported.",
        ));
    }
    let mimes = match input {
        gltf::image::Source::View { mime_type, .. } => [Some(mime_type), None],
        gltf::image::Source::Uri { mime_type, uri } => [
            mime_type,
            uri.strip_prefix("data:")
                .and_then(|data| data.split_once(';').map(|(mime, _)| mime)),
        ],
    };
    if mimes.into_iter().flatten().any(|m| {
        m != if format == image::ImageFormat::Png {
            "image/png"
        } else {
            "image/jpeg"
        }
    }) {
        return Err(fail(
            budget.source,
            path,
            "import_image_invalid",
            "MIME type does not match image bytes.",
        ));
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let source = budget.source;
    let image_error = |error: image::ImageError| {
        let code = if matches!(error, image::ImageError::Limits(_)) {
            "import_image_budget_exceeded"
        } else {
            "import_image_invalid"
        };
        fail(source, path, code, error.to_string())
    };
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(budget.limits.decoded_image_bytes);
    limits.max_image_width = Some(budget.limits.image_pixels.min(u32::MAX as u64) as u32);
    limits.max_image_height = limits.max_image_width;
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(image_error)?;
    let (width, height) = decoder.dimensions();
    let pixels = u64::from(width) * u64::from(height);
    if pixels == 0 || pixels > budget.limits.image_pixels {
        return Err(fail(
            budget.source,
            path,
            "import_image_budget_exceeded",
            "Decoded image pixel budget exceeded before pixel allocation.",
        ));
    }
    // Bound decoder output as well as canonical RGBA8 (e.g. 16-bit PNG).
    let decoded_size = decoder.total_bytes().max(pixels * 4);
    charge(
        &mut budget.image_bytes,
        decoded_size,
        budget.limits.decoded_image_bytes,
        budget.source,
        path,
        "import_image_budget_exceeded",
    )?;
    charge(
        &mut budget.cooked_bytes,
        pixels * 4,
        budget.limits.cooked_bytes,
        budget.source,
        path,
        "import_cooked_budget_exceeded",
    )?;
    let image = image::DynamicImage::from_decoder(decoder)
        .map_err(image_error)?
        .into_rgba8();
    Ok(DecodedImage {
        width,
        height,
        rgba8: image.into_raw(),
    })
}
