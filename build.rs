use std::{env, fs, path::PathBuf};

struct Library(&'static str, &'static str);

const fn link_kind() -> &'static str {
    if cfg!(feature = "static-link") {
        "static"
    } else {
        "dylib"
    }
}

fn lib_names() -> Vec<Library> {
    let stdcxx_link_kind = if link_kind() == "static" {
        "static:-bundle"
    } else {
        "dylib"
    };

    let mut libraries = Vec::new();

    libraries.push(Library("assimp", link_kind()));

    if cfg!(all(unix, not(target_os = "macos")))
        || (cfg!(target_os = "windows") && env::var("TARGET").unwrap().ends_with("-gnu"))
    {
        libraries.push(Library("stdc++", stdcxx_link_kind));
    }

    if cfg!(target_os = "macos") {
        libraries.push(Library("c++", stdcxx_link_kind));
    }

    libraries
}

#[cfg(feature = "build-assimp")]
fn build_from_source() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Build static libs?
    let build_shared = if link_kind() == "static" { "OFF" } else { "ON" };

    // CMake
    let mut cmake = cmake::Config::new("assimp");
    cmake
        .profile("Release")
        .static_crt(true)
        .out_dir(out_dir.join(link_kind()))
        .define("BUILD_SHARED_LIBS", build_shared)
        .define("ASSIMP_BUILD_ASSIMP_TOOLS", "OFF")
        .define("ASSIMP_BUILD_TESTS", "OFF")
        .define("ASSIMP_BUILD_ZLIB", "OFF")
        // Disable being overly strict with warnings, which can cause build issues
        // such as: https://github.com/assimp/assimp/issues/5315
        .define("ASSIMP_WARNINGS_AS_ERRORS", "OFF")
        .define("LIBRARY_SUFFIX", "");

    if let Ok(zlib_include_dir) = env::var("DEP_Z_INCLUDE") {
        // Use the zlib provided by libz-sys, if it built zlib from source and couldn't find it on
        // the system. Inspired by the following example:
        // https://doc.rust-lang.org/cargo/reference/build-script-examples.html#using-another-sys-crate
        cmake.define(
            "ZLIB_ROOT",
            PathBuf::from(zlib_include_dir).parent().unwrap(),
        );
    }

    if cfg!(target_env = "msvc") {
        // Find Ninja
        if which::which("ninja").is_ok() {
            env::set_var("CMAKE_GENERATOR", "Ninja");
        }
    }

    let cmake_dir = cmake.build();

    println!(
        "cargo:rustc-link-search=native={}",
        cmake_dir.join("lib").display()
    );

    println!(
        "cargo:rustc-link-search=native={}",
        cmake_dir.join("bin").display()
    );
}

#[cfg(all(feature = "prebuilt", not(feature = "build-assimp")))]
fn link_from_package() {
    use flate2::read::GzDecoder;

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap();
    let crate_version = env::var("CARGO_PKG_VERSION").unwrap();
    let archive_name = format!(
        "russimp-{}-{}-{}.tar.gz",
        crate_version,
        target,
        link_kind()
    );

    let ar_src_dir;

    if option_env!("RUSSIMP_PACKAGE_DIR").is_some() {
        ar_src_dir = PathBuf::from(env::var("RUSSIMP_PACKAGE_DIR").unwrap());
    } else {
        ar_src_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let dl_link = format!("https://github.com/AlexTMjugador/russimp-sys/releases/download/v{crate_version}/{archive_name}");

        match fs::File::open(ar_src_dir.join(&archive_name)) {
            Ok(_) => {}
            Err(_) => {
                let resp = ureq::get(&dl_link).call().unwrap();

                let mut file = fs::File::create(ar_src_dir.join(&archive_name)).unwrap();
                std::io::copy(&mut resp.into_reader(), &mut file).unwrap();
            }
        }
    }

    dbg!(ar_src_dir.join(&archive_name));

    let file = fs::File::open(ar_src_dir.join(&archive_name)).unwrap();
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    let ar_dest_dir = out_dir.join(link_kind());

    archive.unpack(&ar_dest_dir).unwrap();

    fs::rename(ar_dest_dir.join("bindings.rs"), out_dir.join("bindings.rs")).expect(
        "missing bindings from archive, are the custom prebuilt tarballs being downloaded?",
    );

    println!(
        "cargo:rustc-link-search=native={}",
        ar_dest_dir.join("lib").display()
    );

    println!(
        "cargo:rustc-link-search=native={}",
        ar_dest_dir.join("bin").display()
    );
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    fs::write(out_dir.join("target"), env::var("TARGET").unwrap()).unwrap();

    // Look for assimp lib in Brew install paths on MacOS.
    // See https://stackoverflow.com/questions/70497361/homebrew-mac-m1-cant-find-installs
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    println!("cargo:rustc-link-search=native=/opt/homebrew/lib/");

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    println!("cargo:rustc-link-search=native=/opt/brew/lib/");

    #[cfg(feature = "build-assimp")]
    build_from_source();

    #[cfg(all(feature = "prebuilt", not(feature = "build-assimp")))]
    link_from_package();

    #[cfg(not(any(feature = "build-assimp", feature = "prebuilt")))]
    compile_error!("Either feature `build-assimp` or `prebuilt` must be enabled for this crate");

    #[cfg(feature = "build-assimp")]
    bindgen::builder()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", out_dir.join(link_kind()).join("include").display()))
        .clang_arg(format!("-I{}", "assimp/include"))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_type("ai.*")
        .allowlist_function("ai.*")
        .allowlist_var("ai.*")
        .allowlist_var("AI_.*")
        .derive_partialeq(true)
        .derive_eq(true)
        .derive_hash(true)
        .derive_debug(true)
        .generate()
        .unwrap()
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Could not generate russimp bindings, for details see https://github.com/jkvargas/russimp-sys");

    for n in lib_names().iter() {
        println!("cargo:rustc-link-lib={}={}", n.1, n.0);
    }
}
