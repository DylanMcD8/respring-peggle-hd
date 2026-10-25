# Building Peggle HD

Use full Xcode selected by `xcode-select`, Python 3, Rust through rustup, CMake and Ninja on PATH. Targets are ARM64 iOS and Apple silicon iOS Simulator.

```sh
rustup toolchain install 1.99.0 --profile minimal
rustup target add --toolchain 1.99.0 aarch64-apple-ios aarch64-apple-ios-sim
```

Installing tools needs network access. Compilation uses `--offline --locked` and vendored dependencies. Debug and Release both use optimized Rust. No sibling project or private toolchain directory is required.

## Xcode

Open `PeggleHD.xcodeproj`, select **PeggleHD**, and choose your destination. For a device, select your team and bundle identifier. Xcode processes `Info.xcode.plist`; the shell phase excludes the final root Info.plist to avoid competing outputs. The Icon Composer source at the repository root is editable.

```sh
xcodebuild -project PeggleHD.xcodeproj -scheme PeggleHD -configuration Release \
  -destination 'generic/platform=iOS Simulator' -derivedDataPath /tmp/PeggleHD-sim CODE_SIGNING_ALLOWED=NO
xcodebuild -project PeggleHD.xcodeproj -scheme PeggleHD -configuration Release \
  -destination 'generic/platform=iOS' -derivedDataPath /tmp/PeggleHD-device CODE_SIGNING_ALLOWED=NO
```

Leave signing enabled and select your team for physical-device Run.

## Direct build and unsigned IPA

```sh
PEGGLE_NATIVE_BUILD=/tmp/PeggleHD-build bash Compatibility/ios/build.sh iphoneos
python3 Scripts/package_ipa.py /tmp/PeggleHD-build/iphoneos/PeggleHD.app --output release-output/Peggle-HD.ipa
python3 Scripts/audit_release.py release-output/Peggle-HD.ipa
```

Use `iphonesimulator` for a simulator app. Build outside File Provider/iCloud-managed output directories, using Xcode DerivedData or `/tmp`. Do not share an output directory between Peggle and Peggle HD.

`Original/Payload` is the sanitized original game input. The build verifies its hashes and reconstructs a nested IPA. The ARM64 host reads the ARMv7 guest executable as data. The package script stages a copy and removes signing/provisioning/account metadata without changing the input app.

Default builds support offline gameplay and manual save-file management. See [iCloud configuration](docs/ICLOUD.md) for developer provisioning.

## Tests

```sh
cargo test --manifest-path Tests/SaveManager/Cargo.toml --offline --locked
```

Tests cover validation, import/export, rollback, version-25 adapters, independent Classic/Nights challenge completions, Adventure checkpoints and merging. Fixtures use anonymous synthetic values. Rendering/performance still require hardware testing.

## Layout

- `Compatibility/ios`: UIKit host, CloudKit transport, Rust entry and save-file adapters.
- `Compatibility/compatibility-core`: modified touchHLE and guarded native helpers.
- `Original`: sanitized game payload and hashes.
- `Vendor`: Cargo and modified SDL2 sources.
- `PeggleSourceTest.icon`: editable app icon.
- `Notices`: dependency notices included in the app.

Historical symbols may retain CHUZZLE or candidate-number prefixes. No expanded AOT experiment is enabled.
