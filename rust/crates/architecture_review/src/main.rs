use architecture_review::{
    bundle::{
        artifact_expectation, file_digest, prepare_review_bundle, ReviewBundleInput,
        DEFAULT_CONTEXT_LIMIT_BYTES, DEFAULT_SUBJECT_LIMIT,
    },
    resolve_artifact_output, review_to_artifact, ArchitectureReviewHttpConfig, ProviderCredential,
    DEFAULT_PROMPT,
};
use quality_gate::architecture_debt::read_debt_ledger;
use quality_gate::architecture_inventory::build_inventory;
use quality_gate::architecture_policy::load_policy;
use quality_gate::architecture_review::ArchitectureReviewRequest;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

fn main() {
    let code = match run() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    };
    std::process::exit(code);
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("prepare") => prepare(&args),
        Some("review") => review(&args),
        _ => Err("usage: architecture_review <prepare|review> [options]".to_string()),
    }
}

fn prepare(args: &[String]) -> Result<(), String> {
    let workspace_root = argument(args, "--workspace-root")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    ensure_tracked_clean(&workspace_root)?;
    let base_commit = required(args, "--base-commit")?;
    let head_commit = required(args, "--head-commit")?;
    ensure_exact_commit(&workspace_root, &base_commit)?;
    ensure_exact_commit(&workspace_root, &head_commit)?;
    let observed_head = git_output(&workspace_root, &["rev-parse", "HEAD"])?;
    if observed_head != head_commit {
        return Err(format!(
            "head commit mismatch: requested {head_commit}, observed {observed_head}"
        ));
    }
    let request_output =
        resolve_artifact_output(&workspace_root, &required_path(args, "--request-output")?)?;
    let context_output =
        resolve_artifact_output(&workspace_root, &required_path(args, "--context-output")?)?;
    let subject_limit = argument(args, "--subject-limit")
        .map(|value| value.parse::<usize>().map_err(|error| error.to_string()))
        .transpose()?
        .unwrap_or(DEFAULT_SUBJECT_LIMIT);
    let policy_path = workspace_root.join("quality/architecture-policy.v1.toml");
    let coverage_path = workspace_root.join("quality/architecture-review-coverage.v1.json");
    let debt_path = workspace_root.join("quality/architecture-debt-ledger.v1.json");
    let policy = load_policy(&policy_path).map_err(|error| error.to_string())?;
    let metadata = command_output(
        &workspace_root,
        "cargo",
        &["metadata", "--locked", "--format-version", "1", "--no-deps"],
    )?;
    let inventory = build_inventory(&workspace_root, &policy, "engine-strict", &metadata).map_err(
        |diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.observed_evidence))
                .collect::<Vec<_>>()
                .join("; ")
        },
    )?;
    if !inventory.diagnostics.is_empty() {
        return Err("deterministic inventory contains diagnostics".to_string());
    }
    let debt = read_debt_ledger(&debt_path, "1970-01-01")?;
    let bundle = prepare_review_bundle(ReviewBundleInput {
        workspace_root: &workspace_root,
        policy,
        inventory: &inventory,
        debt: &debt,
        base_commit,
        head_commit,
        policy_digest: file_digest(&policy_path)?,
        coverage_digest: file_digest(&coverage_path)?,
        subject_limit,
        context_limit_bytes: DEFAULT_CONTEXT_LIMIT_BYTES,
    })?;
    write_json(&request_output, &bundle.request)?;
    write_text(&context_output, &bundle.context_json)?;
    println!("{}", request_output.display());
    println!("{}", context_output.display());
    Ok(())
}

fn review(args: &[String]) -> Result<(), String> {
    let workspace_root = argument(args, "--workspace-root")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let request_path = required_path(args, "--request")?;
    let context_path = required_path(args, "--context")?;
    let output = required_path(args, "--output")?;
    let output = resolve_artifact_output(&workspace_root, &output)?;
    let request: ArchitectureReviewRequest = serde_json::from_str(
        &fs::read_to_string(&request_path).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let context = fs::read_to_string(&context_path).map_err(|error| error.to_string())?;
    let provider_id = env_required("AI_ENGINE_LLM_PROVIDER_ID")
        .unwrap_or_else(|_| "trusted-local-provider".to_string());
    let config = ArchitectureReviewHttpConfig {
        provider_id,
        base_url: env_required("AI_ENGINE_LLM_BASE_URL")?,
        model: env_required("AI_ENGINE_LLM_MODEL")?,
        timeout_ms: 120_000,
        request_limit_bytes: 2 * 1024 * 1024,
        response_limit_bytes: 2 * 1024 * 1024,
        max_output_tokens: 16_384,
        max_total_tokens: 200_000,
        max_cost_micros: 10_000_000,
        cost_per_1k_tokens_micros: 1000,
    };
    let credential = ProviderCredential::new(env_required("AI_ENGINE_LLM_API_KEY")?);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    let artifact = runtime
        .block_on(review_to_artifact(
            &config,
            &credential,
            request,
            DEFAULT_PROMPT,
            &context,
            now,
            CancellationToken::new(),
        ))
        .map_err(|error| error.to_string())?;
    write_json(&output, &artifact)?;
    if let Some(expectation_output) = argument(args, "--expectation-output") {
        let expectation_output =
            resolve_artifact_output(&workspace_root, &PathBuf::from(expectation_output))?;
        write_json(
            &expectation_output,
            &artifact_expectation(&artifact.request, now),
        )?;
        println!("{}", expectation_output.display());
    }
    println!("{}", output.display());
    Ok(())
}

fn required(args: &[String], name: &str) -> Result<String, String> {
    argument(args, name).ok_or_else(|| format!("missing required {name}"))
}

fn required_path(args: &[String], name: &str) -> Result<PathBuf, String> {
    argument(args, name)
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing required {name}"))
}

fn argument(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == name)
        .map(|window| window[1].clone())
}

fn env_required(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("required environment variable {name} is not configured"))
}

fn ensure_tracked_clean(workspace_root: &PathBuf) -> Result<(), String> {
    for args in [
        ["diff", "--quiet"].as_slice(),
        ["diff", "--cached", "--quiet"].as_slice(),
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(workspace_root)
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err("tracked workspace must be clean before preparing a review".to_string());
        }
    }
    Ok(())
}

fn ensure_exact_commit(workspace_root: &PathBuf, commit: &str) -> Result<(), String> {
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "commit {commit:?} is not an exact 40-character SHA"
        ));
    }
    command_output(
        workspace_root,
        "git",
        &["cat-file", "-e", &format!("{commit}^{{commit}}")],
    )?;
    Ok(())
}

fn git_output(workspace_root: &PathBuf, args: &[&str]) -> Result<String, String> {
    String::from_utf8(command_output(workspace_root, "git", args)?)
        .map(|value| value.trim().to_string())
        .map_err(|error| error.to_string())
}

fn command_output(
    workspace_root: &PathBuf,
    program: &str,
    args: &[&str],
) -> Result<Vec<u8>, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(workspace_root)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn write_json(path: &PathBuf, value: &impl serde::Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    write_text(path, &format!("{json}\n"))
}

fn write_text(path: &PathBuf, value: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, value).map_err(|error| error.to_string())
}
