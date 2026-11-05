$taskRoot = $PSScriptRoot
$env:CARGO_HOME = Join-Path $taskRoot '.tools\cargo'
$env:RUSTUP_HOME = Join-Path $taskRoot '.tools\rustup'
$env:JAVA_HOME = (Get-ChildItem -LiteralPath (Join-Path $taskRoot '.tools\jdk') -Directory | Select-Object -First 1).FullName
$env:ANDROID_HOME = Join-Path $taskRoot '.tools\sdk'
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
$env:ANDROID_NDK_HOME = Join-Path $env:ANDROID_HOME 'ndk\android-ndk-r27c'
$env:PATH = "$env:CARGO_HOME\bin;$env:JAVA_HOME\bin;$env:ANDROID_HOME\platform-tools;$env:PATH"
$ndkBin = Join-Path $env:ANDROID_NDK_HOME 'toolchains\llvm\prebuilt\windows-x86_64\bin'
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = Join-Path $ndkBin 'aarch64-linux-android28-clang.cmd'
$env:CC_aarch64_linux_android = $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER
$env:AR_aarch64_linux_android = Join-Path $ndkBin 'llvm-ar.exe'
$env:CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER = Join-Path $ndkBin 'x86_64-linux-android28-clang.cmd'
$env:CC_x86_64_linux_android = $env:CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER
$env:AR_x86_64_linux_android = Join-Path $ndkBin 'llvm-ar.exe'
$env:RUSTFLAGS = '-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-z,common-page-size=16384'
