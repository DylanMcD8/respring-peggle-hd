#!/bin/bash
set -euo pipefail
project_root=$(cd "$(dirname "$0")/.." && pwd)
sdk=${1:-iphoneos}
case "$sdk" in
  iphoneos) target=aarch64-apple-ios; platform_flag=ios ;;
  iphonesimulator) target=aarch64-apple-ios-sim; platform_flag=ios-simulator ;;
  *) echo 'Usage: ios/build.sh [iphoneos|iphonesimulator]' >&2; exit 2 ;;
esac
repository=$(cd "$project_root/.." && pwd -P)
cd "$repository"
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
output_root=${PEGGLE_NATIVE_BUILD:-"$repository/.build"}
export CARGO_TARGET_DIR="$output_root/rust"
export IPHONEOS_DEPLOYMENT_TARGET=15.0
export SDKROOT=$(xcrun --sdk "$sdk" --show-sdk-path)
sdl_source="$repository/Vendor/SDL2"
build_dir="$output_root/$sdk"
mkdir -p "$build_dir"
# Optional read-only reuse of unchanged SDL; omit to rebuild from source.
sdl_install=${PEGGLE_REUSE_SDL_ROOT:-$build_dir/sdl-scenes-install}
fps_sdl_source_hash=$(shasum -a 256 "$sdl_source/src/video/uikit/SDL_uikitopenglview.m" | cut -d ' ' -f 1)
if [ -n "${PEGGLE_REUSE_SDL_ROOT:-}" ]; then
  witness="$sdl_install/peggle-fps-sdl-source-sha256.txt"
  if [ ! -f "$witness" ] || [ "$(cat "$witness")" != "$fps_sdl_source_hash" ]; then
    echo 'C28 requires its SDL presentation-success hook; omit PEGGLE_REUSE_SDL_ROOT to build the private fork.' >&2
    exit 2
  fi
fi
if [ -n "${PEGGLE_REUSE_SDL_ROOT:-}" ]; then
  test -f "$sdl_install/lib/libSDL2.a"
  test -f "$sdl_install/include/SDL2/SDL.h"
else
cmake -S "$sdl_source" -B "$build_dir/sdl-scenes" -G Ninja \
  -DCMAKE_SYSTEM_NAME=iOS -DCMAKE_OSX_SYSROOT="$SDKROOT" \
  -DCMAKE_OSX_ARCHITECTURES=arm64 -DCMAKE_OSX_DEPLOYMENT_TARGET=15.0 \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
  -DCMAKE_INSTALL_PREFIX="$sdl_install" \
  -DSDL_SHARED=OFF -DSDL_STATIC=ON -DSDL_TEST=OFF -DSDL_TESTS=OFF \
  -DSDL_HIDAPI=OFF -DSDL_RENDER_METAL=OFF
cmake --build "$build_dir/sdl-scenes" --parallel 8
cmake --install "$build_dir/sdl-scenes"
printf '%s\n' "$fps_sdl_source_hash" > "$sdl_install/peggle-fps-sdl-source-sha256.txt"
export CHUZZLE_SDL_LIBRARY="$sdl_install/lib/libSDL2.a"
fi
export CHUZZLE_SDL_LIBRARY="$sdl_install/lib/libSDL2.a"
# Encoded flags preserve paths containing spaces (including Peggle HD).
unset RUSTFLAGS
rust_flags=(-L "native=$sdl_install/lib" --remap-path-prefix "$HOME=~" --remap-path-prefix "$repository=.")
for framework in UIKit Foundation OpenGLES OpenAL QuartzCore AVFoundation AudioToolbox CoreAudio CoreGraphics CoreMotion GameController CoreHaptics CoreBluetooth Metal Security; do
  rust_flags+=(-C link-arg=-framework -C "link-arg=$framework")
done
printf -v CARGO_ENCODED_RUSTFLAGS '%s\x1f' "${rust_flags[@]}"
export CARGO_ENCODED_RUSTFLAGS=${CARGO_ENCODED_RUSTFLAGS%$'\x1f'}
# sdl2-sys bundles the external SDL archive into its rlib. Cargo does not track
# changes to that external file, so invalidate this small package when the local
# SDL implementation changes, otherwise a stale copy reaches the final staticlib.
sdl_archive_hash=$(shasum -a 256 "$CHUZZLE_SDL_LIBRARY" | cut -d ' ' -f 1)
sdl_hash_file="$build_dir/sdl-scenes-rust-hash.txt"
if [ ! -f "$sdl_hash_file" ] || [ "$(cat "$sdl_hash_file")" != "$sdl_archive_hash" ]; then
  cargo clean --manifest-path "$project_root/ios/rust-entry/Cargo.toml" \
    --target "$target" --release -p sdl2-sys
fi
cargo build --manifest-path "$project_root/ios/rust-entry/Cargo.toml" \
  --target "$target" --release --offline --locked
printf '%s\n' "$sdl_archive_hash" > "$sdl_hash_file"
app="$build_dir/PeggleHD.app"
python3 "$repository/Scripts/prepare_app.py" "$app"
mkdir -p "$app"
xcrun --sdk "$sdk" clang -ffile-prefix-map="$HOME=~" -arch arm64 -isysroot "$SDKROOT" \
  -target "arm64-apple-$platform_flag" -mios-version-min=15.0 -O2 -fobjc-arc \
  -c "$project_root/ios/fps_overlay.m" -o "$build_dir/fps-overlay.o"
xcrun --sdk "$sdk" clang -ffile-prefix-map="$HOME=~" -arch arm64 -isysroot "$SDKROOT" \
  -target "arm64-apple-$platform_flag" -mios-version-min=15.0 -O2 -std=c11 \
  -c "$project_root/ios/fps_counter.c" -o "$build_dir/fps-counter.o"
xcrun --sdk "$sdk" clang -ffile-prefix-map="$HOME=~" -arch arm64 -isysroot "$SDKROOT" \
  -target "arm64-apple-$platform_flag" -mios-version-min=15.0 -O2 -fobjc-arc \
  -c "$project_root/ios/frame_pacing.m" -o "$build_dir/frame-pacing.o"
xcrun --sdk "$sdk" clang -ffile-prefix-map="$HOME=~" -arch arm64 -isysroot "$SDKROOT" \
  -target "arm64-apple-$platform_flag" -mios-version-min=15.0 -O2 -std=c11 \
  -c "$project_root/ios/frame_pacing_policy.c" -o "$build_dir/frame-pacing-policy.o"
xcrun --sdk "$sdk" clang -ffile-prefix-map="$HOME=~" -arch arm64 -isysroot "$SDKROOT" \
  -target "arm64-apple-$platform_flag" -mios-version-min=15.0 -O2 -fobjc-arc \
  -c "$project_root/ios/auto_progress.m" -o "$build_dir/auto-progress.o"
xcrun --sdk "$sdk" clang -ffile-prefix-map="$HOME=~" -arch arm64 -isysroot "$SDKROOT" \
  -target "arm64-apple-$platform_flag" -mios-version-min=15.0 -O2 -fobjc-arc \
  -c "$project_root/ios/cloud_progress.m" -o "$build_dir/cloud-progress.o"
xcrun --sdk "$sdk" clang -ffile-prefix-map="$HOME=~" -arch arm64 -isysroot "$SDKROOT" \
  -target "arm64-apple-$platform_flag" -mios-version-min=15.0 -O2 -fobjc-arc \
  -c "$project_root/ios/cloud_capability.m" -o "$build_dir/cloud-capability.o"
xcrun --sdk "$sdk" clang -ffile-prefix-map="$HOME=~" -arch arm64 -isysroot "$SDKROOT" \
  -target "arm64-apple-$platform_flag" -mios-version-min=15.0 -O2 -fobjc-arc \
  -c "$project_root/ios/folder_sync.m" -o "$build_dir/folder-sync.o"
xcrun --sdk "$sdk" clang -ffile-prefix-map="$HOME=~" -arch arm64 -isysroot "$SDKROOT" \
  -target "arm64-apple-$platform_flag" -mios-version-min=15.0 \
  -I "$sdl_install/include/SDL2" \
  "$project_root/ios/main.m" \
  "$project_root/ios/wrapper_preferences.m" \
  "$project_root/ios/save_manager.m" \
  "$project_root/ios/nights_settings.m" \
  "$build_dir/cloud-progress.o" \
  "$build_dir/cloud-capability.o" \
  "$build_dir/folder-sync.o" \
  "$build_dir/auto-progress.o" \
  "$project_root/ios/import_preview.m" \
  "$project_root/ios/keyboard_bridge.m" \
  "$build_dir/frame-pacing.o" "$build_dir/frame-pacing-policy.o" \
  "$build_dir/fps-overlay.o" "$build_dir/fps-counter.o" \
  "$CARGO_TARGET_DIR/$target/release/libpeggle_compatibility.a" \
  -L "$sdl_install/lib" -lSDL2main -lSDL2 -lc++ -liconv -lz \
  -framework UIKit -framework Foundation -framework OpenGLES -framework OpenAL \
  -framework QuartzCore -framework AVFoundation -framework AudioToolbox \
  -framework CoreAudio -framework CoreGraphics -framework CoreMotion \
  -framework GameController -framework CoreHaptics -framework CoreBluetooth \
  -framework Metal -framework Security -framework CloudKit -framework UniformTypeIdentifiers \
  -Wl,-no_adhoc_codesign -o "$app/PeggleCompatibility"
cp "$project_root/ios/Info.plist" "$app/Info.plist"
cp -R "$project_root/ios/Settings.bundle" "$app/Settings.bundle"
# No clean-profile marker: original first-run name entry is enabled.
if [ "$sdk" = iphonesimulator ]; then
  plutil -replace CFBundleSupportedPlatforms -json '["iPhoneSimulator"]' "$app/Info.plist"
  plutil -replace RespringProgressCloudKit -bool NO "$app/Info.plist"
fi
python3 "$repository/Scripts/pack_original.py" "$app/Peggle.ipa"
cp "$project_root/ios/Icon.png" "$app/Icon.png"
cp "$project_root/ios/ManageSavesIcon.png" "$app/ManageSavesIcon.png"
cp -R "$project_root/compatibility-core/touchHLE_fonts" "$app/"
cp -R "$project_root/compatibility-core/touchHLE_dylibs" "$app/"
cp "$project_root/compatibility-core/touchHLE_default_options.txt" "$app/"
python3 "$project_root/ios/compile_icon.py" "$app" "$sdk"
cp "$project_root/compatibility-core/LICENSE" "$app/touchHLE-LICENSE.txt"
cp "$project_root/compatibility-core/native/file-zlib-113/NOTICE.txt" "$app/NativeFileZlib-NOTICE.txt"
python3 "$repository/Scripts/configure_info.py" "$app/Info.plist" "$sdk"
ditto --norsrc --noextattr "$repository/Notices" "$app/ThirdPartyNotices"
if [ "$sdk" = iphonesimulator ]; then
  codesign --force --sign - "$app"
fi
echo "Built $app"
