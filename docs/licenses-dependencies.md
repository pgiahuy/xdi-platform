# Giấy phép các phụ thuộc

File này sinh bằng `make licenses` (scripts/licenses.mjs), không sửa tay. Đánh giá và bảng tóm tắt ở
[LICENSES.md](LICENSES.md).

407 crate Rust (từ `backend/Cargo.lock`, gồm cả crate chỉ dùng trên nền tảng khác Linux), 550 gói npm
(từ `frontend/package-lock.json`). Giấy phép không thuộc nhóm cho phép thông dụng: 28.

## Không thuộc nhóm cho phép thông dụng

| Gói | Phiên bản | Giấy phép | Chỉ lúc build |
| --- | --- | --- | --- |
| @fontsource-variable/jetbrains-mono | 5.3.0 | OFL-1.1 |  |
| @fontsource/be-vietnam-pro | 5.3.0 | OFL-1.1 |  |
| lightningcss | 1.32.0 | MPL-2.0 | có |
| lightningcss | 1.33.0 | MPL-2.0 | có |
| lightningcss-android-arm64 | 1.32.0 | MPL-2.0 | có |
| lightningcss-android-arm64 | 1.33.0 | MPL-2.0 | có |
| lightningcss-darwin-arm64 | 1.32.0 | MPL-2.0 | có |
| lightningcss-darwin-arm64 | 1.33.0 | MPL-2.0 | có |
| lightningcss-darwin-x64 | 1.32.0 | MPL-2.0 | có |
| lightningcss-darwin-x64 | 1.33.0 | MPL-2.0 | có |
| lightningcss-freebsd-x64 | 1.32.0 | MPL-2.0 | có |
| lightningcss-freebsd-x64 | 1.33.0 | MPL-2.0 | có |
| lightningcss-linux-arm-gnueabihf | 1.32.0 | MPL-2.0 | có |
| lightningcss-linux-arm-gnueabihf | 1.33.0 | MPL-2.0 | có |
| lightningcss-linux-arm64-gnu | 1.32.0 | MPL-2.0 | có |
| lightningcss-linux-arm64-gnu | 1.33.0 | MPL-2.0 | có |
| lightningcss-linux-arm64-musl | 1.32.0 | MPL-2.0 | có |
| lightningcss-linux-arm64-musl | 1.33.0 | MPL-2.0 | có |
| lightningcss-linux-x64-gnu | 1.32.0 | MPL-2.0 | có |
| lightningcss-linux-x64-gnu | 1.33.0 | MPL-2.0 | có |
| lightningcss-linux-x64-musl | 1.32.0 | MPL-2.0 | có |
| lightningcss-linux-x64-musl | 1.33.0 | MPL-2.0 | có |
| lightningcss-win32-arm64-msvc | 1.32.0 | MPL-2.0 | có |
| lightningcss-win32-arm64-msvc | 1.33.0 | MPL-2.0 | có |
| lightningcss-win32-x64-msvc | 1.32.0 | MPL-2.0 | có |
| lightningcss-win32-x64-msvc | 1.33.0 | MPL-2.0 | có |
| webpki-root-certs | 1.0.10 | CDLA-Permissive-2.0 |  |
| webpki-roots | 1.0.10 | CDLA-Permissive-2.0 |  |

## Rust

| Crate | Phiên bản | Giấy phép |
| --- | --- | --- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| aead | 0.5.2 | MIT OR Apache-2.0 |
| aes | 0.8.4 | MIT OR Apache-2.0 |
| aes-gcm | 0.10.3 | Apache-2.0 OR MIT |
| ahash | 0.8.12 | MIT OR Apache-2.0 |
| aho-corasick | 1.1.5 | Unlicense OR MIT |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 |
| android_system_properties | 0.1.6 | MIT OR Apache-2.0 |
| ar_archive_writer | 0.5.3 | Apache-2.0 WITH LLVM-exception |
| argon2 | 0.6.0 | MIT OR Apache-2.0 |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 |
| arrow | 58.4.0 | Apache-2.0 |
| arrow-arith | 58.4.0 | Apache-2.0 |
| arrow-array | 58.4.0 | Apache-2.0 AND MIT |
| arrow-buffer | 58.4.0 | Apache-2.0 |
| arrow-cast | 58.4.0 | Apache-2.0 |
| arrow-data | 58.4.0 | Apache-2.0 |
| arrow-ord | 58.4.0 | Apache-2.0 |
| arrow-row | 58.4.0 | Apache-2.0 |
| arrow-schema | 58.4.0 | Apache-2.0 |
| arrow-select | 58.4.0 | Apache-2.0 |
| arrow-string | 58.4.0 | Apache-2.0 |
| async-trait | 0.1.92 | MIT OR Apache-2.0 |
| atoi | 2.0.0 | MIT |
| atoi_simd | 0.18.1 | MIT OR Apache-2.0 |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT |
| autocfg | 1.5.1 | Apache-2.0 OR MIT |
| axum | 0.8.9 | MIT |
| axum-core | 0.5.6 | MIT |
| axum-extra | 0.12.6 | MIT |
| base64 | 0.22.1 | MIT OR Apache-2.0 |
| base64 | 0.23.1 | MIT OR Apache-2.0 |
| base64ct | 1.8.3 | Apache-2.0 OR MIT |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| blake2 | 0.11.0 | MIT OR Apache-2.0 |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 |
| borsh | 1.8.1 | MIT OR Apache-2.0 |
| borsh-derive | 1.8.1 | Apache-2.0 |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 |
| byteorder | 1.5.0 | Unlicense OR MIT |
| bytes | 1.12.1 | MIT |
| calamine | 0.37.0 | MIT |
| cast | 0.3.0 | MIT OR Apache-2.0 |
| cc | 1.9.0 | MIT OR Apache-2.0 |
| cfg_aliases | 0.2.2 | MIT |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 |
| chrono | 0.4.45 | MIT OR Apache-2.0 |
| cipher | 0.4.4 | MIT OR Apache-2.0 |
| cmov | 0.5.4 | Apache-2.0 OR MIT |
| codepage | 0.1.3 | Apache-2.0 OR MIT |
| combine | 4.6.9 | MIT |
| comfy-table | 7.1.4 | MIT |
| const-oid | 0.9.6 | Apache-2.0 OR MIT |
| const-random | 0.1.18 | MIT OR Apache-2.0 |
| const-random-macro | 0.1.16 | MIT OR Apache-2.0 |
| cookie | 0.18.2 | MIT OR Apache-2.0 |
| core_detect | 1.0.0 | MIT OR Apache-2.0 |
| core-foundation | 0.10.1 | MIT OR Apache-2.0 |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 |
| crc | 3.4.0 | MIT OR Apache-2.0 |
| crc-catalog | 2.5.0 | MIT OR Apache-2.0 |
| crc32fast | 1.5.3 | MIT OR Apache-2.0 |
| crossbeam-queue | 0.3.14 | MIT OR Apache-2.0 |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 |
| crossterm | 0.28.1 | MIT |
| crossterm_winapi | 0.9.1 | MIT |
| crunchy | 0.2.4 | MIT |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 |
| ctr | 0.9.2 | MIT OR Apache-2.0 |
| ctutils | 0.4.3 | Apache-2.0 OR MIT |
| debug_unsafe | 0.1.4 | MIT OR Apache-2.0 |
| der | 0.7.10 | Apache-2.0 OR MIT |
| deranged | 0.5.9 | MIT OR Apache-2.0 |
| deunicode | 1.6.2 | BSD-3-Clause |
| digest | 0.10.7 | MIT OR Apache-2.0 |
| digest | 0.11.3 | MIT OR Apache-2.0 |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 |
| dotenvy | 0.15.7 | MIT |
| duckdb | 1.10506.1 | MIT |
| either | 1.19.0 | MIT OR Apache-2.0 |
| encoding_rs | 0.8.42 | (Apache-2.0 OR MIT) AND BSD-3-Clause |
| equivalent | 1.0.3 | Apache-2.0 OR MIT |
| errno | 0.3.14 | MIT OR Apache-2.0 |
| etcetera | 0.8.0 | MIT OR Apache-2.0 |
| event-listener | 5.4.2 | Apache-2.0 OR MIT |
| fallible-iterator | 0.3.0 | MIT OR Apache-2.0 |
| fallible-streaming-iterator | 0.1.9 | MIT OR Apache-2.0 |
| fast-float2 | 0.2.4 | MIT OR Apache-2.0 |
| filetime | 0.2.29 | MIT OR Apache-2.0 |
| find-msvc-tools | 0.1.15 | MIT OR Apache-2.0 |
| flate2 | 1.1.10 | MIT OR Apache-2.0 |
| flume | 0.11.1 | Apache-2.0 OR MIT |
| foldhash | 0.1.5 | Zlib |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 |
| futures-core | 0.3.34 | MIT OR Apache-2.0 |
| futures-executor | 0.3.34 | MIT OR Apache-2.0 |
| futures-intrusive | 0.5.0 | MIT OR Apache-2.0 |
| futures-io | 0.3.34 | MIT OR Apache-2.0 |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 |
| futures-task | 0.3.34 | MIT OR Apache-2.0 |
| futures-util | 0.3.34 | MIT OR Apache-2.0 |
| generic-array | 0.14.7 | MIT |
| getrandom | 0.2.17 | MIT OR Apache-2.0 |
| getrandom | 0.3.4 | MIT OR Apache-2.0 |
| getrandom | 0.4.3 | MIT OR Apache-2.0 |
| ghash | 0.5.1 | Apache-2.0 OR MIT |
| half | 2.7.1 | MIT OR Apache-2.0 |
| hashbrown | 0.15.5 | MIT OR Apache-2.0 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| hashlink | 0.10.0 | MIT OR Apache-2.0 |
| heck | 0.5.0 | MIT OR Apache-2.0 |
| hex | 0.4.3 | MIT OR Apache-2.0 |
| hkdf | 0.12.4 | MIT OR Apache-2.0 |
| hmac | 0.12.1 | MIT OR Apache-2.0 |
| home | 0.5.12 | MIT OR Apache-2.0 |
| http | 1.5.0 | MIT OR Apache-2.0 |
| http-body | 1.1.0 | MIT |
| http-body-util | 0.1.5 | MIT |
| http-range-header | 0.4.2 | MIT |
| httparse | 1.10.1 | MIT OR Apache-2.0 |
| httpdate | 1.0.3 | MIT OR Apache-2.0 |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 |
| hyper | 1.12.0 | MIT |
| hyper-rustls | 0.27.10 | Apache-2.0 OR ISC OR MIT |
| hyper-util | 0.1.21 | MIT |
| iana-time-zone | 0.1.65 | MIT OR Apache-2.0 |
| iana-time-zone-haiku | 0.1.2 | MIT OR Apache-2.0 |
| icu_collections | 2.3.0 | Unicode-3.0 |
| icu_locale_core | 2.3.0 | Unicode-3.0 |
| icu_normalizer | 2.3.0 | Unicode-3.0 |
| icu_normalizer_data | 2.3.0 | Unicode-3.0 |
| icu_properties | 2.3.0 | Unicode-3.0 |
| icu_properties_data | 2.3.0 | Unicode-3.0 |
| icu_provider | 2.3.1 | Unicode-3.0 |
| idna | 1.1.0 | MIT OR Apache-2.0 |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT |
| indexmap | 2.14.2 | Apache-2.0 OR MIT |
| inout | 0.1.4 | MIT OR Apache-2.0 |
| ipnet | 2.12.2 | MIT OR Apache-2.0 |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| jni | 0.22.4 | MIT OR Apache-2.0 |
| jni-macros | 0.22.4 | MIT OR Apache-2.0 |
| jni-sys | 0.4.1 | MIT OR Apache-2.0 |
| jni-sys-macros | 0.4.1 | MIT OR Apache-2.0 |
| jobserver | 0.1.35 | MIT OR Apache-2.0 |
| js-sys | 0.3.106 | MIT OR Apache-2.0 |
| lazy_static | 1.5.1 | MIT OR Apache-2.0 |
| lexical-core | 1.0.6 | MIT OR Apache-2.0 |
| lexical-parse-float | 1.0.6 | MIT OR Apache-2.0 |
| lexical-parse-integer | 1.0.6 | MIT OR Apache-2.0 |
| lexical-util | 1.0.7 | MIT OR Apache-2.0 |
| lexical-write-float | 1.0.6 | MIT OR Apache-2.0 |
| lexical-write-integer | 1.0.6 | MIT OR Apache-2.0 |
| libc | 0.2.191 | MIT OR Apache-2.0 |
| libduckdb-sys | 1.10506.1 | MIT |
| libm | 0.2.16 | MIT |
| libredox | 0.1.25 | MIT |
| libsqlite3-sys | 0.30.1 | MIT |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| linux-raw-sys | 0.4.15 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| litemap | 0.8.3 | Unicode-3.0 |
| lock_api | 0.4.14 | MIT OR Apache-2.0 |
| log | 0.4.34 | MIT OR Apache-2.0 |
| matchers | 0.2.0 | MIT |
| matchit | 0.8.4 | MIT AND BSD-3-Clause |
| md-5 | 0.10.6 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| mime | 0.3.17 | MIT OR Apache-2.0 |
| mime_guess | 2.0.5 | MIT |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| mio | 1.2.4 | MIT |
| multer | 3.1.0 | MIT |
| multiversion_no_op | 1.0.0 | Apache-2.0 OR MIT |
| nix | 0.31.3 | MIT |
| nu-ansi-term | 0.50.3 | MIT |
| num-bigint | 0.4.8 | MIT OR Apache-2.0 |
| num-bigint-dig | 0.8.6 | MIT OR Apache-2.0 |
| num-complex | 0.4.6 | MIT OR Apache-2.0 |
| num-conv | 0.2.2 | MIT OR Apache-2.0 |
| num-integer | 0.1.47 | MIT OR Apache-2.0 |
| num-iter | 0.1.46 | MIT OR Apache-2.0 |
| num-traits | 0.2.19 | MIT OR Apache-2.0 |
| object | 0.39.1 | Apache-2.0 OR MIT |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| opaque-debug | 0.3.1 | MIT OR Apache-2.0 |
| openssl-probe | 0.2.1 | MIT OR Apache-2.0 |
| parking | 2.2.1 | Apache-2.0 OR MIT |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 |
| password-hash | 0.6.1 | MIT OR Apache-2.0 |
| pastey | 0.2.3 | MIT OR Apache-2.0 |
| pem-rfc7468 | 0.7.0 | Apache-2.0 OR MIT |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 |
| phc | 0.6.1 | Apache-2.0 OR MIT |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| pkcs1 | 0.7.5 | Apache-2.0 OR MIT |
| pkcs8 | 0.10.2 | Apache-2.0 OR MIT |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 |
| plain | 0.2.3 | MIT OR Apache-2.0 |
| polyval | 0.6.2 | Apache-2.0 OR MIT |
| potential_utf | 0.1.6 | Unicode-3.0 |
| powerfmt | 0.2.1 | MIT OR Apache-2.0 |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| psm | 0.1.32 | MIT OR Apache-2.0 |
| quick-xml | 0.42.0 | MIT |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| r-efi | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| rand | 0.8.8 | MIT OR Apache-2.0 |
| rand | 0.9.5 | MIT OR Apache-2.0 |
| rand_chacha | 0.3.1 | MIT OR Apache-2.0 |
| rand_chacha | 0.9.0 | MIT OR Apache-2.0 |
| rand_core | 0.6.4 | MIT OR Apache-2.0 |
| rand_core | 0.9.5 | MIT OR Apache-2.0 |
| recursive | 0.1.1 | MIT |
| recursive-proc-macro-impl | 0.1.1 | MIT |
| redox_syscall | 0.5.18 | MIT |
| redox_syscall | 0.9.5 | MIT |
| regex | 1.13.1 | MIT OR Apache-2.0 |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 |
| reqwest | 0.13.5 | MIT OR Apache-2.0 |
| ring | 0.17.14 | Apache-2.0 AND ISC |
| rlimit | 0.11.0 | MIT |
| rsa | 0.9.10 | MIT OR Apache-2.0 |
| rust_decimal | 1.43.0 | MIT |
| rust_xlsxwriter | 0.101.0 | MIT OR Apache-2.0 |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 |
| rustix | 0.38.44 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| rustix | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT |
| rustls-native-certs | 0.8.5 | Apache-2.0 OR ISC OR MIT |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 |
| rustls-platform-verifier | 0.7.1 | MIT OR Apache-2.0 |
| rustls-platform-verifier-android | 0.2.0 | MIT OR Apache-2.0 |
| rustls-webpki | 0.103.16 | ISC |
| rustversion | 1.0.23 | MIT OR Apache-2.0 |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 |
| same-file | 1.0.6 | Unlicense OR MIT |
| schannel | 0.1.30 | MIT |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 |
| security-framework | 3.7.0 | MIT OR Apache-2.0 |
| security-framework-sys | 2.17.0 | MIT OR Apache-2.0 |
| semver | 1.0.28 | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | MIT OR Apache-2.0 |
| serde_path_to_error | 0.1.20 | MIT OR Apache-2.0 |
| serde_urlencoded | 0.7.1 | MIT OR Apache-2.0 |
| sha1 | 0.10.7 | MIT OR Apache-2.0 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 |
| sharded-slab | 0.1.7 | MIT |
| shlex | 2.0.1 | MIT OR Apache-2.0 |
| signal-hook-registry | 1.4.8 | MIT OR Apache-2.0 |
| signature | 2.2.0 | Apache-2.0 OR MIT |
| simd_cesu8 | 1.2.0 | Apache-2.0 OR MIT |
| simd-adler32 | 0.3.10 | MIT |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 |
| slab | 0.4.12 | MIT |
| smallvec | 1.16.3 | MIT OR Apache-2.0 |
| socket2 | 0.6.5 | MIT OR Apache-2.0 |
| spin | 0.9.9 | MIT |
| spki | 0.7.3 | Apache-2.0 OR MIT |
| sqlparser | 0.63.0 | Apache-2.0 |
| sqlparser_derive | 0.6.0 | Apache-2.0 |
| sqlx | 0.8.6 | MIT OR Apache-2.0 |
| sqlx-core | 0.8.6 | MIT OR Apache-2.0 |
| sqlx-macros | 0.8.6 | MIT OR Apache-2.0 |
| sqlx-macros-core | 0.8.6 | MIT OR Apache-2.0 |
| sqlx-mysql | 0.8.6 | MIT OR Apache-2.0 |
| sqlx-postgres | 0.8.6 | MIT OR Apache-2.0 |
| sqlx-sqlite | 0.8.6 | MIT OR Apache-2.0 |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| stacker | 0.1.25 | MIT OR Apache-2.0 |
| stringprep | 0.1.5 | MIT OR Apache-2.0 |
| strum | 0.27.2 | MIT |
| strum_macros | 0.27.2 | MIT |
| subtle | 2.6.1 | BSD-3-Clause |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| syn | 3.0.9 | MIT OR Apache-2.0 |
| sync_wrapper | 1.0.2 | Apache-2.0 |
| synstructure | 0.14.0 | MIT |
| tar | 0.4.46 | MIT OR Apache-2.0 |
| thiserror | 2.0.21 | MIT OR Apache-2.0 |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 |
| thread_local | 1.1.10 | MIT OR Apache-2.0 |
| time | 0.3.55 | MIT OR Apache-2.0 |
| time-core | 0.1.9 | MIT OR Apache-2.0 |
| time-macros | 0.2.32 | MIT OR Apache-2.0 |
| tiny-keccak | 2.0.2 | CC0-1.0 |
| tinystr | 0.8.4 | Unicode-3.0 |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT |
| tokio | 1.54.1 | MIT |
| tokio-macros | 2.7.2 | MIT |
| tokio-rustls | 0.26.6 | MIT OR Apache-2.0 |
| tokio-stream | 0.1.19 | MIT |
| tokio-util | 0.7.20 | MIT |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_edit | 0.25.17+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 |
| tower | 0.5.3 | MIT |
| tower-http | 0.6.11 | MIT |
| tower-http | 0.7.1 | MIT |
| tower-layer | 0.3.3 | MIT |
| tower-service | 0.3.3 | MIT |
| tracing | 0.1.44 | MIT |
| tracing-attributes | 0.1.31 | MIT |
| tracing-core | 0.1.36 | MIT |
| tracing-log | 0.2.0 | MIT |
| tracing-subscriber | 0.3.23 | MIT |
| try-lock | 0.2.5 | MIT |
| typed-path | 0.12.4 | MIT OR Apache-2.0 |
| typenum | 1.20.1 | MIT OR Apache-2.0 |
| unicase | 2.10.0 | MIT OR Apache-2.0 |
| unicode-bidi | 0.3.18 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| unicode-normalization | 0.1.26 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| unicode-properties | 0.1.4 | MIT OR Apache-2.0 |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 |
| unicode-width | 0.2.2 | MIT OR Apache-2.0 |
| universal-hash | 0.5.1 | MIT OR Apache-2.0 |
| untrusted | 0.9.0 | ISC |
| ureq | 3.4.2 | MIT OR Apache-2.0 |
| ureq-proto | 0.6.5 | MIT OR Apache-2.0 |
| url | 2.5.8 | MIT OR Apache-2.0 |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT |
| utf8-zero | 0.8.1 | MIT OR Apache-2.0 |
| utoipa | 6.0.0 | MIT OR Apache-2.0 |
| utoipa-axum | 0.3.0 | MIT OR Apache-2.0 |
| utoipa-gen | 6.0.1 | MIT OR Apache-2.0 |
| utoipa-swagger-ui | 10.0.2 | MIT OR Apache-2.0 |
| utoipa-swagger-ui-vendored | 0.2.0 | MIT OR Apache-2.0 |
| uuid | 1.28.0 | Apache-2.0 OR MIT |
| valuable | 0.1.1 | MIT |
| vcpkg | 0.2.15 | MIT OR Apache-2.0 |
| version_check | 0.9.5 | MIT OR Apache-2.0 |
| walkdir | 2.5.0 | Unlicense OR MIT |
| want | 0.3.2 | MIT |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasip2 | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasite | 0.1.0 | Apache-2.0 OR BSL-1.0 OR MIT |
| wasm-bindgen | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-futures | 0.4.79 | MIT OR Apache-2.0 |
| wasm-bindgen-macro | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-macro-support | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-shared | 0.2.129 | MIT OR Apache-2.0 |
| web-sys | 0.3.106 | MIT OR Apache-2.0 |
| webpki-root-certs | 1.0.10 | CDLA-Permissive-2.0 |
| webpki-roots | 1.0.10 | CDLA-Permissive-2.0 |
| whoami | 1.6.1 | Apache-2.0 OR BSL-1.0 OR MIT |
| winapi | 0.3.9 | MIT OR Apache-2.0 |
| winapi-i686-pc-windows-gnu | 0.4.0 | MIT OR Apache-2.0 |
| winapi-util | 0.1.11 | Unlicense OR MIT |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | MIT OR Apache-2.0 |
| windows_aarch64_gnullvm | 0.48.5 | MIT OR Apache-2.0 |
| windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_aarch64_msvc | 0.48.5 | MIT OR Apache-2.0 |
| windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnu | 0.48.5 | MIT OR Apache-2.0 |
| windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_msvc | 0.48.5 | MIT OR Apache-2.0 |
| windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnu | 0.48.5 | MIT OR Apache-2.0 |
| windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnullvm | 0.48.5 | MIT OR Apache-2.0 |
| windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.48.5 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows-core | 0.62.2 | MIT OR Apache-2.0 |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-result | 0.4.1 | MIT OR Apache-2.0 |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 |
| windows-sys | 0.48.0 | MIT OR Apache-2.0 |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| windows-targets | 0.48.5 | MIT OR Apache-2.0 |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 |
| winnow | 1.0.4 | MIT |
| wit-bindgen | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| writeable | 0.6.4 | Unicode-3.0 |
| xattr | 1.6.1 | MIT OR Apache-2.0 |
| yoke | 0.8.3 | Unicode-3.0 |
| yoke-derive | 0.8.4 | Unicode-3.0 |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerofrom | 0.1.8 | Unicode-3.0 |
| zerofrom-derive | 0.1.8 | Unicode-3.0 |
| zeroize | 1.9.1 | Apache-2.0 OR MIT |
| zerotrie | 0.2.5 | Unicode-3.0 |
| zerovec | 0.11.8 | Unicode-3.0 |
| zerovec-derive | 0.11.6 | Unicode-3.0 |
| zip | 9.0.5 | MIT |
| zlib-rs | 0.6.8 | Zlib |
| zmij | 1.0.23 | MIT |
| zopfli | 0.8.4 | Apache-2.0 |

## npm

| Gói | Phiên bản | Giấy phép | Chỉ lúc build |
| --- | --- | --- | --- |
| @babel/code-frame | 7.29.7 | MIT | có |
| @babel/compat-data | 7.29.7 | MIT | có |
| @babel/core | 7.29.7 | MIT | có |
| @babel/generator | 7.29.8 | MIT | có |
| @babel/helper-annotate-as-pure | 7.29.7 | MIT | có |
| @babel/helper-compilation-targets | 7.29.7 | MIT | có |
| @babel/helper-create-class-features-plugin | 7.29.7 | MIT | có |
| @babel/helper-globals | 7.29.7 | MIT | có |
| @babel/helper-member-expression-to-functions | 7.29.7 | MIT | có |
| @babel/helper-module-imports | 7.29.7 | MIT | có |
| @babel/helper-module-transforms | 7.29.7 | MIT | có |
| @babel/helper-optimise-call-expression | 7.29.7 | MIT | có |
| @babel/helper-plugin-utils | 7.29.7 | MIT | có |
| @babel/helper-replace-supers | 7.29.7 | MIT | có |
| @babel/helper-skip-transparent-expression-wrappers | 7.29.7 | MIT | có |
| @babel/helper-string-parser | 7.29.7 | MIT | có |
| @babel/helper-validator-identifier | 7.29.7 | MIT | có |
| @babel/helper-validator-option | 7.29.7 | MIT | có |
| @babel/helpers | 7.29.10 | MIT | có |
| @babel/parser | 7.29.9 | MIT | có |
| @babel/plugin-syntax-jsx | 7.29.7 | MIT | có |
| @babel/plugin-syntax-typescript | 7.29.7 | MIT | có |
| @babel/plugin-transform-modules-commonjs | 7.29.7 | MIT | có |
| @babel/plugin-transform-typescript | 7.29.9 | MIT | có |
| @babel/preset-typescript | 7.29.7 | MIT | có |
| @babel/template | 7.29.7 | MIT | có |
| @babel/traverse | 7.29.10 | MIT | có |
| @babel/types | 7.29.8 | MIT | có |
| @dotenvx/dotenvx | 1.75.1 | BSD-3-Clause | có |
| @dotenvx/primitives | 0.8.0 | BSD-3-Clause | có |
| @emnapi/core | 1.11.1 | MIT | có |
| @emnapi/runtime | 1.11.1 | MIT | có |
| @emnapi/wasi-threads | 1.2.2 | MIT | có |
| @floating-ui/core | 1.8.0 | MIT |  |
| @floating-ui/dom | 1.8.0 | MIT |  |
| @floating-ui/react-dom | 2.1.9 | MIT |  |
| @floating-ui/utils | 0.2.12 | MIT |  |
| @fontsource-variable/jetbrains-mono | 5.3.0 | OFL-1.1 |  |
| @fontsource/be-vietnam-pro | 5.3.0 | OFL-1.1 |  |
| @hono/node-server | 2.1.5 | MIT | có |
| @jridgewell/gen-mapping | 0.3.13 | MIT | có |
| @jridgewell/remapping | 2.3.5 | MIT | có |
| @jridgewell/resolve-uri | 3.1.2 | MIT | có |
| @jridgewell/sourcemap-codec | 1.6.0 | MIT | có |
| @jridgewell/trace-mapping | 0.3.31 | MIT | có |
| @modelcontextprotocol/sdk | 1.32.1 | MIT | có |
| @napi-rs/wasm-runtime | 1.1.4 | MIT | có |
| @nodelib/fs.scandir | 2.1.5 | MIT | có |
| @nodelib/fs.stat | 2.0.5 | MIT | có |
| @nodelib/fs.walk | 1.2.8 | MIT | có |
| @oxc-project/types | 0.154.0 | MIT | có |
| @oxlint/binding-android-arm-eabi | 1.89.0 | MIT | có |
| @oxlint/binding-android-arm64 | 1.89.0 | MIT | có |
| @oxlint/binding-darwin-arm64 | 1.89.0 | MIT | có |
| @oxlint/binding-darwin-x64 | 1.89.0 | MIT | có |
| @oxlint/binding-freebsd-x64 | 1.89.0 | MIT | có |
| @oxlint/binding-linux-arm-gnueabihf | 1.89.0 | MIT | có |
| @oxlint/binding-linux-arm-musleabihf | 1.89.0 | MIT | có |
| @oxlint/binding-linux-arm64-gnu | 1.89.0 | MIT | có |
| @oxlint/binding-linux-arm64-musl | 1.89.0 | MIT | có |
| @oxlint/binding-linux-ppc64-gnu | 1.89.0 | MIT | có |
| @oxlint/binding-linux-riscv64-gnu | 1.89.0 | MIT | có |
| @oxlint/binding-linux-riscv64-musl | 1.89.0 | MIT | có |
| @oxlint/binding-linux-s390x-gnu | 1.89.0 | MIT | có |
| @oxlint/binding-linux-x64-gnu | 1.89.0 | MIT | có |
| @oxlint/binding-linux-x64-musl | 1.89.0 | MIT | có |
| @oxlint/binding-openharmony-arm64 | 1.89.0 | MIT | có |
| @oxlint/binding-win32-arm64-msvc | 1.89.0 | MIT | có |
| @oxlint/binding-win32-ia32-msvc | 1.89.0 | MIT | có |
| @oxlint/binding-win32-x64-msvc | 1.89.0 | MIT | có |
| @radix-ui/number | 1.1.3 | MIT |  |
| @radix-ui/primitive | 1.2.0 | MIT |  |
| @radix-ui/react-accessible-icon | 1.2.0 | MIT |  |
| @radix-ui/react-accordion | 1.3.1 | MIT |  |
| @radix-ui/react-alert-dialog | 1.2.1 | MIT |  |
| @radix-ui/react-arrow | 1.1.17 | MIT |  |
| @radix-ui/react-aspect-ratio | 1.2.0 | MIT |  |
| @radix-ui/react-avatar | 1.3.1 | MIT |  |
| @radix-ui/react-checkbox | 1.4.1 | MIT |  |
| @radix-ui/react-collapsible | 1.2.1 | MIT |  |
| @radix-ui/react-collection | 1.2.0 | MIT |  |
| @radix-ui/react-compose-refs | 1.1.5 | MIT |  |
| @radix-ui/react-context | 1.3.0 | MIT |  |
| @radix-ui/react-context-menu | 2.4.1 | MIT |  |
| @radix-ui/react-dialog | 1.3.1 | MIT |  |
| @radix-ui/react-direction | 1.2.0 | MIT |  |
| @radix-ui/react-dismissable-layer | 1.2.1 | MIT |  |
| @radix-ui/react-dropdown-menu | 2.2.1 | MIT |  |
| @radix-ui/react-focus-guards | 1.2.0 | MIT |  |
| @radix-ui/react-focus-scope | 1.3.1 | MIT |  |
| @radix-ui/react-form | 0.3.0 | MIT |  |
| @radix-ui/react-hover-card | 1.2.1 | MIT |  |
| @radix-ui/react-id | 1.1.4 | MIT |  |
| @radix-ui/react-label | 2.1.17 | MIT |  |
| @radix-ui/react-menu | 2.2.1 | MIT |  |
| @radix-ui/react-menubar | 1.2.1 | MIT |  |
| @radix-ui/react-navigation-menu | 1.4.1 | MIT |  |
| @radix-ui/react-one-time-password-field | 0.2.1 | MIT |  |
| @radix-ui/react-password-toggle-field | 0.2.1 | MIT |  |
| @radix-ui/react-popover | 1.3.1 | MIT |  |
| @radix-ui/react-popper | 1.4.0 | MIT |  |
| @radix-ui/react-portal | 1.2.0 | MIT |  |
| @radix-ui/react-presence | 1.1.11 | MIT |  |
| @radix-ui/react-primitive | 2.2.0 | MIT |  |
| @radix-ui/react-progress | 1.2.1 | MIT |  |
| @radix-ui/react-radio-group | 1.5.1 | MIT |  |
| @radix-ui/react-roving-focus | 1.2.1 | MIT |  |
| @radix-ui/react-scroll-area | 1.4.0 | MIT |  |
| @radix-ui/react-select | 2.4.1 | MIT |  |
| @radix-ui/react-separator | 1.2.0 | MIT |  |
| @radix-ui/react-slider | 1.6.1 | MIT |  |
| @radix-ui/react-slot | 1.5.0 | MIT |  |
| @radix-ui/react-switch | 1.4.1 | MIT |  |
| @radix-ui/react-tabs | 1.2.1 | MIT |  |
| @radix-ui/react-toast | 1.3.1 | MIT |  |
| @radix-ui/react-toggle | 1.2.1 | MIT |  |
| @radix-ui/react-toggle-group | 1.2.1 | MIT |  |
| @radix-ui/react-toolbar | 1.2.1 | MIT |  |
| @radix-ui/react-tooltip | 1.4.1 | MIT |  |
| @radix-ui/react-use-callback-ref | 1.1.5 | MIT |  |
| @radix-ui/react-use-controllable-state | 1.3.0 | MIT |  |
| @radix-ui/react-use-effect-event | 0.0.5 | MIT |  |
| @radix-ui/react-use-escape-keydown | 1.1.6 | MIT |  |
| @radix-ui/react-use-is-hydrated | 0.1.3 | MIT |  |
| @radix-ui/react-use-layout-effect | 1.1.4 | MIT |  |
| @radix-ui/react-use-previous | 1.1.4 | MIT |  |
| @radix-ui/react-use-rect | 1.1.4 | MIT |  |
| @radix-ui/react-use-size | 1.1.5 | MIT |  |
| @radix-ui/react-visually-hidden | 1.2.13 | MIT |  |
| @radix-ui/rect | 1.1.3 | MIT |  |
| @redocly/ajv | 8.11.2 | MIT | có |
| @redocly/config | 0.22.0 | MIT | có |
| @redocly/openapi-core | 1.34.21 | MIT | có |
| @reduxjs/toolkit | 2.13.0 | MIT |  |
| @remix-run/route-pattern | 0.22.1 | MIT |  |
| @rolldown/binding-android-arm-eabi | 1.2.14 | MIT | có |
| @rolldown/binding-android-arm64 | 1.2.14 | MIT | có |
| @rolldown/binding-darwin-arm64 | 1.2.14 | MIT | có |
| @rolldown/binding-darwin-x64 | 1.2.14 | MIT | có |
| @rolldown/binding-freebsd-x64 | 1.2.14 | MIT | có |
| @rolldown/binding-linux-arm-gnueabihf | 1.2.14 | MIT | có |
| @rolldown/binding-linux-arm64-gnu | 1.2.14 | MIT | có |
| @rolldown/binding-linux-arm64-musl | 1.2.14 | MIT | có |
| @rolldown/binding-linux-ppc64-gnu | 1.2.14 | MIT | có |
| @rolldown/binding-linux-s390x-gnu | 1.2.14 | MIT | có |
| @rolldown/binding-linux-x64-gnu | 1.2.14 | MIT | có |
| @rolldown/binding-linux-x64-musl | 1.2.14 | MIT | có |
| @rolldown/binding-openharmony-arm64 | 1.2.14 | MIT | có |
| @rolldown/binding-win32-arm64-msvc | 1.2.14 | MIT | có |
| @rolldown/binding-win32-x64-msvc | 1.2.14 | MIT | có |
| @rolldown/pluginutils | 1.0.1 | MIT | có |
| @sec-ant/readable-stream | 0.4.1 | MIT | có |
| @shadcn/registry | 0.2.0 | MIT | có |
| @sindresorhus/merge-streams | 4.0.0 | MIT | có |
| @standard-schema/spec | 1.1.0 | MIT |  |
| @standard-schema/utils | 0.3.0 | MIT |  |
| @tailwindcss/node | 4.3.3 | MIT | có |
| @tailwindcss/oxide | 4.3.3 | MIT | có |
| @tailwindcss/oxide-android-arm64 | 4.3.3 | MIT | có |
| @tailwindcss/oxide-darwin-arm64 | 4.3.3 | MIT | có |
| @tailwindcss/oxide-darwin-x64 | 4.3.3 | MIT | có |
| @tailwindcss/oxide-freebsd-x64 | 4.3.3 | MIT | có |
| @tailwindcss/oxide-linux-arm-gnueabihf | 4.3.3 | MIT | có |
| @tailwindcss/oxide-linux-arm64-gnu | 4.3.3 | MIT | có |
| @tailwindcss/oxide-linux-arm64-musl | 4.3.3 | MIT | có |
| @tailwindcss/oxide-linux-x64-gnu | 4.3.3 | MIT | có |
| @tailwindcss/oxide-linux-x64-musl | 4.3.3 | MIT | có |
| @tailwindcss/oxide-wasm32-wasi | 4.3.3 | MIT | có |
| @tailwindcss/oxide-win32-arm64-msvc | 4.3.3 | MIT | có |
| @tailwindcss/oxide-win32-x64-msvc | 4.3.3 | MIT | có |
| @tailwindcss/vite | 4.3.3 | MIT | có |
| @tanstack/query-core | 5.104.5 | MIT |  |
| @tanstack/react-query | 5.104.5 | MIT |  |
| @ts-morph/common | 0.27.0 | MIT | có |
| @tybys/wasm-util | 0.10.2 | MIT | có |
| @types/d3-array | 3.2.2 | MIT |  |
| @types/d3-color | 3.1.3 | MIT |  |
| @types/d3-ease | 3.0.2 | MIT |  |
| @types/d3-interpolate | 3.0.4 | MIT |  |
| @types/d3-path | 3.1.1 | MIT |  |
| @types/d3-scale | 4.0.9 | MIT |  |
| @types/d3-shape | 3.2.0 | MIT |  |
| @types/d3-time | 3.0.4 | MIT |  |
| @types/d3-timer | 3.0.2 | MIT |  |
| @types/node | 24.19.2 | MIT | có |
| @types/react | 19.3.0 | MIT |  |
| @types/react-dom | 19.3.0 | MIT |  |
| @types/use-sync-external-store | 0.0.6 | MIT |  |
| @types/validate-npm-package-name | 4.0.2 | MIT | có |
| @vitejs/plugin-react | 6.1.2 | MIT | có |
| accepts | 2.0.0 | MIT | có |
| agent-base | 7.1.4 | MIT | có |
| ajv | 8.20.0 | MIT | có |
| ajv-formats | 2.1.1 | MIT | có |
| ajv-formats | 3.0.1 | MIT | có |
| ansi-colors | 4.1.3 | MIT | có |
| ansi-regex | 5.0.1 | MIT | có |
| ansi-regex | 6.4.0 | MIT | có |
| ansi-regex | 6.4.0 | MIT | có |
| argparse | 2.0.1 | Python-2.0 | có |
| aria-hidden | 1.2.6 | MIT |  |
| ast-types | 0.16.3 | MIT | có |
| atomically | 1.7.0 | MIT | có |
| balanced-match | 1.0.2 | MIT | có |
| balanced-match | 4.0.4 | MIT | có |
| baseline-browser-mapping | 2.11.30 | Apache-2.0 | có |
| body-parser | 2.4.0 | MIT | có |
| brace-expansion | 2.1.7 | MIT | có |
| brace-expansion | 5.0.12 | MIT | có |
| braces | 3.0.3 | MIT | có |
| browserslist | 4.29.4 | MIT | có |
| bundle-name | 4.1.1 | MIT | có |
| bytes | 3.1.2 | MIT | có |
| call-bind-apply-helpers | 1.0.2 | MIT | có |
| call-bound | 1.0.4 | MIT | có |
| caniuse-lite | 1.0.30001817 | CC-BY-4.0 | có |
| chalk | 5.6.2 | MIT | có |
| change-case | 5.4.4 | MIT | có |
| class-variance-authority | 0.7.1 | Apache-2.0 |  |
| cli-cursor | 5.0.0 | MIT | có |
| cli-spinners | 2.9.2 | MIT | có |
| clsx | 2.1.1 | MIT |  |
| cn | 0.2.6 | MIT | có |
| cn | 0.2.6 | MIT | có |
| cn | 0.5.0 | MIT |  |
| code-block-writer | 13.0.3 | MIT | có |
| colorette | 1.4.0 | MIT | có |
| commander | 11.1.0 | MIT | có |
| commander | 14.0.3 | MIT | có |
| conf | 10.2.0 | MIT | có |
| content-disposition | 2.0.1 | MIT | có |
| content-type | 1.0.5 | MIT | có |
| content-type | 2.1.0 | MIT | có |
| content-type | 2.1.0 | MIT | có |
| content-type | 2.1.0 | MIT | có |
| content-type | 2.1.0 | MIT | có |
| convert-source-map | 2.0.0 | MIT | có |
| cookie | 0.7.2 | MIT | có |
| cookie-es | 3.1.1 | MIT |  |
| cookie-signature | 1.2.2 | MIT | có |
| cors | 2.8.6 | MIT | có |
| cross-spawn | 7.0.6 | MIT | có |
| cssesc | 3.0.0 | MIT | có |
| csstype | 3.2.3 | MIT |  |
| d3-array | 3.2.4 | ISC |  |
| d3-color | 3.1.0 | ISC |  |
| d3-ease | 3.0.1 | BSD-3-Clause |  |
| d3-format | 3.1.2 | ISC |  |
| d3-interpolate | 3.0.1 | ISC |  |
| d3-path | 3.1.0 | ISC |  |
| d3-scale | 4.0.2 | ISC |  |
| d3-shape | 3.2.0 | ISC |  |
| d3-time | 3.1.0 | ISC |  |
| d3-time-format | 4.1.0 | ISC |  |
| d3-timer | 3.0.1 | ISC |  |
| debounce-fn | 4.0.0 | MIT | có |
| debug | 4.4.3 | MIT | có |
| decimal.js-light | 2.5.1 | MIT |  |
| dedent | 1.7.2 | MIT | có |
| deepmerge | 4.3.1 | MIT | có |
| default-browser | 5.5.1 | MIT | có |
| default-browser-id | 5.0.1 | MIT | có |
| define-lazy-prop | 2.0.0 | MIT | có |
| define-lazy-prop | 3.0.0 | MIT | có |
| depd | 2.0.0 | MIT | có |
| detect-libc | 2.1.2 | Apache-2.0 | có |
| detect-node-es | 1.1.0 | MIT |  |
| diff | 8.0.4 | BSD-3-Clause | có |
| dot-prop | 6.0.1 | MIT | có |
| dotenv | 17.4.2 | BSD-2-Clause | có |
| dunder-proto | 1.0.1 | MIT | có |
| ee-first | 1.1.1 | MIT | có |
| electron-to-chromium | 1.5.462 | ISC | có |
| emoji-regex | 10.6.0 | MIT | có |
| encodeurl | 2.0.0 | MIT | có |
| enhanced-resolve | 5.26.0 | MIT | có |
| enquirer | 2.4.1 | MIT | có |
| env-paths | 2.2.1 | MIT | có |
| es-define-property | 1.0.1 | MIT | có |
| es-errors | 1.3.0 | MIT | có |
| es-object-atoms | 1.1.2 | MIT | có |
| es-toolkit | 1.52.0 | MIT |  |
| escalade | 3.2.0 | MIT | có |
| escape-html | 1.0.3 | MIT | có |
| esprima | 4.0.1 | BSD-2-Clause | có |
| etag | 1.8.1 | MIT | có |
| eventemitter3 | 5.0.4 | MIT |  |
| eventsource | 3.0.7 | MIT | có |
| eventsource-parser | 3.1.1 | MIT | có |
| execa | 5.1.1 | MIT | có |
| execa | 9.6.1 | MIT | có |
| express | 5.3.0 | MIT | có |
| express-rate-limit | 8.7.1 | MIT | có |
| fast-deep-equal | 3.1.3 | MIT | có |
| fast-glob | 3.3.3 | MIT | có |
| fast-uri | 3.1.9 | BSD-3-Clause | có |
| fastq | 1.20.3 | ISC | có |
| fdir | 6.5.0 | MIT | có |
| figures | 6.1.0 | MIT | có |
| fill-range | 7.1.1 | MIT | có |
| finalhandler | 2.1.1 | MIT | có |
| find-up | 3.0.0 | MIT | có |
| forwarded | 0.2.0 | MIT | có |
| fresh | 2.0.0 | MIT | có |
| fs-extra | 11.4.1 | MIT | có |
| fsevents | 2.3.3 | MIT | có |
| function-bind | 1.1.2 | MIT | có |
| fuzzysort | 3.1.0 | MIT | có |
| gensync | 1.0.0-beta.2 | MIT | có |
| get-east-asian-width | 1.7.0 | MIT | có |
| get-intrinsic | 1.3.0 | MIT | có |
| get-nonce | 1.0.1 | MIT |  |
| get-own-enumerable-keys | 1.0.0 | MIT | có |
| get-proto | 1.0.1 | MIT | có |
| get-stream | 6.0.1 | MIT | có |
| get-stream | 9.0.1 | MIT | có |
| glob-parent | 5.1.2 | ISC | có |
| gopd | 1.2.0 | MIT | có |
| graceful-fs | 4.2.11 | ISC | có |
| has-symbols | 1.1.0 | MIT | có |
| hasown | 2.0.4 | MIT | có |
| hono | 4.13.13 | MIT | có |
| http-errors | 2.0.1 | MIT | có |
| https-proxy-agent | 7.0.6 | MIT | có |
| human-signals | 2.1.0 | Apache-2.0 | có |
| human-signals | 8.0.1 | Apache-2.0 | có |
| iconv-lite | 0.7.3 | MIT | có |
| ignore | 5.3.2 | MIT | có |
| immer | 11.2.9 | MIT |  |
| index-to-position | 1.2.0 | MIT | có |
| inherits | 2.0.4 | ISC | có |
| internmap | 2.0.3 | ISC |  |
| ip-address | 10.7.4 | MIT | có |
| ipaddr.js | 1.9.1 | MIT | có |
| is-docker | 2.2.1 | MIT | có |
| is-docker | 3.0.0 | MIT | có |
| is-extglob | 2.1.1 | MIT | có |
| is-glob | 4.0.3 | MIT | có |
| is-in-ssh | 1.0.0 | MIT | có |
| is-inside-container | 1.0.0 | MIT | có |
| is-interactive | 2.0.0 | MIT | có |
| is-number | 7.0.0 | MIT | có |
| is-obj | 2.0.0 | MIT | có |
| is-obj | 3.0.0 | MIT | có |
| is-plain-obj | 4.1.0 | MIT | có |
| is-promise | 4.0.0 | MIT | có |
| is-regexp | 3.1.0 | MIT | có |
| is-stream | 2.0.1 | MIT | có |
| is-stream | 4.0.1 | MIT | có |
| is-unicode-supported | 1.3.0 | MIT | có |
| is-unicode-supported | 2.1.0 | MIT | có |
| is-wsl | 2.2.0 | MIT | có |
| is-wsl | 3.1.1 | MIT | có |
| isexe | 2.0.0 | ISC | có |
| isexe | 3.1.5 | BlueOak-1.0.0 | có |
| jiti | 2.7.0 | MIT | có |
| jose | 6.2.13 | MIT | có |
| js-levenshtein | 1.1.6 | MIT | có |
| js-tokens | 4.0.0 | MIT | có |
| js-yaml | 4.3.2 | MIT | có |
| jsesc | 3.1.0 | MIT | có |
| json-schema-traverse | 1.0.0 | MIT | có |
| json-schema-typed | 7.0.3 | BSD-2-Clause | có |
| json-schema-typed | 8.0.2 | BSD-2-Clause | có |
| json5 | 2.2.3 | MIT | có |
| jsonfile | 6.2.1 | MIT | có |
| kleur | 3.0.3 | MIT | có |
| kleur | 4.1.5 | MIT | có |
| lightningcss | 1.32.0 | MPL-2.0 | có |
| lightningcss | 1.33.0 | MPL-2.0 | có |
| lightningcss-android-arm64 | 1.32.0 | MPL-2.0 | có |
| lightningcss-android-arm64 | 1.33.0 | MPL-2.0 | có |
| lightningcss-darwin-arm64 | 1.32.0 | MPL-2.0 | có |
| lightningcss-darwin-arm64 | 1.33.0 | MPL-2.0 | có |
| lightningcss-darwin-x64 | 1.32.0 | MPL-2.0 | có |
| lightningcss-darwin-x64 | 1.33.0 | MPL-2.0 | có |
| lightningcss-freebsd-x64 | 1.32.0 | MPL-2.0 | có |
| lightningcss-freebsd-x64 | 1.33.0 | MPL-2.0 | có |
| lightningcss-linux-arm-gnueabihf | 1.32.0 | MPL-2.0 | có |
| lightningcss-linux-arm-gnueabihf | 1.33.0 | MPL-2.0 | có |
| lightningcss-linux-arm64-gnu | 1.32.0 | MPL-2.0 | có |
| lightningcss-linux-arm64-gnu | 1.33.0 | MPL-2.0 | có |
| lightningcss-linux-arm64-musl | 1.32.0 | MPL-2.0 | có |
| lightningcss-linux-arm64-musl | 1.33.0 | MPL-2.0 | có |
| lightningcss-linux-x64-gnu | 1.32.0 | MPL-2.0 | có |
| lightningcss-linux-x64-gnu | 1.33.0 | MPL-2.0 | có |
| lightningcss-linux-x64-musl | 1.32.0 | MPL-2.0 | có |
| lightningcss-linux-x64-musl | 1.33.0 | MPL-2.0 | có |
| lightningcss-win32-arm64-msvc | 1.32.0 | MPL-2.0 | có |
| lightningcss-win32-arm64-msvc | 1.33.0 | MPL-2.0 | có |
| lightningcss-win32-x64-msvc | 1.32.0 | MPL-2.0 | có |
| lightningcss-win32-x64-msvc | 1.33.0 | MPL-2.0 | có |
| locate-path | 3.0.0 | MIT | có |
| log-symbols | 6.0.0 | MIT | có |
| lru-cache | 5.1.1 | ISC | có |
| lucide-react | 1.60.0 | ISC |  |
| magic-string | 0.30.21 | MIT | có |
| math-intrinsics | 1.1.0 | MIT | có |
| media-typer | 1.1.1 | MIT | có |
| merge-descriptors | 2.0.0 | MIT | có |
| merge-stream | 2.0.0 | MIT | có |
| merge2 | 1.4.1 | MIT | có |
| micromatch | 4.0.8 | MIT | có |
| mime-db | 1.54.0 | MIT | có |
| mime-types | 3.0.2 | MIT | có |
| mimic-fn | 2.1.0 | MIT | có |
| mimic-fn | 3.1.0 | MIT | có |
| mimic-function | 5.0.1 | MIT | có |
| minimatch | 10.2.6 | BlueOak-1.0.0 | có |
| minimatch | 5.1.9 | ISC | có |
| minimist | 1.2.8 | MIT | có |
| ms | 2.1.3 | MIT | có |
| nanoid | 3.3.20 | MIT | có |
| negotiator | 1.1.0 | MIT | có |
| node-releases | 2.0.59 | MIT | có |
| npm-run-path | 4.0.1 | MIT | có |
| npm-run-path | 6.0.0 | MIT | có |
| object-assign | 4.1.1 | MIT | có |
| object-inspect | 1.13.4 | MIT | có |
| object-treeify | 1.1.33 | MIT | có |
| on-finished | 2.4.1 | MIT | có |
| once | 1.4.0 | ISC | có |
| onetime | 5.1.2 | MIT | có |
| onetime | 7.0.0 | MIT | có |
| open | 11.0.4 | MIT | có |
| open | 8.4.2 | MIT | có |
| openapi-typescript | 7.13.0 | MIT | có |
| ora | 8.2.0 | MIT | có |
| oxlint | 1.89.0 | MIT | có |
| p-limit | 2.3.0 | MIT | có |
| p-locate | 3.0.0 | MIT | có |
| p-try | 2.2.0 | MIT | có |
| parse-json | 8.3.0 | MIT | có |
| parse-ms | 4.0.0 | MIT | có |
| parseurl | 1.3.3 | MIT | có |
| path-browserify | 1.0.1 | MIT | có |
| path-exists | 3.0.0 | MIT | có |
| path-key | 3.1.1 | MIT | có |
| path-key | 4.0.0 | MIT | có |
| path-to-regexp | 8.4.2 | MIT | có |
| picocolors | 1.1.1 | ISC | có |
| picomatch | 2.3.2 | MIT | có |
| picomatch | 4.0.7 | MIT | có |
| pkce-challenge | 5.0.1 | MIT | có |
| pkg-up | 3.1.0 | MIT | có |
| pluralize | 8.0.0 | MIT | có |
| postcss | 8.5.31 | MIT | có |
| postcss-selector-parser | 7.1.7 | MIT | có |
| powershell-utils | 0.2.1 | MIT | có |
| pretty-ms | 9.3.1 | MIT | có |
| prompts | 2.4.2 | MIT | có |
| proxy-addr | 2.0.8 | MIT | có |
| qs | 6.16.1 | BSD-3-Clause | có |
| queue-microtask | 1.2.3 | MIT | có |
| radix-ui | 1.8.1 | MIT |  |
| range-parser | 1.3.0 | MIT | có |
| raw-body | 3.0.2 | MIT | có |
| react | 19.3.1 | MIT |  |
| react-dom | 19.3.1 | MIT |  |
| react-is | 19.3.1 | MIT |  |
| react-redux | 9.3.0 | MIT |  |
| react-remove-scroll | 2.7.2 | MIT |  |
| react-remove-scroll-bar | 2.3.8 | MIT |  |
| react-router | 8.5.0 | MIT |  |
| react-style-singleton | 2.2.3 | MIT |  |
| recast | 0.23.21 | MIT | có |
| recharts | 3.10.1 | MIT |  |
| redux | 5.0.1 | MIT |  |
| redux-thunk | 3.1.0 | MIT |  |
| require-from-string | 2.0.2 | MIT | có |
| reselect | 5.2.0 | MIT |  |
| restore-cursor | 5.1.0 | MIT | có |
| reusify | 1.1.0 | MIT | có |
| rolldown | 1.2.14 | MIT | có |
| router | 2.2.0 | MIT | có |
| run-applescript | 7.1.0 | MIT | có |
| run-parallel | 1.2.0 | MIT | có |
| safer-buffer | 2.1.2 | MIT | có |
| scheduler | 0.28.0 | MIT |  |
| semver | 6.3.1 | ISC | có |
| semver | 7.8.5 | ISC | có |
| send | 1.2.1 | MIT | có |
| serve-static | 2.2.1 | MIT | có |
| setprototypeof | 1.2.0 | ISC | có |
| shadcn | 4.22.1 | MIT | có |
| shebang-command | 2.0.0 | MIT | có |
| shebang-regex | 3.0.0 | MIT | có |
| side-channel | 1.1.1 | MIT | có |
| side-channel-list | 1.0.1 | MIT | có |
| side-channel-map | 1.0.1 | MIT | có |
| side-channel-weakmap | 1.0.2 | MIT | có |
| signal-exit | 3.0.7 | ISC | có |
| signal-exit | 4.1.0 | ISC | có |
| sisteransi | 1.0.5 | MIT | có |
| smart-buffer | 4.2.0 | MIT | có |
| socks | 2.8.10 | MIT | có |
| source-map | 0.6.1 | BSD-3-Clause | có |
| source-map-js | 1.2.2 | BSD-3-Clause | có |
| statuses | 2.0.2 | MIT | có |
| stdin-discarder | 0.2.2 | MIT | có |
| string-width | 7.2.0 | MIT | có |
| stringify-object | 5.0.0 | BSD-2-Clause | có |
| strip-ansi | 6.0.1 | MIT | có |
| strip-ansi | 7.2.0 | MIT | có |
| strip-ansi | 7.2.0 | MIT | có |
| strip-bom | 3.0.0 | MIT | có |
| strip-final-newline | 2.0.0 | MIT | có |
| strip-final-newline | 4.0.0 | MIT | có |
| supports-color | 10.2.2 | MIT | có |
| systeminformation | 5.33.16 | MIT | có |
| tailwindcss | 4.3.3 | MIT | có |
| tapable | 2.3.3 | MIT | có |
| tiny-invariant | 1.3.3 | MIT |  |
| tinyglobby | 0.2.17 | MIT | có |
| to-regex-range | 5.0.1 | MIT | có |
| toidentifier | 1.0.1 | MIT | có |
| ts-morph | 26.0.0 | MIT | có |
| tsconfig-paths | 4.2.0 | MIT | có |
| tslib | 2.8.1 | 0BSD | có |
| tslib | 2.8.1 | 0BSD |  |
| tw-animate-css | 1.4.0 | MIT |  |
| type-fest | 4.41.0 | (MIT OR CC0-1.0) | có |
| type-is | 2.1.0 | MIT | có |
| typescript | 6.0.3 | Apache-2.0 | có |
| undici | 7.30.1 | MIT | có |
| undici-types | 7.24.6 | MIT | có |
| unicorn-magic | 0.3.0 | MIT | có |
| universalify | 2.0.1 | MIT | có |
| unpipe | 1.0.0 | MIT | có |
| update-browserslist-db | 1.3.4 | MIT | có |
| uri-js-replace | 1.0.1 | MIT | có |
| use-callback-ref | 1.3.3 | MIT |  |
| use-sidecar | 1.1.3 | MIT |  |
| use-sync-external-store | 1.7.0 | MIT |  |
| util-deprecate | 1.0.2 | MIT | có |
| validate-npm-package-name | 7.0.2 | ISC | có |
| vary | 1.1.2 | MIT | có |
| victory-vendor | 37.3.6 | MIT AND ISC |  |
| vite | 8.3.4 | MIT | có |
| which | 2.0.2 | ISC | có |
| which | 4.0.0 | ISC | có |
| wrappy | 1.0.2 | ISC | có |
| wsl-utils | 1.0.1 | MIT | có |
| yallist | 3.1.1 | ISC | có |
| yaml-ast-parser | 0.0.43 | Apache-2.0 | có |
| yargs-parser | 21.1.1 | ISC | có |
| yocto-spinner | 1.2.2 | MIT | có |
| yoctocolors | 2.2.0 | MIT | có |
| zod | 4.6.7 | MIT | có |
| zod-to-json-schema | 3.25.2 | ISC | có |
