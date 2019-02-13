use std::env;

fn main() {
    println!(
        "cargo:rustc-link-search={}/libs",
        env::var("CARGO_MANIFEST_DIR").unwrap()
    );
}
