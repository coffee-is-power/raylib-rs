/* raylib-sys
   build.rs - Cargo build script

Copyright (c) 2018-2019 Paul Clement (@deltaphc)

This software is provided "as-is", without any express or implied warranty. In no event will the authors be held liable for any damages arising from the use of this software.

Permission is granted to anyone to use this software for any purpose, including commercial applications, and to alter it and redistribute it freely, subject to the following restrictions:

  1. The origin of this software must not be misrepresented; you must not claim that you wrote the original software. If you use this software in a product, an acknowledgment in the product documentation would be appreciated but is not required.

  2. Altered source versions must be plainly marked as such, and must not be misrepresented as being the original software.

  3. This notice may not be removed or altered from any source distribution.
*/
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::{env, fs};

/// latest version on github's release page as of time or writing
const LATEST_RAYLIB_VERSION: &str = "3.7.0";
const LATEST_RAYLIB_API_VERSION: &str = "3";

#[cfg(feature = "nobuild")]
fn build_with_cmake(_src_path: &str) {}

#[cfg(not(feature = "nobuild"))]
fn build_with_cmake(src_path: &str) {
    // CMake uses different lib directories on different systems.
    // I do not know how CMake determines what directory to use,
    // so we will check a few possibilities and use whichever is present.
    fn join_cmake_lib_directory(path: PathBuf) -> PathBuf {
        let possible_cmake_lib_directories = ["lib", "lib64", "lib32"];
        for lib_directory in &possible_cmake_lib_directories {
            let lib_path = path.join(lib_directory);
            if lib_path.exists() {
                return lib_path;
            }
        }
        path
    }

    let target = env::var("TARGET").expect("Cargo build scripts always have TARGET");
    let (platform, platform_os) = platform_from_target(&target);

    let mut conf = cmake::Config::new(src_path);
    let builder;
    #[cfg(debug_assertions)]
    {
        builder = conf.profile("Debug");
    }

    #[cfg(not(debug_assertions))]
    {
        builder = conf.profile("Release");
    }

    builder
        .define("BUILD_EXAMPLES", "OFF")
        .define("CMAKE_BUILD_TYPE", "Release")
        // turn off until this is fixed
        .define("SUPPORT_BUSY_WAIT_LOOP", "OFF");

    // Enable wayland cmake flag if feature is specified
    #[cfg(not(target_os = "android"))]
    {
        builder.define("USE_WAYLAND", "ON");
        builder.define("USE_EXTERNAL_GLFW", "ON"); // Necessary for wayland support in my testing
    }

    // This seems redundant, but I felt it was needed incase raylib changes it's default
    #[cfg(not(feature = "wayland"), target_os = "android")]
    builder.define("USE_WAYLAND", "OFF");

    // Scope implementing flags for forcing OpenGL version
    // See all possible flags at https://github.com/raysan5/raylib/wiki/CMake-Build-Options
    {
        #[cfg(feature = "opengl_33")]
        builder.define("OPENGL_VERSION", "3.3");

        #[cfg(feature = "opengl_21")]
        builder.define("OPENGL_VERSION", "2.1");

        // #[cfg(feature = "opengl_11")]
        // builder.define("OPENGL_VERSION", "1.1");

        #[cfg(feature = "opengl_es_20")]
        builder.define("OPENGL_VERSION", "ES 2.0");

        // Once again felt this was necessary incase a default was changed :)
        #[cfg(not(any(
            feature = "opengl_33",
            feature = "opengl_21",
            // feature = "opengl_11",
            feature = "opengl_es_20"
        )))]
        builder.define("OPENGL_VERSION", "OFF");
    }
    match platform {
        Platform::Desktop => conf.define("PLATFORM", "Desktop"),
        Platform::Web => conf.define("PLATFORM", "Web"),
        Platform::RPI => conf.define("PLATFORM", "Raspberry Pi"),
        Platform::Android => {
            // Get the Android NDK path from an environment variable
            let android_ndk_path = env::var("ANDROID_NDK_ROOT")
                .expect("Please set the ANDROID_NDK_HOME environment variable");
            // Get cmake toolchain file path using ANDROID_HOME environment variable
            let toolchain_file = format!("{android_ndk_path}/build/cmake/android.toolchain.cmake");
            // Detect ANDROID_ABI using the target triple
            let android_abi = match target.as_str() {
                "aarch64-linux-android" => "arm64-v8a",
                "armv7-linux-androideabi" => "armeabi-v7a",
                _ => panic!("Unsupported target triple for Android"),
            };
            conf
                .define("ANDROID_ABI", android_abi)
                .define("CMAKE_TOOLCHAIN_FILE", &toolchain_file)
                .define("PLATFORM", "Android")

        }
    };

    let dst = conf.build();
    let dst_lib = join_cmake_lib_directory(dst);
    // on windows copy the static library to the proper file name
    if platform_os == PlatformOS::Windows {
        if Path::new(&dst_lib.join("raylib.lib")).exists() {
            // DO NOTHING
        } else if Path::new(&dst_lib.join("raylib_static.lib")).exists() {
            std::fs::copy(
                dst_lib.join("raylib_static.lib"),
                dst_lib.join("raylib.lib"),
            )
            .expect("failed to create windows library");
        } else if Path::new(&dst_lib.join("libraylib_static.a")).exists() {
            std::fs::copy(
                dst_lib.join("libraylib_static.a"),
                dst_lib.join("libraylib.a"),
            )
            .expect("failed to create windows library");
        } else if Path::new(&dst_lib.join("libraylib.a")).exists() {
            // DO NOTHING
        } else {
            panic!("failed to create windows library");
        }
    } // on web copy libraylib.bc to libraylib.a
    if platform == Platform::Web {
        std::fs::copy(dst_lib.join("libraylib.bc"), dst_lib.join("libraylib.a"))
            .expect("failed to create wasm library");
    }
    // println!("cmake build {}", c.display());
    println!("cargo:rustc-link-search=native={}", dst_lib.display());
}

fn gen_bindings() {
    let target = env::var("TARGET").expect("Cargo build scripts always have TARGET");
    let out_dir =
        PathBuf::from(env::var("OUT_DIR").expect("Cargo build scripts always have an OUT_DIR"));

    let (platform, platform_os) = platform_from_target(&target);

    // Generate bindings
    match (platform, platform_os) {
        (_, PlatformOS::Windows) => {
            fs::write(
                out_dir.join("bindings.rs"),
                include_str!("bindings_windows.rs"),
            )
            .expect("failed to write bindings");
        }
        (_, PlatformOS::Linux) => {
            fs::write(
                out_dir.join("bindings.rs"),
                include_str!("bindings_linux.rs"),
            )
            .expect("failed to write bindings");
        }
        (_, PlatformOS::OSX) => {
            fs::write(out_dir.join("bindings.rs"), include_str!("bindings_osx.rs"))
                .expect("failed to write bindings");
        }
        (Platform::Web, _) => {
            fs::write(out_dir.join("bindings.rs"), include_str!("bindings_web.rs"))
                .expect("failed to write bindings");
        }
        // for other platforms use bindgen and hope it works
        _ => panic!("raylib-rs not supported on your platform"),
    }
}

fn gen_rgui() {
    // Compile the code and link with cc crate
    #[cfg(target_os = "windows")]
    {
        cc::Build::new()
            .file("rgui_wrapper.cpp")
            .include(".")
            .warnings(false)
            // .flag("-std=c99")
            .extra_warnings(false)
            .compile("rgui");
    }
    #[cfg(not(target_os = "windows"))]
    {
        cc::Build::new()
            .file("rgui_wrapper.c")
            .include(".")
            .warnings(false)
            // .flag("-std=c99")
            .extra_warnings(false)
            .compile("rgui");
    }
}

#[cfg(feature = "nobuild")]
fn link(_platform: Platform, _platform_os: PlatformOS) {}

#[cfg(not(feature = "nobuild"))]
fn link(platform: Platform, platform_os: PlatformOS) {
    match platform_os {
        PlatformOS::Windows => {
            println!("cargo:rustc-link-lib=dylib=winmm");
            println!("cargo:rustc-link-lib=dylib=gdi32");
            println!("cargo:rustc-link-lib=dylib=user32");
            println!("cargo:rustc-link-lib=dylib=shell32");
        }
        PlatformOS::Linux => {
            // X11 linking
            #[cfg(not(feature = "wayland"))]
            {
                println!("cargo:rustc-link-search=/usr/local/lib");
                println!("cargo:rustc-link-lib=X11");
            }

            // Wayland linking
            #[cfg(feature = "wayland")]
            {
                println!("cargo:rustc-link-search=/usr/local/lib");
                println!("cargo:rustc-link-lib=wayland-client");
                println!("cargo:rustc-link-lib=glfw"); // Link against locally installed glfw
            }
        }
        PlatformOS::OSX => {
            println!("cargo:rustc-link-search=native=/usr/local/lib");
            println!("cargo:rustc-link-lib=framework=OpenGL");
            println!("cargo:rustc-link-lib=framework=Cocoa");
            println!("cargo:rustc-link-lib=framework=IOKit");
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
            println!("cargo:rustc-link-lib=framework=CoreVideo");
        }
        _ => (),
    }
    if platform == Platform::Web {
        println!("cargo:rustc-link-lib=glfw");
    } else if platform == Platform::RPI {
        println!("cargo:rustc-link-search=/opt/vc/lib");
        println!("cargo:rustc-link-lib=bcm_host");
        println!("cargo:rustc-link-lib=brcmEGL");
        println!("cargo:rustc-link-lib=brcmGLESv2");
        println!("cargo:rustc-link-lib=vcos");
    }

    println!("cargo:rustc-link-lib=static=raylib");
}

fn main() {
    let target = env::var("TARGET").expect("Cargo build scripts always have TARGET");
    let (platform, platform_os) = platform_from_target(&target);

    // Donwload raylib source
    let src = cp_raylib();
    build_with_cmake(&src);

    gen_bindings();

    link(platform, platform_os);

    gen_rgui();
}

// cp_raylib copy raylib to an out dir
fn cp_raylib() -> String {
    let out = env::var("OUT_DIR").unwrap();
    let out = Path::new(&out); //.join("raylib_source");

    let mut options = fs_extra::dir::CopyOptions::new();
    options.skip_exist = true;
    fs_extra::dir::copy("raylib", &out, &options).expect(&format!(
        "failed to copy raylib source to {}",
        &out.to_string_lossy()
    ));

    out.join("raylib").to_string_lossy().to_string()
}

// run_command runs a command to completion or panics. Used for running curl and powershell.
fn run_command(cmd: &str, args: &[&str]) {
    use std::process::Command;
    match Command::new(cmd).args(args).output() {
        Ok(output) => {
            if !output.status.success() {
                let error = std::str::from_utf8(&output.stderr).unwrap();
                panic!("Command '{}' failed: {}", cmd, error);
            }
        }
        Err(error) => {
            panic!("Error running command '{}': {:#}", cmd, error);
        }
    }
}

fn platform_from_target(target: &str) -> (Platform, PlatformOS) {
    let platform = if target.contains("wasm32") {
        // make sure cmake knows that it should bundle glfw in
        // Cargo web takes care of this but better safe than sorry
        env::set_var("EMMAKEN_CFLAGS", "-s USE_GLFW=3");
        Platform::Web
    } else if target.contains("armv7-unknown-linux") {
        Platform::RPI
    } else if target.ends_with("linux-android") {
        Platform::Android
    } else {
        Platform::Desktop
    };

    let platform_os = if platform == Platform::Desktop {
        // Determine PLATFORM_OS in case PLATFORM_DESKTOP selected
        if env::var("OS")
            .unwrap_or("".to_owned())
            .contains("Windows_NT")
            || env::var("TARGET")
                .unwrap_or("".to_owned())
                .contains("windows")
        {
            // No uname.exe on MinGW!, but OS=Windows_NT on Windows!
            // ifeq ($(UNAME),Msys) -> Windows
            PlatformOS::Windows
        } else {
            let un: &str = &uname();
            match un {
                "Linux" => PlatformOS::Linux,
                "FreeBSD" => PlatformOS::BSD,
                "OpenBSD" => PlatformOS::BSD,
                "NetBSD" => PlatformOS::BSD,
                "DragonFly" => PlatformOS::BSD,
                "Darwin" => PlatformOS::OSX,
                _ => panic!("Unknown platform {}", uname()),
            }
        }
    } else if matches!(platform, Platform::RPI | Platform::Android) {
        PlatformOS::Linux
    } else {
        PlatformOS::Unknown
    };

    (platform, platform_os)
}

fn uname() -> String {
    use std::process::Command;
    String::from_utf8_lossy(
        &Command::new("uname")
            .output()
            .expect("failed to run uname")
            .stdout,
    )
    .trim()
    .to_owned()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Platform {
    Web,
    Desktop,
    Android,
    RPI, // raspberry pi
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlatformOS {
    Windows,
    Linux,
    BSD,
    OSX,
    Unknown,
}

#[derive(Debug, PartialEq)]
enum LibType {
    Static,
    _Shared,
}

#[derive(Debug, PartialEq)]
enum BuildMode {
    Release,
    Debug,
}

struct BuildSettings {
    pub platform: Platform,
    pub platform_os: PlatformOS,
    pub bundled_glfw: bool,
}
