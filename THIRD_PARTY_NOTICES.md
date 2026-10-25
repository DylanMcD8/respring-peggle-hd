# Third-party sources and notices

- **touchHLE** — https://github.com/touchHLE/touchHLE — MPL-2.0. Modified source is in `Compatibility/compatibility-core`: ARM/Thumb interpretation, guarded native helpers, legacy iOS APIs, rendering, input and lifecycle compatibility.
- **ARM interpreter** — original notices remain in `Compatibility/compatibility-core/src/cpu/interpreter/LICENSE`.
- **SDL2** — https://github.com/libsdl-org/SDL — zlib license. `Vendor/SDL2` includes the modified UIKit scene/display and presentation implementation.
- **Rust dependencies** — exact versions/checksums are in Cargo.lock. Sources and licenses are in `Vendor/Rust`; metadata is collected in `Notices/rust-dependencies.txt`.
- **zlib 1.1.3** — native helper source and notices are in `Compatibility/compatibility-core/native/file-zlib-113`.
- **stb_image and PVRTDecompress** — original notices remain beside their sources under `Compatibility/compatibility-core/vendor`.
- **Fonts** — license texts accompany files in `Compatibility/compatibility-core/touchHLE_fonts`.
- **Legacy libraries** — provenance, aliases and notices are in `Compatibility/compatibility-core/touchHLE_dylibs/README.md` and its `COPYING.*` files. The release's `Peggle-HD-runtime-sources.zip` includes Apple gcc-5664 and libstdcxx-39 upstream sources/build files. These historic libraries have not been rebuilt with modern Xcode.

Built apps include `ThirdPartyNotices`. The matching tagged source archive supplies modified MPL-covered source and dependencies. Peggle content remains the property of its respective rights holders, including PopCap and EA; this project claims no affiliation or endorsement.
