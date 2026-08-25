//! Builds Google Draco and the C entry point in `csrc/`.
//!
//! Draco comes from `third_party/draco`, a submodule pinned to
//! `Construkted-Reality/draco` on branch `fix/options-float-precision`. The
//! fork carries one patch. Read `third_party/draco/CONSTRUKTED-CHANGES.md`
//! before you consider moving the pin.
//!
//! `DRACO_TRANSCODER_SUPPORTED` is not optional here.
//! `ExpertEncoder::SetAttributeGridQuantization`, which is the whole reason
//! this crate exists, is compiled out without it.

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let draco = manifest
        .join("third_party/draco")
        .canonicalize()
        .expect(
            "third_party/draco is missing. Run: \
             git submodule update --init --recursive",
        );

    if !draco
        .join("src/draco/compression/expert_encode.h")
        .is_file()
    {
        panic!(
            "third_party/draco is empty at {}. Run: \
             git submodule update --init --recursive",
            draco.display()
        );
    }
    for sub in ["eigen", "tinygltf", "filesystem"] {
        let path = draco.join("third_party").join(sub);
        if !path
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
        {
            panic!(
                "Draco's own submodule {sub} is empty. Run: \
                 git submodule update --init --recursive"
            );
        }
    }

    let dst = cmake::Config::new(&draco)
        .define("DRACO_TRANSCODER_SUPPORTED", "ON")
        .define("DRACO_TESTS", "OFF")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("CMAKE_POLICY_VERSION_MINIMUM", "3.5")
        .build_target("draco_static")
        .build();

    let build = dst.join("build");
    println!("cargo:rustc-link-search=native={}", build.display());
    println!("cargo:rustc-link-lib=static=draco");

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file(manifest.join("csrc/tileforge_draco.cc"))
        .include(draco.join("src"))
        // draco_features.h is generated into the cmake build directory.
        .include(&build)
        .include(draco.join("third_party/eigen"))
        .warnings(false)
        .compile("tileforge_draco");

    // The C entry point is C++, so the standard library has to come after it.
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("apple") {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if !target.contains("msvc") {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }

    println!("cargo:rerun-if-changed=csrc/tileforge_draco.cc");
    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rerun-if-changed={}",
        draco.join("src/draco/core/options.h").display()
    );
}
