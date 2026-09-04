use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let wrapper_header = format!("{manifest_dir}/../../runtime/runtime.h");
    let target = env::var("TARGET").expect("Cargo did not provide TARGET");
    let host = env::var("HOST").expect("Cargo did not provide HOST");
    let is_wasi = target == "wasm32-wasip1";
    let runtime_library_dir = if is_wasi { "lib-wasm" } else { "lib" };
    let runtime_library =
        format!("{manifest_dir}/../../runtime/{runtime_library_dir}/librapruntime.a");

    println!("cargo:rustc-link-search={manifest_dir}/../../runtime/{runtime_library_dir}");
    // Rebuild on runtime library changes
    println!("cargo:rerun-if-changed={runtime_library}");

    println!("cargo:rustc-link-lib=rapruntime");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings_builder = bindgen::Builder::default()
        .use_core()
        // The input header we would like to generate
        // bindings for.
        .header(wrapper_header)
        .clang_arg(format!("--target={host}"))
        .allowlist_type("RAP_Value")
        .allowlist_type("RAP_Object")
        .allowlist_var("RAP_TAG_MASK")
        .allowlist_function("RAP_.*")
        // FIXME: for some reason when i directly use WASI as a target
        // the bindings for functions are not generated, so we use this workaround
        // untill i wrap my head why its happening
        .opaque_type("RAP_Object")
        .opaque_type("RAP_Callable")
        .opaque_type("RAP_Tuple")
        .opaque_type("RAP_Slice")
        .opaque_type("RAP_Variant");

    let bindings = bindings_builder
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
