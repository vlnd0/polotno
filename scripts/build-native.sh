#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
sdk_root="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [[ -z "$sdk_root" && -f local.properties ]]; then
  sdk_root="$(sed -n 's/^sdk.dir=//p' local.properties)"
fi
: "${sdk_root:?Set ANDROID_HOME or sdk.dir in local.properties}"
ndk_root="${ANDROID_NDK_HOME:-$sdk_root/ndk/27.0.12077973}"
case "$(uname -s)" in Darwin) ndk_host=darwin-x86_64;; Linux) ndk_host=linux-x86_64;; *) echo 'Use a macOS or Linux host for the prototype' >&2; exit 1;; esac
toolchain="$ndk_root/toolchains/llvm/prebuilt/$ndk_host/bin"
cargo fetch --locked
export CC_aarch64_linux_android="$toolchain/aarch64-linux-android26-clang"
export AR_aarch64_linux_android="$toolchain/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
export CC_armv7_linux_androideabi="$toolchain/armv7a-linux-androideabi26-clang"
export AR_armv7_linux_androideabi="$toolchain/llvm-ar"
export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="$CC_armv7_linux_androideabi"
export CC_x86_64_linux_android="$toolchain/x86_64-linux-android26-clang"
export AR_x86_64_linux_android="$toolchain/llvm-ar"
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$CC_x86_64_linux_android"
for spec in 'armv7-linux-androideabi armeabi-v7a' 'aarch64-linux-android arm64-v8a' 'x86_64-linux-android x86_64'; do
  read -r target abi <<< "$spec"
  cargo build --locked --release --lib -p polotno-core --target "$target"
  mkdir -p "android/app/build/rustJniLibs/$abi"
  cp "target/$target/release/libpolotno_core.so" "android/app/build/rustJniLibs/$abi/"
done
python3 scripts/bundle-rust-licenses.py
