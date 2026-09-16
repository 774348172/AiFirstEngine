fn main() {
    let exit_code = quality_gate::run_cli(std::env::args().skip(1));
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}
