fn main() {
    // These don't propagate from vm-interp's build.rs to the final binary
    println!("cargo:rustc-link-arg=-znostart-stop-gc");
    println!("cargo:rustc-link-arg=-Wl,--defsym=__start_custom_data=0");
    println!("cargo:rustc-link-arg=-Wl,--defsym=__stop_custom_data=0");
}
