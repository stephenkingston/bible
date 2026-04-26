fn main() {
    if let Err(e) = bible::cli::run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
