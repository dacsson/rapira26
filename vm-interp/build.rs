use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let wrapper_header = format!("{manifest_dir}/../runtime-c/rt.h");

    // Tell cargo to look for shared libraries in the specified directory
    println!("cargo:rustc-link-search={manifest_dir}/../runtime-c/");

    // Tell cargo to tell rustc to link the system bzip2
    // shared library.
    println!("cargo:rustc-link-lib=lamart");

    // GC-specific linker flags
    println!("cargo:rustc-link-arg=-znostart-stop-gc");
    println!("cargo:rustc-link-arg=-Wl,--defsym=__start_custom_data=0");
    println!("cargo:rustc-link-arg=-Wl,--defsym=__stop_custom_data=0");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        .use_core()
        // The input header we would like to generate
        // bindings for.
        .header(wrapper_header)
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
