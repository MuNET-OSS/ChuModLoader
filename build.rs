fn main() {
    println!(
        "cargo:rustc-cdylib-link-arg=/DEF:{}",
        std::path::Path::new("src/version.def")
            .canonicalize()
            .unwrap()
            .display()
    );
}
