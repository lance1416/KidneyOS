use std::{env, path::Path, process};

fn main() {
    println!("cargo:rerun-if-env-changed=KIDNEYOS_TRAMPOLINE_DIR");

    // Host builds (tests, clippy) don't link the trampoline.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("none") {
        return;
    }

    // The trampoline is built and post-processed by the Makefile, which tells us
    // where it put the result.
    let Ok(trampoline_dir) = env::var("KIDNEYOS_TRAMPOLINE_DIR") else {
        fail(
            "KIDNEYOS_TRAMPOLINE_DIR is not set. The kernel links against the trampoline \
             library that the Makefile builds, so build the kernel with `make`, not with \
             cargo directly.",
        );
    };
    let library = Path::new(&trampoline_dir).join("libkidneyos_trampoline.a");
    if !library.is_file() {
        fail(&format!(
            "{} (from KIDNEYOS_TRAMPOLINE_DIR) does not exist. Build the kernel with `make`.",
            library.display()
        ));
    }

    println!("cargo:rustc-link-search=native={trampoline_dir}");
    println!("cargo:rustc-link-lib=static=kidneyos_trampoline");
    println!("cargo:rerun-if-changed={}", library.display());
}

fn fail(message: &str) -> ! {
    eprintln!("error: {message}");
    process::exit(1);
}
