use std::env;
use std::process::Command;

fn main() {
    let outdir: String = env::var("OUT_DIR").unwrap();
    // build avm library
    Command::new("make")
        .arg("-C")
        .arg("libs/avmjni")
        .arg(format!("{}={}", "OUTDIR", outdir))
        .status()
        .expect("failed to build avm");

    println!("cargo:rustc-link-search=native={}", outdir);
    println!("cargo:rustc-link-lib=static=avmjni");
    
    // fetch avm libs
    Command::new("wget")
            .arg("https://github.com/aionnetwork/AVM/archive/1.0.tar.gz")
            .args(["-O", "/tmp/avm.tar.gz"].iter())
            .status()
            .expect("update AVM error");

    // unpack avm package and put the jars in libs dir
    Command::new("tar")
    .arg("-xvf")
    .arg("/tmp/avm.tar.gz")
    .args(["-C", "/tmp/"].iter());
}
