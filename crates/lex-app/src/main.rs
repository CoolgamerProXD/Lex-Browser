fn main() {
    if let Err(error) = lex_app::run() {
        eprintln!("LEX: {error}");
        std::process::exit(1);
    }
}
