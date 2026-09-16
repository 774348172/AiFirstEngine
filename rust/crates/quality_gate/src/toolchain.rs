use crate::report::{QualityDiagnostic, ToolchainEvidence};

pub const EXPECTED_RUST_RELEASE: &str = "1.96.0";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolchainOutput {
    pub rustc: String,
    pub cargo: String,
    pub clippy: String,
    pub rustfmt: String,
}

pub fn evaluate_toolchain(output: ToolchainOutput) -> (ToolchainEvidence, Vec<QualityDiagnostic>) {
    let release = field(&output.rustc, "release").unwrap_or_default();
    let host = field(&output.rustc, "host").unwrap_or_default();
    let cargo_matches = output
        .cargo
        .split_whitespace()
        .nth(1)
        .is_some_and(|version| version == EXPECTED_RUST_RELEASE);
    let clippy_matches = output
        .clippy
        .split_whitespace()
        .nth(1)
        .is_some_and(|version| version == "0.1.96");
    let components_present = !output.rustfmt.trim().is_empty() && !output.clippy.trim().is_empty();
    let matched =
        release == EXPECTED_RUST_RELEASE && cargo_matches && clippy_matches && components_present;

    let evidence = ToolchainEvidence {
        expected_release: EXPECTED_RUST_RELEASE.to_string(),
        rustc_version: release.clone(),
        cargo_version: output.cargo.trim().to_string(),
        clippy_version: output.clippy.trim().to_string(),
        rustfmt_version: output.rustfmt.trim().to_string(),
        host,
        matched,
    };
    let mut diagnostics = Vec::new();
    if !components_present {
        diagnostics.push(diagnostic(
            "quality_gate.component_missing",
            "rustfmt or Clippy did not return component identity",
            "Install the rustfmt and clippy components declared by rust-toolchain.toml.",
        ));
    }
    if !matched {
        diagnostics.push(diagnostic(
            "quality_gate.toolchain_mismatch",
            format!(
                "expected Rust {EXPECTED_RUST_RELEASE}; observed rustc={release:?}, cargo={:?}, clippy={:?}",
                evidence.cargo_version, evidence.clippy_version
            ),
            "Use the committed rust-toolchain.toml before running the quality gate.",
        ));
    }
    (evidence, diagnostics)
}

fn field(output: &str, name: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == name).then(|| value.trim().to_string())
    })
}

fn diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> QualityDiagnostic {
    QualityDiagnostic {
        code: code.into(),
        message: message.into(),
        next_action: next_action.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_toolchain_matches() {
        let (evidence, diagnostics) = evaluate_toolchain(ToolchainOutput {
            rustc: "rustc 1.96.0\nbinary: rustc\ncommit-hash: abc\nhost: x86_64-pc-windows-msvc\nrelease: 1.96.0\n".to_string(),
            cargo: "cargo 1.96.0 (abc 2026-05-25)".to_string(),
            clippy: "clippy 0.1.96 (abc)".to_string(),
            rustfmt: "rustfmt 1.9.0-stable (abc)".to_string(),
        });
        assert!(evidence.matched);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn wrong_release_fails_closed() {
        let (_, diagnostics) = evaluate_toolchain(ToolchainOutput {
            rustc: "host: x86_64-pc-windows-msvc\nrelease: 1.95.0".to_string(),
            cargo: "cargo 1.95.0".to_string(),
            clippy: "clippy 0.1.95".to_string(),
            rustfmt: "rustfmt 1.8.0".to_string(),
        });
        assert!(diagnostics
            .iter()
            .any(|item| item.code == "quality_gate.toolchain_mismatch"));
    }
}
