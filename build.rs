use bindgen::builder;
use std::{
    path::{Path, PathBuf},
    thread::Scope,
};

pub fn main() {
    color_eyre::install().unwrap();
    println!("cargo::rerun-if-changed=src/file_dialog.cpp");
    println!("cargo::rerun-if-changed=src/file_dialog.h");
    println!("cargo::rerun-if-changed=src/chrono.cpp");
    println!("cargo::rerun-if-changed=src/chrono.h");

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("Could not find emsdk path"));
    let emsdk = PathBuf::from(std::env::var_os("EMSDK").expect("Could not find emsdk path"));

    let emscripten = emsdk.join("upstream/emscripten");
    let sysroot = emscripten.join("cache/sysroot");
    let include = sysroot.join("include");

    std::thread::scope(|s| {
        build_bindings(s, &include, &sysroot, &out_dir);
        build_file_dialog(s, &sysroot, &out_dir);

        if std::env::var_os("CARGO_FEATURE_CHRONO").is_some() {
            build_chrono(s, &sysroot, &out_dir);
        }
        if std::env::var_os("CARGO_FEATURE_FETCH").is_some() {
            build_fetch(s, &sysroot, &out_dir);
        }
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
            // .header("glue.h")
            .clang_arg("--target=x86_64-linux")
            .clang_arg(format!("--sysroot={}", sysroot.display()))
            .blocklist_type(".*")
            .allowlist_function("(_emscripten|emscripten|em|glue)_.*")
            .default_enum_style(bindgen::EnumVariation::Rust {
                non_exhaustive: true,
            })
            .generate_cstr(true)
            .layout_tests(false);

        if std::env::var_os("CARGO_FEATURE_HTML").is_some() {
            em_builder = em_builder.header(include.join("emscripten/html5.h").display().to_string())
        }

        if std::env::var_os("CARGO_FEATURE_PROXYING").is_some() {
            em_builder =
                em_builder.header(include.join("emscripten/proxying.h").display().to_string())
        }

        if std::env::var_os("CARGO_FEATURE_CONSOLE").is_some() {
            em_builder =
                em_builder.header(include.join("emscripten/console.h").display().to_string())
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
            // .header("glue.h")
            .clang_arg(format!("--sysroot={}", sysroot.display()))
            .allowlist_file(format!(
                "(.*glue.h|{}.*)",
                include.join("emscripten/").display()
            ))
            .blocklist_function(".*")
            .blocklist_type("pthread_t")
            .default_enum_style(bindgen::EnumVariation::Rust {
                non_exhaustive: true,
            })
            .generate_cstr(true)
            .layout_tests(false);

        if std::env::var_os("CARGO_FEATURE_HTML").is_some() {
            em_builder = em_builder.header(include.join("emscripten/html5.h").display().to_string())
        }

        if std::env::var_os("CARGO_FEATURE_PROXYING").is_some() {
            em_builder =
                em_builder.header(include.join("emscripten/proxying.h").display().to_string())
        }

        if std::env::var_os("CARGO_FEATURE_CONSOLE").is_some() {
            em_builder =
                em_builder.header(include.join("emscripten/console.h").display().to_string())
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

fn build_file_dialog<'scope, 'env>(
    s: &'scope Scope<'scope, 'env>,
    sysroot: &'env Path,
    out_dir: &'env Path,
) {
    // TYPES
    s.spawn(|| {
        builder()
            .header("file_dialog.h")
            .clang_arg(format!("--sysroot={}", sysroot.display()))
            .clang_arg("-fvisibility=default")
            .clang_arg("--target=wasm32-emscripten")
            .default_enum_style(bindgen::EnumVariation::Rust {
                non_exhaustive: true,
            })
            .generate_cstr(true)
            .layout_tests(false)
            .generate()
            .unwrap()
            .write_to_file(out_dir.join("file_dialog.rs"))
            .unwrap();
    });

    // COMPILE
    s.spawn(|| {
        cc::Build::new()
            .file("file_dialog.cpp")
            .flag("-fvisibility=default")
            .flag(format!("--sysroot={}", sysroot.display()))
            .compile("file_dialog");
    });
}

fn build_chrono<'scope, 'env>(
    s: &'scope Scope<'scope, 'env>,
    sysroot: &'env Path,
    out_dir: &'env Path,
) {
    // TYPES
    s.spawn(|| {
        builder()
            .header("chrono.h")
            .clang_arg(format!("--sysroot={}", sysroot.display()))
            .clang_arg("-fvisibility=default")
            .clang_arg("--target=wasm32-emscripten")
            .default_enum_style(bindgen::EnumVariation::Rust {
                non_exhaustive: true,
            })
            .generate_cstr(true)
            .layout_tests(false)
            .generate()
            .unwrap()
            .write_to_file(out_dir.join("chrono.rs"))
            .unwrap();
    });

    // COMPILE
    s.spawn(|| {
        cc::Build::new()
            .file("chrono.cpp")
            .flag("-fvisibility=default")
            .flag(format!("--sysroot={}", sysroot.display()))
            .compile("chrono");
    });
}

fn build_fetch<'scope, 'env>(
    s: &'scope Scope<'scope, 'env>,
    sysroot: &'env Path,
    out_dir: &'env Path,
) {
    // TYPES
    s.spawn(|| {
        builder()
            .header("fetch.h")
            .clang_arg(format!("--sysroot={}", sysroot.display()))
            .clang_arg("-fvisibility=default")
            .clang_arg("--target=wasm32-emscripten")
            .default_enum_style(bindgen::EnumVariation::Rust {
                non_exhaustive: true,
            })
            .generate_cstr(true)
            .layout_tests(false)
            .generate()
            .unwrap()
            .write_to_file(out_dir.join("fetch.rs"))
            .unwrap();
    });

    // COMPILE
    s.spawn(|| {
        cc::Build::new()
            .file("fetch.cpp")
            .flag("-fvisibility=default")
            .flag(format!("--sysroot={}", sysroot.display()))
            .compile("fetch");
    });
}
