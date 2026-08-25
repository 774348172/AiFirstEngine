use editor_core::{
    AndroidDevAbi, AndroidDevExportPipeline, AndroidDevExportRequest, AndroidDevExportStatus,
};
use std::ffi::OsString;
use std::path::PathBuf;

fn main() {
    let request = match parse_request(std::env::args_os().skip(1)) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("usage: export_android_dev <project-root> [--abi arm64-v8a|x86_64]");
            std::process::exit(2);
        }
    };
    let report = AndroidDevExportPipeline::export(request);
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("Android export report must serialize")
    );
    if report.status != AndroidDevExportStatus::Success {
        std::process::exit(1);
    }
}

fn parse_request(
    args: impl IntoIterator<Item = OsString>,
) -> Result<AndroidDevExportRequest, String> {
    let mut args = args.into_iter();
    let Some(project_root) = args.next().map(PathBuf::from) else {
        return Err("android_export.project_root_missing".to_string());
    };
    let abi = match args.next() {
        None => AndroidDevAbi::Arm64V8a,
        Some(flag) if flag == "--abi" => {
            let value = args
                .next()
                .ok_or_else(|| "android_export.abi_value_missing".to_string())?;
            let value = value
                .to_str()
                .ok_or_else(|| "android_export.abi_value_invalid_utf8".to_string())?;
            AndroidDevAbi::parse(value)?
        }
        Some(value) => {
            return Err(format!(
                "android_export.unexpected_argument: {}",
                value.to_string_lossy()
            ));
        }
    };
    if let Some(value) = args.next() {
        return Err(format!(
            "android_export.unexpected_argument: {}",
            value.to_string_lossy()
        ));
    }
    Ok(AndroidDevExportRequest::for_abi(project_root, abi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_defaults_to_arm64_and_requires_explicit_x86_64() {
        let arm64 = parse_request([OsString::from("project")]).unwrap();
        assert_eq!(arm64.abi, AndroidDevAbi::Arm64V8a);
        assert_eq!(arm64.output_dir, PathBuf::from("project/Build/Android/dev"));

        let emulator = parse_request([
            OsString::from("project"),
            OsString::from("--abi"),
            OsString::from("x86_64"),
        ])
        .unwrap();
        assert_eq!(emulator.abi, AndroidDevAbi::X86_64);
        assert_eq!(
            emulator.output_dir,
            PathBuf::from("project/Build/Android/emulator-x86_64")
        );
    }

    #[test]
    fn cli_rejects_unknown_abi_and_extra_arguments() {
        assert_eq!(
            parse_request([
                OsString::from("project"),
                OsString::from("--abi"),
                OsString::from("armeabi-v7a"),
            ])
            .unwrap_err(),
            "android_export.unsupported_abi: armeabi-v7a"
        );
        assert!(parse_request([
            OsString::from("project"),
            OsString::from("--abi"),
            OsString::from("x86_64"),
            OsString::from("extra"),
        ])
        .unwrap_err()
        .contains("unexpected_argument"));
    }
}
