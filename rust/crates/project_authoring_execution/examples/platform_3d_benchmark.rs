use project_authoring_execution::platform_3d_benchmark::{project_source_digest, run_request};
fn main() {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() == 2 && args[0] == "--source-digest" {
        match project_source_digest(std::path::Path::new(&args[1])) {
            Ok(digest) => println!("{digest}"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.len() != 1 {
        eprintln!("usage: platform_3d_benchmark <absolute-request.json>");
        std::process::exit(2);
    }
    match run_request(std::path::Path::new(&args[0])) {
        Ok(output) => println!("{}", output.display()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
