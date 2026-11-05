. (Join-Path $PSScriptRoot 'build-env.ps1')
$env:RUSTFLAGS = '-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-z,common-page-size=16384'
$env:CARGO_TARGET_DIR = Join-Path $PSScriptRoot 'filmcraft-android/target'
$env:CRAFT_FONTS_DIR = Join-Path $PSScriptRoot 'craft-fonts'
$env:CRAFT_FONTS_REQUIRED = '1'
$env:PYTHONUTF8 = '1'
