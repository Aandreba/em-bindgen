use bindgen::builder;
use std::{
    path::{Path, PathBuf},
    thread::Scope,
};

pub fn main() {
    color_eyre::install().unwrap();

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("Could not find emsdk path"));
    let emsdk = PathBuf::from(std::env::var_os("EMSDK").expect("Could not find emsdk path"));

    let emscripten = emsdk.join("upstream/emscripten");
    let sysroot = emscripten.join("cache/sysroot");
    let include = sysroot.join("include");

    std::thread::scope(|s| {
        s.spawn(|| build_bindings(s, &include, &sysroot, &out_dir));
        // s.spawn(|| build_glue(s, &sysroot, &include, &out_dir));
    });
}

fn build_bindings<'scope, 'env>(
    s: &'scope Scope<'scope, 'env>,
    include: &'env Path,
    sysroot: &'env Path,
    out_dir: &'env Path,
) {
    // panic!("{}", include.join("emscripten/emscripten.h").display());
    // Generate functions
    let funcs = s.spawn(|| {
        let mut em_builder = builder()
            .header(include.join("emscripten.h").display().to_string())
            .clang_arg("--target=x86_64-linux")
            .clang_arg(format!("--sysroot={}", sysroot.display()))
            .blocklist_type(".*")
            .allowlist_function("emscripten_.*")
            .default_enum_style(bindgen::EnumVariation::Rust {
                non_exhaustive: true,
            })
            .generate_cstr(true)
            .layout_tests(false);

        if std::env::var_os("CARGO_FEATURE_FETCH").is_some() {
            em_builder = em_builder.header(include.join("emscripten/fetch.h").display().to_string())
        }

        let mut dst = Vec::new();
        em_builder
            .generate()
            .expect("Error generating emscripten bindings")
            .write(Box::new(&mut dst))
            .unwrap();
        dst
    });

    // Generate types
    let types = s.spawn(|| {
        let mut em_builder = builder()
            .header(include.join("emscripten.h").display().to_string())
            .clang_arg(format!("--sysroot={}", sysroot.display()))
            .allowlist_file(format!("{}.*", include.join("emscripten/").display()))
            .blocklist_function(".*")
            .default_enum_style(bindgen::EnumVariation::Rust {
                non_exhaustive: true,
            })
            .generate_cstr(true)
            .layout_tests(false);

        if std::env::var_os("CARGO_FEATURE_FETCH").is_some() {
            em_builder = em_builder.header(include.join("emscripten/fetch.h").display().to_string())
        }

        let mut dst = Vec::new();
        em_builder
            .generate()
            .expect("Error generating emscripten bindings")
            .write(Box::new(&mut dst))
            .unwrap();
        dst
    });

    let mut contents = funcs.join().unwrap();
    contents.append(&mut types.join().unwrap());
    std::fs::write(out_dir.join("emscripten.rs"), contents).unwrap();

    // panic!(
    //     "{}",
    //     std::fs::read_to_string(out_dir.join("emscripten.rs")).unwrap()
    // );
}

fn build_glue<'scope, 'env>(
    s: &'scope Scope<'scope, 'env>,
    sysroot: &'env Path,
    include: &'env Path,
    out_dir: &'env Path,
) {
    // TODO Compile and link

    // Generate bindings
    s.spawn(|| {
        builder()
            .header("glue.h")
            .clang_args(["-xc++", "-std=c++20"])
            .clang_arg(format!("--sysroot={}", sysroot.display()))
            // .clang_arg(format!("-I{}", include.display()))
            .default_enum_style(bindgen::EnumVariation::Rust {
                non_exhaustive: true,
            })
            .generate_cstr(true)
            .generate()
            .expect("Error generating emscripten bindings")
            .write_to_file(out_dir.join("glue.rs"))
            .unwrap();
    });
}
