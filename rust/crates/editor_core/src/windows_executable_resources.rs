use crate::{BuildProfileApplication, BuildProfileIconRef};
use editpe::constants::{LANGUAGE_ID_EN_US, RT_GROUP_ICON};
use editpe::types::{VersionU16, VersionU32};
use editpe::{
    Image, ResourceDirectory, ResourceEntry, ResourceEntryName, VersionInfo, VersionStringTable,
};
use engine_runtime::release_package_manifest::ReleasePackageApplication;
use engine_runtime::runtime_package_path::{
    safe_join_runtime_package, validate_package_path_segment,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};

pub const WINDOWS_APPLICATION_ICON_SIZES: [u32; 6] = [16, 32, 48, 64, 128, 256];
const DEFAULT_WINDOWS_APPLICATION_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedReleaseIcon {
    pub asset_id: String,
    pub descriptor_path: String,
    pub source_path: String,
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsExecutableResourceReadback {
    pub product_name: String,
    pub company_name: String,
    pub file_description: String,
    pub product_version: String,
    pub file_version: String,
    pub copyright: String,
    pub original_filename: String,
    pub fixed_file_version: [u16; 4],
    pub fixed_product_version: [u16; 4],
    pub icon_sizes: Vec<u32>,
    pub manifest_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsExecutableResourceExpectation {
    pub product_name: String,
    pub company_name: String,
    pub file_description: String,
    pub product_version: String,
    pub file_version: String,
    pub copyright: String,
    pub original_filename: String,
    pub fixed_file_version: [u16; 4],
    pub fixed_product_version: [u16; 4],
    pub icon_sizes: Vec<u32>,
    pub manifest_present: bool,
}

impl WindowsExecutableResourceExpectation {
    pub fn from_build_profile(application: &BuildProfileApplication) -> Self {
        Self::new(
            &application.display_name,
            &application.executable_name,
            &application.company_name,
            &application.file_description,
            &application.display_version,
            application.windows_file_version,
            application.windows_product_version,
            &application.copyright,
        )
    }

    pub fn from_release_manifest(application: &ReleasePackageApplication) -> Self {
        Self::new(
            &application.display_name,
            &application.executable_name,
            &application.company_name,
            &application.file_description,
            &application.display_version,
            application.windows_file_version,
            application.windows_product_version,
            &application.copyright,
        )
    }

    fn new(
        display_name: &str,
        executable_name: &str,
        company_name: &str,
        file_description: &str,
        display_version: &str,
        windows_file_version: [u16; 4],
        windows_product_version: [u16; 4],
        copyright: &str,
    ) -> Self {
        Self {
            product_name: display_name.to_string(),
            company_name: company_name.to_string(),
            file_description: file_description.to_string(),
            product_version: display_version.to_string(),
            file_version: version_text(windows_file_version),
            copyright: copyright.to_string(),
            original_filename: format!("{executable_name}.exe"),
            fixed_file_version: windows_file_version,
            fixed_product_version: windows_product_version,
            icon_sizes: WINDOWS_APPLICATION_ICON_SIZES.to_vec(),
            manifest_present: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsExecutableResourceError {
    pub code: &'static str,
    pub stage: &'static str,
    pub path: PathBuf,
    pub message: String,
    pub next_action: &'static str,
}

impl std::fmt::Display for WindowsExecutableResourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} at {} for {}: {}; next action: {}",
            self.code,
            self.stage,
            self.path.display(),
            self.message,
            self.next_action
        )
    }
}

impl std::error::Error for WindowsExecutableResourceError {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReleaseIconTextureDescriptor {
    #[serde(rename = "schemaVersion")]
    schema_version: String,
    asset_id: String,
    #[serde(rename = "assetGuid", default)]
    _asset_guid: Option<String>,
    #[serde(rename = "displayName", default)]
    _display_name: Option<String>,
    source_image: String,
    #[serde(default)]
    importer: Option<ReleaseIconImporter>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReleaseIconImporter {
    #[serde(default)]
    format: Option<String>,
    #[serde(rename = "colorSpace", default)]
    _color_space: Option<String>,
    #[serde(rename = "sampler", default)]
    _sampler: Option<String>,
}

pub fn resolve_release_icon_asset(
    project_root: &Path,
    icon_ref: &BuildProfileIconRef,
) -> Result<ResolvedReleaseIcon, WindowsExecutableResourceError> {
    validate_package_path_segment(&icon_ref.asset_id).map_err(|error| {
        resource_error(
            "release_icon_asset_missing",
            "resolve_icon_asset_ref",
            project_root,
            format!("invalid icon AssetRef assetId: {error}"),
            "Select a project Texture/Sprite asset through the Asset Picker.",
        )
    })?;
    let descriptor_relative_path = resolve_icon_descriptor_relative_path(project_root, icon_ref)?;
    let descriptor_path = safe_join_runtime_package(project_root, &descriptor_relative_path)
        .map_err(|error| {
            resource_error(
                "release_icon_asset_missing",
                "resolve_icon_descriptor",
                project_root,
                error.to_string(),
                "Repair the icon AssetRef or its project asset descriptor.",
            )
        })?;
    let text = fs::read_to_string(&descriptor_path).map_err(|error| {
        resource_error(
            "release_icon_asset_missing",
            "read_icon_descriptor",
            &descriptor_path,
            error.to_string(),
            "Create the referenced texture asset descriptor or choose another icon asset.",
        )
    })?;
    let descriptor: ReleaseIconTextureDescriptor =
        serde_json::from_str(&text).map_err(|error| {
            resource_error(
                "release_icon_decode_failed",
                "parse_icon_descriptor",
                &descriptor_path,
                error.to_string(),
                "Fix the texture-asset.v1 descriptor before building a release package.",
            )
        })?;
    if descriptor.schema_version != "texture-asset.v1" || descriptor.asset_id != icon_ref.asset_id {
        return Err(resource_error(
            "release_icon_asset_missing",
            "validate_icon_descriptor",
            &descriptor_path,
            format!(
                "expected texture-asset.v1 assetId {}, got {} assetId {}",
                icon_ref.asset_id, descriptor.schema_version, descriptor.asset_id
            ),
            "Repair the descriptor schema/assetId or choose another icon asset.",
        ));
    }
    if descriptor
        .importer
        .as_ref()
        .and_then(|importer| importer.format.as_deref())
        .is_some_and(|format| !format.eq_ignore_ascii_case("png"))
        || !descriptor
            .source_image
            .to_ascii_lowercase()
            .ends_with(".png")
    {
        return Err(resource_error(
            "release_icon_decode_failed",
            "validate_icon_format",
            &descriptor_path,
            "portable-directory-v1 currently accepts PNG icon source assets only",
            "Use a square PNG texture asset for the application icon.",
        ));
    }
    let source_path =
        safe_join_runtime_package(project_root, &descriptor.source_image).map_err(|error| {
            resource_error(
                "release_icon_asset_missing",
                "resolve_icon_source",
                project_root,
                error.to_string(),
                "Repair the texture descriptor sourceImage path.",
            )
        })?;
    let decoded = decode_png_rgba8(&source_path).map_err(|message| {
        resource_error(
            "release_icon_decode_failed",
            "decode_icon_source",
            &source_path,
            message,
            "Use a valid non-empty square RGBA/RGB PNG.",
        )
    })?;
    if decoded.width == 0 || decoded.width != decoded.height {
        return Err(resource_error(
            "release_icon_decode_failed",
            "validate_icon_dimensions",
            &source_path,
            format!(
                "icon source must be non-empty and square, got {}x{}",
                decoded.width, decoded.height
            ),
            "Replace the icon source with a non-empty square PNG.",
        ));
    }
    Ok(ResolvedReleaseIcon {
        asset_id: icon_ref.asset_id.clone(),
        descriptor_path: descriptor_relative_path,
        source_path: descriptor.source_image,
        width: decoded.width,
        height: decoded.height,
        rgba8: decoded.bytes,
    })
}

fn resolve_icon_descriptor_relative_path(
    project_root: &Path,
    icon_ref: &BuildProfileIconRef,
) -> Result<String, WindowsExecutableResourceError> {
    const ASSET_DATABASE_RELATIVE_PATH: &str = "Library/AssetPipeline/asset-database.json";
    let database_path = safe_join_runtime_package(project_root, ASSET_DATABASE_RELATIVE_PATH)
        .map_err(|error| {
            resource_error(
                "release_icon_asset_missing",
                "resolve_icon_asset_database",
                project_root,
                error.to_string(),
                "Repair the project Asset DB path before building a release package.",
            )
        })?;
    if !database_path.is_file() {
        return Ok(format!("Assets/{}.asset", icon_ref.asset_id));
    }
    let text = fs::read_to_string(&database_path).map_err(|error| {
        resource_error(
            "release_icon_asset_missing",
            "read_icon_asset_database",
            &database_path,
            error.to_string(),
            "Repair or rebuild the project Asset DB before building a release package.",
        )
    })?;
    let database: crate::AssetDatabaseDocument = serde_json::from_str(&text).map_err(|error| {
        resource_error(
            "release_icon_asset_missing",
            "parse_icon_asset_database",
            &database_path,
            error.to_string(),
            "Repair or rebuild the project Asset DB before building a release package.",
        )
    })?;
    let Some(record) = database.asset_by_id(&icon_ref.asset_id) else {
        return Err(resource_error(
            "release_icon_asset_missing",
            "resolve_icon_asset_ref",
            &database_path,
            format!("Asset DB does not contain assetId {}", icon_ref.asset_id),
            "Import the icon asset or choose another Texture asset through the Asset Picker.",
        ));
    };
    if record.asset_type != "texture" {
        return Err(resource_error(
            "release_icon_asset_missing",
            "validate_icon_asset_type",
            &database_path,
            format!(
                "release icon asset {} has type {}, expected texture",
                icon_ref.asset_id, record.asset_type
            ),
            "Choose a Texture asset through the Asset Picker.",
        ));
    }
    Ok(record.descriptor_path.clone())
}

pub fn stamp_windows_executable_resources(
    executable_path: &Path,
    application: &BuildProfileApplication,
    icon: &ResolvedReleaseIcon,
) -> Result<WindowsExecutableResourceReadback, WindowsExecutableResourceError> {
    let mut image = Image::parse_file(executable_path).map_err(|error| {
        resource_error(
            "release_resource_stamp_failed",
            "parse_staging_executable",
            executable_path,
            error.to_string(),
            "Use an unmodified Windows PE runtime template and retry the release build.",
        )
    })?;
    let mut resources = image.resource_directory().cloned().unwrap_or_default();
    resources.remove_main_icon().map_err(|error| {
        resource_error(
            "release_resource_stamp_failed",
            "remove_existing_main_icon",
            executable_path,
            error.to_string(),
            "Inspect the staging PE resource directory before retrying.",
        )
    })?;
    let icon_bytes = build_windows_icon(icon).map_err(|message| {
        resource_error(
            "release_icon_decode_failed",
            "build_multi_size_icon",
            Path::new(&icon.source_path),
            message,
            "Use a valid square PNG source and retry.",
        )
    })?;
    resources.set_main_icon(icon_bytes).map_err(|error| {
        resource_error(
            "release_resource_stamp_failed",
            "write_group_icon",
            executable_path,
            error.to_string(),
            "Inspect the staging PE icon resource and retry.",
        )
    })?;
    resources
        .set_version_info(&build_version_info(application))
        .map_err(|error| {
            resource_error(
                "release_resource_stamp_failed",
                "write_version_info",
                executable_path,
                error.to_string(),
                "Inspect the staging PE version resource and retry.",
            )
        })?;
    if resources
        .get_manifest()
        .map_err(|error| {
            resource_error(
                "release_resource_stamp_failed",
                "read_application_manifest",
                executable_path,
                error.to_string(),
                "Inspect the staging PE manifest resource and retry.",
            )
        })?
        .is_none()
    {
        resources
            .set_manifest(DEFAULT_WINDOWS_APPLICATION_MANIFEST)
            .map_err(|error| {
                resource_error(
                    "release_resource_stamp_failed",
                    "write_application_manifest",
                    executable_path,
                    error.to_string(),
                    "Use a supported PE template and retry resource stamping.",
                )
            })?;
    }
    image.set_resource_directory(resources).map_err(|error| {
        resource_error(
            "release_resource_stamp_failed",
            "rebuild_resource_directory",
            executable_path,
            error.to_string(),
            "Use a supported unpacked PE runtime template with sufficient header space.",
        )
    })?;
    image.write_file(executable_path).map_err(|error| {
        resource_error(
            "release_resource_stamp_failed",
            "write_staging_executable",
            executable_path,
            error.to_string(),
            "Check the owned staging path and retry without modifying the source template.",
        )
    })?;

    verify_windows_executable_resource_contract(
        executable_path,
        &WindowsExecutableResourceExpectation::from_build_profile(application),
    )
}

pub fn verify_windows_executable_resource_contract(
    executable_path: &Path,
    expectation: &WindowsExecutableResourceExpectation,
) -> Result<WindowsExecutableResourceReadback, WindowsExecutableResourceError> {
    let readback = read_windows_executable_resources(executable_path)?;
    let string_fields = [
        (
            "ProductName",
            &expectation.product_name,
            &readback.product_name,
        ),
        (
            "CompanyName",
            &expectation.company_name,
            &readback.company_name,
        ),
        (
            "FileDescription",
            &expectation.file_description,
            &readback.file_description,
        ),
        (
            "ProductVersion",
            &expectation.product_version,
            &readback.product_version,
        ),
        (
            "FileVersion",
            &expectation.file_version,
            &readback.file_version,
        ),
        (
            "LegalCopyright",
            &expectation.copyright,
            &readback.copyright,
        ),
        (
            "OriginalFilename",
            &expectation.original_filename,
            &readback.original_filename,
        ),
    ];
    if let Some((field, expected, actual)) = string_fields
        .into_iter()
        .find(|(_, expected, actual)| expected != actual)
    {
        return Err(resource_error(
            "release_resource_readback_mismatch",
            "verify_version_string_contract",
            executable_path,
            format!("{field} expected {expected:?}, got {actual:?}"),
            "Repeat resource stamping on a fresh staging copy.",
        ));
    }
    for (field, expected, actual) in [
        (
            "fixedFileVersion",
            expectation.fixed_file_version,
            readback.fixed_file_version,
        ),
        (
            "fixedProductVersion",
            expectation.fixed_product_version,
            readback.fixed_product_version,
        ),
    ] {
        if expected != actual {
            return Err(resource_error(
                "release_resource_readback_mismatch",
                "verify_fixed_version_contract",
                executable_path,
                format!("{field} expected {expected:?}, got {actual:?}"),
                "Repeat resource stamping on a fresh staging copy.",
            ));
        }
    }
    if readback.icon_sizes != expectation.icon_sizes {
        return Err(resource_error(
            "release_resource_readback_mismatch",
            "verify_group_icon_contract",
            executable_path,
            format!(
                "icon sizes expected {:?}, got {:?}",
                expectation.icon_sizes, readback.icon_sizes
            ),
            "Repeat icon stamping on a fresh staging copy.",
        ));
    }
    if readback.manifest_present != expectation.manifest_present {
        return Err(resource_error(
            "release_resource_readback_mismatch",
            "verify_application_manifest_contract",
            executable_path,
            format!(
                "application manifest presence expected {}, got {}",
                expectation.manifest_present, readback.manifest_present
            ),
            "Restore the application manifest in the source Runtime executable template.",
        ));
    }
    Ok(readback)
}

pub fn read_windows_executable_resources(
    executable_path: &Path,
) -> Result<WindowsExecutableResourceReadback, WindowsExecutableResourceError> {
    let image = Image::parse_file(executable_path).map_err(|error| {
        resource_error(
            "release_resource_readback_mismatch",
            "readback_parse_executable",
            executable_path,
            error.to_string(),
            "Rebuild the staging executable from the source runtime template.",
        )
    })?;
    let resources = image.resource_directory().ok_or_else(|| {
        resource_error(
            "release_resource_readback_mismatch",
            "readback_resource_directory",
            executable_path,
            "stamped executable has no PE resource directory",
            "Repeat resource stamping on a fresh staging copy.",
        )
    })?;
    let version = resources
        .get_version_info()
        .map_err(|error| {
            resource_error(
                "release_resource_readback_mismatch",
                "readback_version_info",
                executable_path,
                error.to_string(),
                "Repeat resource stamping on a fresh staging copy.",
            )
        })?
        .ok_or_else(|| {
            resource_error(
                "release_resource_readback_mismatch",
                "readback_version_info",
                executable_path,
                "stamped executable has no VERSIONINFO resource",
                "Repeat resource stamping on a fresh staging copy.",
            )
        })?;
    let fixed = version.info;
    let file_version = fixed.file_version;
    let product_version = fixed.product_version;
    let manifest_present = resources
        .get_manifest()
        .map_err(|error| {
            resource_error(
                "release_resource_readback_mismatch",
                "readback_manifest",
                executable_path,
                error.to_string(),
                "Inspect the source template manifest resource.",
            )
        })?
        .is_some();
    Ok(WindowsExecutableResourceReadback {
        product_name: version_string(&version, "ProductName"),
        company_name: version_string(&version, "CompanyName"),
        file_description: version_string(&version, "FileDescription"),
        product_version: version_string(&version, "ProductVersion"),
        file_version: version_string(&version, "FileVersion"),
        copyright: version_string(&version, "LegalCopyright"),
        original_filename: version_string(&version, "OriginalFilename"),
        fixed_file_version: unpack_version(file_version),
        fixed_product_version: unpack_version(product_version),
        icon_sizes: read_group_icon_sizes(resources).map_err(|message| {
            resource_error(
                "release_resource_readback_mismatch",
                "readback_group_icon",
                executable_path,
                message,
                "Repeat icon stamping on a fresh staging copy.",
            )
        })?,
        manifest_present,
    })
}

fn build_version_info(application: &BuildProfileApplication) -> VersionInfo {
    let mut version = VersionInfo::default();
    version.info.file_version = pack_version(application.windows_file_version);
    version.info.product_version = pack_version(application.windows_product_version);
    let mut strings = VersionStringTable {
        key: "040904B0".to_string(),
        ..VersionStringTable::default()
    };
    let original_filename = format!("{}.exe", application.executable_name);
    for (key, value) in [
        ("CompanyName", application.company_name.clone()),
        ("FileDescription", application.file_description.clone()),
        (
            "FileVersion",
            version_text(application.windows_file_version),
        ),
        ("InternalName", application.executable_name.clone()),
        ("LegalCopyright", application.copyright.clone()),
        ("OriginalFilename", original_filename),
        ("ProductName", application.display_name.clone()),
        ("ProductVersion", application.display_version.clone()),
    ] {
        strings.strings.insert(key.to_string(), value);
    }
    version.strings.push(strings);
    version.vars.push(VersionU16 {
        major: LANGUAGE_ID_EN_US,
        minor: 1200,
    });
    version
}

fn build_windows_icon(icon: &ResolvedReleaseIcon) -> Result<Vec<u8>, String> {
    let mut images = Vec::with_capacity(WINDOWS_APPLICATION_ICON_SIZES.len());
    for size in WINDOWS_APPLICATION_ICON_SIZES {
        let resized = resize_rgba8_bilinear(&icon.rgba8, icon.width, icon.height, size, size)?;
        images.push((size, encode_png_rgba8(size, size, &resized)?));
    }
    let directory_size = 6 + images.len() * 16;
    let mut offset = directory_size as u32;
    let mut bytes = Vec::with_capacity(
        directory_size + images.iter().map(|(_, image)| image.len()).sum::<usize>(),
    );
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&(images.len() as u16).to_le_bytes());
    for (size, image) in &images {
        bytes.push(if *size == 256 { 0 } else { *size as u8 });
        bytes.push(if *size == 256 { 0 } else { *size as u8 });
        bytes.push(0);
        bytes.push(0);
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(&(image.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&offset.to_le_bytes());
        offset += image.len() as u32;
    }
    for (_, image) in images {
        bytes.extend_from_slice(&image);
    }
    Ok(bytes)
}

fn read_group_icon_sizes(resources: &ResourceDirectory) -> Result<Vec<u32>, String> {
    let group_table = resources
        .root()
        .get(ResourceEntryName::ID(RT_GROUP_ICON as u32))
        .and_then(ResourceEntry::as_table)
        .ok_or_else(|| "GROUP_ICON table is missing".to_string())?;
    let main_name = ResourceEntryName::from_string("MAINICON");
    let group_entry = group_table.get(&main_name).or_else(|| {
        group_table
            .entries()
            .first()
            .and_then(|name| group_table.get(*name))
    });
    let language_table = group_entry
        .and_then(ResourceEntry::as_table)
        .ok_or_else(|| "MAINICON group is missing".to_string())?;
    let data = language_table
        .entries()
        .first()
        .and_then(|name| language_table.get(*name))
        .and_then(ResourceEntry::as_data)
        .ok_or_else(|| "MAINICON group data is missing".to_string())?
        .data();
    if data.len() < 6 {
        return Err("MAINICON group data is truncated".to_string());
    }
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;
    if data.len() < 6 + count * 14 {
        return Err("MAINICON entries are truncated".to_string());
    }
    let mut sizes = Vec::with_capacity(count);
    for index in 0..count {
        let offset = 6 + index * 14;
        let width = if data[offset] == 0 {
            256
        } else {
            u32::from(data[offset])
        };
        let height = if data[offset + 1] == 0 {
            256
        } else {
            u32::from(data[offset + 1])
        };
        if width != height {
            return Err(format!("GROUP_ICON entry is not square: {width}x{height}"));
        }
        sizes.push(width);
    }
    sizes.sort_unstable();
    sizes.dedup();
    Ok(sizes)
}

fn version_string(version: &VersionInfo, key: &str) -> String {
    version
        .strings
        .iter()
        .find_map(|table| table.strings.get(key))
        .cloned()
        .unwrap_or_default()
}

fn pack_version(version: [u16; 4]) -> VersionU32 {
    VersionU32 {
        major: (u32::from(version[0]) << 16) | u32::from(version[1]),
        minor: (u32::from(version[2]) << 16) | u32::from(version[3]),
    }
}

fn unpack_version(version: VersionU32) -> [u16; 4] {
    [
        (version.major >> 16) as u16,
        version.major as u16,
        (version.minor >> 16) as u16,
        version.minor as u16,
    ]
}

fn version_text(version: [u16; 4]) -> String {
    format!(
        "{}.{}.{}.{}",
        version[0], version[1], version[2], version[3]
    )
}

struct DecodedRgbaImage {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

fn decode_png_rgba8(path: &Path) -> Result<DecodedRgbaImage, String> {
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    let bytes = &buffer[..info.buffer_size()];
    let rgba8 = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => bytes
            .chunks_exact(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
            .collect(),
        png::ColorType::Grayscale => bytes.iter().flat_map(|v| [*v, *v, *v, 255]).collect(),
        png::ColorType::GrayscaleAlpha => bytes
            .chunks_exact(2)
            .flat_map(|value| [value[0], value[0], value[0], value[1]])
            .collect(),
        png::ColorType::Indexed => return Err("indexed PNG was not expanded".to_string()),
    };
    Ok(DecodedRgbaImage {
        width: info.width,
        height: info.height,
        bytes: rgba8,
    })
}

fn resize_rgba8_bilinear(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> Result<Vec<u8>, String> {
    if source.len() != source_width as usize * source_height as usize * 4 {
        return Err("decoded RGBA byte length does not match icon dimensions".to_string());
    }
    let mut target = vec![0; target_width as usize * target_height as usize * 4];
    for target_y in 0..target_height {
        let source_y = ((target_y as f64 + 0.5) * source_height as f64 / target_height as f64
            - 0.5)
            .clamp(0.0, source_height.saturating_sub(1) as f64);
        let y0 = source_y.floor() as u32;
        let y1 = (y0 + 1).min(source_height - 1);
        let y_weight = source_y - f64::from(y0);
        for target_x in 0..target_width {
            let source_x = ((target_x as f64 + 0.5) * source_width as f64 / target_width as f64
                - 0.5)
                .clamp(0.0, source_width.saturating_sub(1) as f64);
            let x0 = source_x.floor() as u32;
            let x1 = (x0 + 1).min(source_width - 1);
            let x_weight = source_x - f64::from(x0);
            for channel in 0..4 {
                let sample = |x: u32, y: u32| -> f64 {
                    source[((y * source_width + x) * 4 + channel) as usize] as f64
                };
                let top = sample(x0, y0) * (1.0 - x_weight) + sample(x1, y0) * x_weight;
                let bottom = sample(x0, y1) * (1.0 - x_weight) + sample(x1, y1) * x_weight;
                target[((target_y * target_width + target_x) * 4 + channel) as usize] =
                    (top * (1.0 - y_weight) + bottom * y_weight).round() as u8;
            }
        }
    }
    Ok(target)
}

fn encode_png_rgba8(width: u32, height: u32, rgba8: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut bytes), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
        writer
            .write_image_data(rgba8)
            .map_err(|error| error.to_string())?;
    }
    Ok(bytes)
}

fn resource_error(
    code: &'static str,
    stage: &'static str,
    path: &Path,
    message: impl Into<String>,
    next_action: &'static str,
) -> WindowsExecutableResourceError {
    WindowsExecutableResourceError {
        code,
        stage,
        path: path.to_path_buf(),
        message: message.into(),
        next_action,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BuildProfile;
    use editpe::constants::{LANGUAGE_ID_EN_US, RT_RCDATA};
    use editpe::{ResourceData, ResourceTable};
    use std::time::{SystemTime, UNIX_EPOCH};

    const TEST_MANIFEST: &str = "<?xml version=\"1.0\"?><assembly>preserve-me</assembly>";
    const TEST_RCDATA: &[u8] = b"unrelated-resource-preserved";

    #[test]
    fn windows_executable_resources_stamp_and_readback_preserves_template_resources() {
        let root = unique_temp_dir("windows-executable-resources");
        fs::create_dir_all(&root).unwrap();
        let template = root.join("runtime-template.exe");
        let staging = root.join("ComplexShooter.exe");
        fs::copy(std::env::current_exe().unwrap(), &template).unwrap();
        seed_unrelated_resources(&template);
        let template_before = fs::read(&template).unwrap();
        fs::copy(&template, &staging).unwrap();

        let (project_root, application) = sample_application();
        let icon = resolve_release_icon_asset(&project_root, &application.icon).unwrap();
        let readback = stamp_windows_executable_resources(&staging, &application, &icon).unwrap();

        assert_eq!(readback.product_name, application.display_name);
        assert_eq!(readback.fixed_file_version, [1, 0, 0, 0]);
        assert_eq!(readback.fixed_product_version, [1, 0, 0, 0]);
        assert_eq!(readback.icon_sizes, WINDOWS_APPLICATION_ICON_SIZES);
        assert!(readback.manifest_present);
        assert_eq!(fs::read(&template).unwrap(), template_before);
        assert_eq!(read_manifest(&staging), TEST_MANIFEST);
        assert_eq!(read_rcdata(&staging), TEST_RCDATA);
    }

    #[test]
    fn release_icon_asset_resolves_typed_png_and_rejects_path_ref() {
        let (project_root, application) = sample_application();
        let icon = resolve_release_icon_asset(&project_root, &application.icon).unwrap();
        assert_eq!(icon.asset_id, "app-icon");
        assert_eq!(icon.descriptor_path, "Assets/app-icon.asset");
        assert_eq!(icon.source_path, "Assets/Images/app-icon.png");
        assert_eq!(icon.width, icon.height);
        assert!(icon.width >= 256);
        assert_eq!(
            icon.rgba8.len(),
            icon.width as usize * icon.height as usize * 4
        );

        let error = resolve_release_icon_asset(
            &project_root,
            &BuildProfileIconRef {
                asset_id: "../app-icon.png".to_string(),
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "release_icon_asset_missing");
    }

    #[test]
    fn release_icon_asset_resolves_descriptor_through_asset_database() {
        let project_root = unique_temp_dir("release-icon-asset-db");
        let texture_root = project_root.join("Assets/Textures");
        let database_root = project_root.join("Library/AssetPipeline");
        fs::create_dir_all(&texture_root).unwrap();
        fs::create_dir_all(&database_root).unwrap();
        fs::write(
            texture_root.join("nested-icon.png"),
            encode_png_rgba8(8, 8, &vec![255; 8 * 8 * 4]).unwrap(),
        )
        .unwrap();
        fs::write(
            texture_root.join("nested-icon.asset"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "texture-asset.v1",
                "assetId": "nested-icon",
                "assetGuid": "asset-nested-icon",
                "displayName": "Nested Icon",
                "sourceImage": "Assets/Textures/nested-icon.png"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            database_root.join("asset-database.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "project-asset-database.v1",
                "projectId": "release-icon-fixture",
                "databaseVersion": 1,
                "assets": [{
                    "assetGuid": "asset-nested-icon",
                    "assetId": "nested-icon",
                    "displayName": "Nested Icon",
                    "assetType": "texture",
                    "descriptorPath": "Assets/Textures/nested-icon.asset",
                    "sourcePath": "Assets/Textures/nested-icon.png",
                    "metaPath": "Assets/Textures/nested-icon.asset.meta.json",
                    "sourceHash": "sha256:fixture",
                    "sourceByteLength": 1,
                    "importerId": "texture.png.v1",
                    "importerVersion": 1,
                    "settingsHash": "sha256:fixture",
                    "state": "current",
                    "sourceMetadata": { "kind": "localFile" },
                    "license": { "kind": "projectOwned", "identifier": "project-owned" },
                    "directDependencies": []
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let icon = resolve_release_icon_asset(
            &project_root,
            &BuildProfileIconRef {
                asset_id: "nested-icon".to_string(),
            },
        )
        .unwrap();

        assert_eq!(icon.descriptor_path, "Assets/Textures/nested-icon.asset");
        assert_eq!(icon.source_path, "Assets/Textures/nested-icon.png");
        assert_eq!((icon.width, icon.height), (8, 8));
        assert!(build_windows_icon(&icon).is_ok());
    }

    fn sample_application() -> (PathBuf, BuildProfileApplication) {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("samples")
            .join("complex_shooter_project");
        let profile: BuildProfile = serde_json::from_str(
            &fs::read_to_string(project_root.join("BuildProfiles/windows.release.json")).unwrap(),
        )
        .unwrap();
        (project_root, profile.application.unwrap())
    }

    fn seed_unrelated_resources(path: &Path) {
        let mut image = Image::parse_file(path).unwrap();
        let mut resources = image.resource_directory().cloned().unwrap_or_default();
        resources.set_manifest(TEST_MANIFEST).unwrap();

        let mut data = ResourceData::default();
        data.set_data(TEST_RCDATA.to_vec());
        let mut language = ResourceTable::default();
        language.insert(
            ResourceEntryName::ID(LANGUAGE_ID_EN_US as u32),
            ResourceEntry::Data(data),
        );
        let mut identifier = ResourceTable::default();
        identifier.insert(ResourceEntryName::ID(7), ResourceEntry::Table(language));
        resources.root_mut().insert(
            ResourceEntryName::ID(RT_RCDATA as u32),
            ResourceEntry::Table(identifier),
        );
        image.set_resource_directory(resources).unwrap();
        image.write_file(path).unwrap();
    }

    fn read_manifest(path: &Path) -> String {
        let image = Image::parse_file(path).unwrap();
        image
            .resource_directory()
            .unwrap()
            .get_manifest()
            .unwrap()
            .unwrap()
    }

    fn read_rcdata(path: &Path) -> Vec<u8> {
        let image = Image::parse_file(path).unwrap();
        let root = image.resource_directory().unwrap().root();
        root.get(ResourceEntryName::ID(RT_RCDATA as u32))
            .and_then(ResourceEntry::as_table)
            .and_then(|table| table.get(ResourceEntryName::ID(7)))
            .and_then(ResourceEntry::as_table)
            .and_then(|table| table.get(ResourceEntryName::ID(LANGUAGE_ID_EN_US as u32)))
            .and_then(ResourceEntry::as_data)
            .unwrap()
            .data()
            .to_vec()
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{stamp}"))
    }
}
