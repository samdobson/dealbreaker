#!/usr/bin/env bash
# Build an Apple XCFramework (iOS + iOS Simulator + macOS) from crates/dealbreaker-ffi.
# Requires Xcode + rustup targets:
#   rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios \
#     aarch64-apple-darwin x86_64-apple-darwin
set -euo pipefail
cd "$(dirname "$0")/.."

OUT="dist/Dealbreaker.xcframework"
rm -rf "$OUT" dist/ios-sim dist/macos
mkdir -p dist/ios-sim dist/macos

for target in \
  aarch64-apple-ios \
  aarch64-apple-ios-sim \
  x86_64-apple-ios \
  aarch64-apple-darwin \
  x86_64-apple-darwin; do
  cargo build --release -p dealbreaker-ffi --target "$target"
done

lipo -create \
  target/aarch64-apple-ios-sim/release/libdealbreaker_ffi.a \
  target/x86_64-apple-ios/release/libdealbreaker_ffi.a \
  -output dist/ios-sim/libdealbreaker_ffi.a

lipo -create \
  target/aarch64-apple-darwin/release/libdealbreaker_ffi.a \
  target/x86_64-apple-darwin/release/libdealbreaker_ffi.a \
  -output dist/macos/libdealbreaker_ffi.a

xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libdealbreaker_ffi.a \
  -headers crates/dealbreaker-ffi/include \
  -library dist/ios-sim/libdealbreaker_ffi.a \
  -headers crates/dealbreaker-ffi/include \
  -library dist/macos/libdealbreaker_ffi.a \
  -headers crates/dealbreaker-ffi/include \
  -output "$OUT"

echo "built $OUT"
