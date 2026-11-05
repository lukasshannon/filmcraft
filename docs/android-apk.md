# FilmCraft Android APK branch

This branch carries the native Android port developed in this chat, based on
`storytold/filmcraft` at `45901fccd06f1c2aa1dcdaac67cc00262a85f1be`. The existing editor changes are applied directly
to their original files. `filmcraft-android/` is an independent Cargo workspace containing
the NativeActivity adapter, document-provider bridge, manifest, icon resources
and pinned Cargo.lock. The Android adapter points at this checkout's `crates/`.

## Build on Windows

Install Python 3, Git and Visual Studio C++ Build Tools. In this checkout:

```powershell
python setup-toolchain.py
$env:CARGO_HOME = Join-Path $PWD '.tools/cargo'
$env:RUSTUP_HOME = Join-Path $PWD '.tools/rustup'
& ./.tools/rustup-init.exe -y --no-modify-path --default-toolchain 1.95.0
. ./build-filmcraft-env.ps1
rustup target add aarch64-linux-android x86_64-linux-android wasm32-unknown-unknown
git clone https://github.com/storytold/craft-fonts.git craft-fonts
git -C craft-fonts checkout 43a994403242b287ab4c78499b7ff7d943ea6dba
python build-filmcraft-apk.py --abi arm64-v8a
python build-filmcraft-apk.py --abi x86_64
```

The helper uses JDK 21, Android SDK 35, NDK r27c and 16 KiB ELF/ZIP alignment.
APKs and metadata are written under `artifacts/`. The build generates a local
test signing key under `.private/` if absent. A newly generated key cannot
update the already distributed APK in place; retain your own signing identity
for subsequent builds. No signing keys, SDKs, APKs, fonts, provider grants,
private hosting configuration or uploaded user screenshots are committed here.

## Validation and provenance

The editor and adapter sources are the previously validated Android port. This
publication reorganizes packaging paths for a standalone fork. It does not
rebase onto newer upstream code or claim another full emulator/build run.
The adapter's locked Cargo metadata and Python syntax are checked after relocation.
The original upstream repository and its licenses remain the source of truth.

## Upstream contribution status

This is an unofficial Android distribution branch, not a claim that the port
has passed upstream review. The editor stays native Rust/egui; the Android
Activity uses handwritten Java for OS document-provider, lifecycle and keyboard
integration. Packaging uses Python and PowerShell. Each native entry also uses
Rust 2024's `#[unsafe(no_mangle)]` attribute to export `android_main`; it is
an unsafe attribute, not an unsafe block. These boundaries must be reviewed
against the upstream Rust-only and unsafe-code policies before submission.
AAudio output also uses one explicit unsafe OS call in `crates/platform/src/android_audio.rs`; its scope needs maintainer review.

Related request: [#1852](https://github.com/storytold/filmcraft/issues/1852).
Overlapping work already exists: [#3107](https://github.com/storytold/filmcraft/pull/3107), [#2737](https://github.com/storytold/filmcraft/pull/2737).
A future upstream proposal must explain that overlap, build on existing work
where appropriate, and use a reviewed current-main base. The tested APK base
above is deliberately retained here; an upstream rebase and its tests have not
been completed. New UI text and shortcuts, icon attribution, dependency policy
and the repository's PR template/checks must all be verified on that proposal.

Do not mark CI, device coverage or upstream-policy exceptions as approved
based only on the previous APK tests or the packaging checks above.

## Automatic APK releases

The fork's default integration/release branch is `android-apk`. Every push to
that branch (including a merged PR) builds the ARM64 phone APK and publishes
a public GitHub Release. `pull_request` runs the packaging regression tests
without signing secrets. Actions also offers a manual run on `android-apk`.
This workflow does not update the private download site.

CI uses Rust 1.95.0, JDK 21, SDK/build-tools 35 and NDK 27.2.12479018,
with 16 KiB native-library and APK alignment. It checks out craft-fonts at
`43a994403242b287ab4c78499b7ff7d943ea6dba` and ships its license notices.
Only ARM64 production builds are released; the existing x86_64 helper may
include emulator instrumentation and is not a public release asset.

Versions are `<Cargo workspace version>-android.ci.<workflow run number>`,
with Android versionCode `1000000000 + run number`. Tags are `android-v<version>`.
Rerunning the same workflow preserves that version and resumes the same draft
release. A completed published release is immutable to reruns. Do not rename
or recreate this workflow without preserving its increasing versionCode range.
The source manifest is not modified; CI versions only the packaging copy.

The build requires repository Actions secret `ANDROID_KEYSTORE_BASE64`, which
must be base64 of this app's existing `.private/filmcraft-test.jks` (local password
`android`, alias `filmcraft-test`). GitHub encrypts that value at rest. The workflow
checks the public certificate fingerprint before signing, refuses to generate
a replacement key in CI, and deletes its temporary key in an `always()` step.
Never commit a key, base64 value or password override. For another signing
identity, configure `ANDROID_KEY_ALIAS`, `ANDROID_STORE_PASSWORD` and
`ANDROID_KEY_PASSWORD` in the build environment and review the pinned public
fingerprint; changing identity prevents updates of existing installations.

The release contains the versioned APK, SHA256SUMS.txt and build metadata
recording the source commit, version and signing identity. The build job has
read-only repository permission; only the separate publishing job gets
`contents: write`. No PAT or long-lived GitHub credential is needed by CI.
