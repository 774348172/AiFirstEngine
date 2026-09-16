use code_hygiene::v2::{generate_hygiene_report_v2, report_v2_to_json, write_report_v2_json};
use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();
    let root = arg_value(&args, "--root")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("crates"));
    let output = arg_value(&args, "--output").map(PathBuf::from);

    let report = match generate_hygiene_report_v2(&root) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("failed to generate code hygiene report: {error}");
            std::process::exit(1);
        }
    };

    if let Some(output) = output {
        if let Err(error) = write_report_v2_json(&report, output) {
            eprintln!("failed to write code hygiene report: {error}");
            std::process::exit(1);
        }
    } else {
        match report_v2_to_json(&report) {
            Ok(json) => print!("{json}"),
            Err(error) => {
                eprintln!("failed to serialize code hygiene report: {error}");
                std::process::exit(1);
            }
        }
    }
}

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == key)
        .map(|window| window[1].clone())
}
