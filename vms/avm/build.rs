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
    println!("cargo:rustc-link-lib=avmjni");

    // fetch jni jar
    let mut jni_jar_path = env!("CARGO_MANIFEST_DIR").to_string();
    jni_jar_path.extend("/libs/aion_vm/org-aion-avm-jni.jar".chars());
    println!("{:?}", jni_jar_path);
    Command::new("wget")
            .arg("https://github.com/aion-camus/rust_avm/releases/download/v0.5.0/org-aion-avm-jni.jar")
            .args(["-O", &jni_jar_path].iter())
            .status()
            .expect("fetch jni jar error");
    
    // // fetch avm libs
    // Command::new("wget")
    //         .arg("https://github.com/aionnetwork/AVM/archive/1.0.tar.gz")
    //         .args(["-O", "/tmp/avm.tar.gz"].iter())
    //         .status()
    //         .expect("fetch AVM error");

    // // unpack avm package and put the jars in libs dir
    // Command::new("tar")
    // .arg("-xvf")
    // .arg("/tmp/avm.tar.gz")
    // .args(["-C", "/tmp/"].iter());
}
