use std::{env, fs, path::Path, process};

fn main() {
    write_init_programs();
}

/// Writes `init_programs.rs` to OUT_DIR: the user programs the kernel starts
/// (see `INIT_PROGRAMS` in main.rs), as an array of `include_bytes!`s.
///
/// They are the comma-separated names in `KIDNEYOS_INIT` (default: `pipes`),
/// looked up among the user programs the Makefile builds (see programs.mk). Host builds (tests, clippy) don't embed
/// any.
fn write_init_programs() {
    println!("cargo:rerun-if-env-changed=KIDNEYOS_INIT");
    let out = env::var("OUT_DIR").expect("cargo sets OUT_DIR");

    let programs = if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("none") {
        let programs_dir = Path::new(&env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"))
            .join("../build/programs");
        let (rust_dir, native_dir) = ("x86_64-unknown-linux-gnu/release", "bin");
        let names = env::var("KIDNEYOS_INIT").unwrap_or_default();
        let names = if names.is_empty() { "pipes" } else { &names };
        names
            .split(',')
            .map(|name| {
                let candidates = [
                    programs_dir.join(rust_dir).join(name),
                    programs_dir.join(native_dir).join(name),
                ];
                let Some(path) = candidates.iter().find(|path| path.is_file()) else {
                    fail(&format!(
                        "init program {name:?} (from KIDNEYOS_INIT) not found at {} or {}; \
                         build the kernel with `make`, which builds the user programs first",
                        candidates[0].display(),
                        candidates[1].display(),
                    ));
                };
                println!("cargo:rerun-if-changed={}", path.display());
                format!(
                    "include_bytes!({:?}).as_slice(),\n",
                    path.display().to_string()
                )
            })
            .collect::<String>()
    } else {
        String::new()
    };

    fs::write(
        Path::new(&out).join("init_programs.rs"),
        format!("&[\n{programs}]\n"),
    )
    .expect("writing to OUT_DIR works");
}

fn fail(message: &str) -> ! {
    eprintln!("error: {message}");
    process::exit(1);
}
