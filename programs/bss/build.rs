fn main() {
    // Load well above the user stack VMA (0x100000..0x1100000), which would
    // otherwise demand-page any part of .bss the ELF loader failed to map.
    println!("cargo:rustc-link-arg-bins=-Wl,--image-base=0x20000000");
}
