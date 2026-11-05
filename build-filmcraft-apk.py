"""Package the native Rust editor with Android platform file access; no Gradle needed."""
from pathlib import Path
import argparse
from scripts.android_release import add_version_arguments, package_manifest, signing_identity
import datetime
import hashlib
import json
import os
import shutil
import subprocess
import zipfile
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('--abi', choices=['arm64-v8a', 'x86_64'], default='arm64-v8a')
parser.add_argument('--skip-rust', action='store_true')
add_version_arguments(parser)
args = parser.parse_args()
target = {'arm64-v8a': 'aarch64-linux-android', 'x86_64': 'x86_64-linux-android'}[args.abi]
sdk = Path(os.environ['ANDROID_HOME'])
java = Path(os.environ['JAVA_HOME']) / 'bin'
build_tools = Path(os.environ.get('ANDROID_BUILD_TOOLS', str(sdk / 'build-tools' / 'android-15')))
android_jar = sdk / 'platforms' / 'android-35' / 'android.jar'
source = root / 'filmcraft-android'
work = source / 'build' / 'package' / args.abi
work.mkdir(parents=True, exist_ok=True)
artifacts = root / 'artifacts/filmcraft'
artifacts.mkdir(parents=True, exist_ok=True)

def run(*command):
    print('Running: ' + Path(str(command[0])).name, flush=True)
    subprocess.run([str(item) for item in command], check=True, cwd=root)

keystore, key_alias = signing_identity(root, java, run, 'filmcraft')

if not args.skip_rust:
    run('cargo', 'build', '--locked', '--manifest-path', source / 'Cargo.toml', '--release', '--target', target)

res = work / 'res'
assets = work / 'assets'
classes = work / 'classes'
dex = work / 'dex'
for directory in [res, assets, classes, dex]:
    directory.mkdir(parents=True, exist_ok=True)
for density, pixels in [('mdpi', 48), ('hdpi', 72), ('xhdpi', 96), ('xxhdpi', 144), ('xxxhdpi', 192)]:
    icon = root / 'assets/app-icon/hicolor' / f'{pixels}x{pixels}/apps/ai.storyteller.filmcraft.png'
    if not icon.exists():
        icon = root / 'assets/app-icon/hicolor/256x256/apps/ai.storyteller.filmcraft.png'
    destination = res / f'mipmap-{density}'
    destination.mkdir(exist_ok=True)
    shutil.copyfile(icon, destination / 'ic_launcher.png')
# Android launchers mask the adaptive icon directly instead of wrapping a desktop
# bitmap in a white compatibility badge. Reuse the original artwork unchanged.
for resource in (source / 'res').rglob('*'):
    if resource.is_file():
        destination = res / resource.relative_to(source / 'res')
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(resource, destination)
artwork = res / 'drawable-nodpi/filmcraft_artwork.png'
artwork.parent.mkdir(parents=True, exist_ok=True)
shutil.copyfile(root / 'assets/app-icon/hicolor/512x512/apps/ai.storyteller.filmcraft.png', artwork)
for name in ['LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE']:
    shutil.copyfile(root / name, assets / name)
for license_file in (root / 'assets').rglob('*'):
    if license_file.is_file() and license_file.name.startswith(('LICENSE', 'OFL', 'COPYING')):
        destination = assets / 'licenses' / license_file.relative_to(root / 'assets')
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(license_file, destination)
for license_file in (root / 'crates').rglob('LICENSE*'):
    if license_file.is_file():
        destination = assets / 'licenses' / license_file.relative_to(root)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(license_file, destination)
shutil.copyfile(root / 'ATTRIBUTION.md', assets / 'ATTRIBUTION.md')
for license_file in (root / 'docs/brand').glob('LICENSE*'):
    destination = assets / 'licenses/brand' / license_file.name
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(license_file, destination)
# Copy only source-controlled sidecars, never packaging output recursively.
attribution_paths = subprocess.check_output(['git', 'ls-files', '*.attribution'], cwd=root, text=True, encoding='utf-8').splitlines()
for relative in attribution_paths:
    attribution = root / relative
    destination = assets / 'licenses/attributions' / attribution.relative_to(root)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(attribution, destination)
for license_file in (root / 'craft-fonts').rglob('*'):
    if '.git' not in license_file.parts and license_file.is_file() and license_file.name.upper().startswith(('LICENSE', 'OFL', 'ATTRIBUTION')):
        destination = assets / 'licenses/craft-fonts' / license_file.relative_to(root / 'craft-fonts')
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(license_file, destination)
metadata = json.loads(subprocess.check_output([
    'cargo', 'metadata', '--locked', '--format-version', '1', '--filter-platform', target,
    '--manifest-path', str(source / 'Cargo.toml'),
], text=True, encoding='utf-8'))
active = {node['id'] for node in metadata['resolve']['nodes']}
package_notices = []
for package in metadata['packages']:
    if package['id'] not in active or package['source'] is None:
        continue
    package_notices.append({key: package.get(key) for key in ['name', 'version', 'license', 'source']})
    package_root = Path(package['manifest_path']).parent
    license_files = [p for p in package_root.iterdir() if p.is_file() and p.name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE'))]
    if package.get('license_file'):
        license_files.append(package_root / package['license_file'])
    if package['name'] == 'epaint_default_fonts':
        license_files.extend((package_root / 'fonts').glob('*.txt'))
    for license_file in license_files:
        if license_file.is_file():
            destination = assets / 'licenses/dependencies' / f"{package['name']}-{package['version']}" / license_file.name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(license_file, destination)
(assets / 'THIRD-PARTY-PACKAGES.json').write_text(json.dumps(package_notices, indent=2) + '\n', encoding='utf-8')
(assets / 'ANDROID-PORT.txt').write_text(
    'Unofficial native Android packaging of FilmCraft 0.10.0.\n'
    'Based on FilmCraft by the ArtCraft team and contributors.\n'
    'Source: https://github.com/storytold/filmcraft\n'
    'The editor, video engine and renderer are Rust. The NativeActivity platform adapter provides Android document access.\n'
    'Signed with a local test key for sideloading; this is not an upstream release.\n', encoding='utf-8')

java_sources = sorted((source / 'java').rglob('*.java'))
if args.abi == 'x86_64':
    java_sources += sorted((source / 'tests/android').rglob('*.java'))
build_config = work / 'BuildConfig.java'
build_config.write_text('package app.filmcraft.android; public final class BuildConfig { public static final boolean INSTRUMENTED = '
                        + ('true' if args.abi == 'x86_64' else 'false') + '; }\n', encoding='utf-8')
java_sources.append(build_config)
run(java / 'javac.exe', '--release', '8', '-encoding', 'UTF-8',
    '-classpath', android_jar, '-d', classes, *java_sources)
class_files = sorted(classes.rglob('*.class'))
run(java / 'java.exe', '-cp', build_tools / 'lib/d8.jar', 'com.android.tools.r8.D8',
    '--release', '--min-api', '28', '--lib', android_jar, '--output', dex, *class_files)
compiled = work / 'resources.zip'
run(build_tools / 'aapt2.exe', 'compile', '--dir', res, '-o', compiled)
unsigned = work / 'unsigned.apk'
manifest_path = source / 'AndroidManifest.xml'
if args.abi == 'x86_64':
    instrumented_manifest = work / 'AndroidManifest.xml'
    instrumented_manifest.write_text(manifest_path.read_text().replace('</application>',
        '<provider android:name=".TestDocumentsProvider" android:authorities="app.filmcraft.android.test.documents" '
        'android:exported="true" android:permission="android.permission.MANAGE_DOCUMENTS" android:grantUriPermissions="true" />\n</application>'), encoding='utf-8')
    manifest_path = instrumented_manifest
manifest_path = package_manifest(manifest_path, work, args)
run(build_tools / 'aapt2.exe', 'link', '-o', unsigned, '-I', android_jar,
    '--manifest', manifest_path, '-A', assets, compiled)
library = Path(os.environ.get('CARGO_TARGET_DIR', str(source / 'target'))) / target / 'release/libfilmcraft_android.so'
with zipfile.ZipFile(unsigned, 'a') as archive:
    archive.write(dex / 'classes.dex', 'classes.dex', compress_type=zipfile.ZIP_DEFLATED)
    archive.write(library, f'lib/{args.abi}/libfilmcraft_android.so', compress_type=zipfile.ZIP_STORED)

aligned = work / 'aligned.apk'
run(build_tools / 'zipalign.exe', '-f', '-P', '16', '4', unsigned, aligned)
label = 'arm64' if args.abi == 'arm64-v8a' else 'x86_64'
manifest = ET.parse(manifest_path).getroot()
android_namespace = '{http://schemas.android.com/apk/res/android}'
version_name = manifest.attrib[android_namespace + 'versionName']
version_code = int(manifest.attrib[android_namespace + 'versionCode'])
apk = artifacts / f'filmcraft-{version_name}-{label}.apk'
run(java / 'java.exe', '-jar', build_tools / 'lib/apksigner.jar', 'sign', '--ks', keystore,
    '--ks-key-alias', key_alias, '--ks-pass', 'env:ANDROID_STORE_PASSWORD', '--key-pass', 'env:ANDROID_KEY_PASSWORD',
    '--v4-signing-enabled', 'false', '--out', apk, aligned)
run(java / 'java.exe', '-jar', build_tools / 'lib/apksigner.jar', 'verify', '--verbose', '--print-certs', apk)
run(build_tools / 'zipalign.exe', '-c', '-P', '16', '4', apk)
run(build_tools / 'aapt2.exe', 'dump', 'badging', apk)
digest = hashlib.sha256(apk.read_bytes()).hexdigest()
checksums = artifacts / 'SHA256SUMS.txt'
lines = [line for line in (checksums.read_text().splitlines() if checksums.exists() else []) if not line.endswith(apk.name)]
lines.append(f'{digest}  {apk.name}')
checksums.write_text('\n'.join(lines) + '\n')
record = {
    'apk': apk.name, 'sha256': digest, 'size_bytes': apk.stat().st_size, 'abi': args.abi,
    'package': 'app.filmcraft.android', 'min_android': '9 (API 28)', 'target_sdk': 35,
    'version_name': version_name, 'version_code': version_code,
    'built_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'upstream': 'https://github.com/storytold/filmcraft',
    'upstream_commit': subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip(),
    'signing': 'Unofficial sideload certificate; not upstream or Play Store signing',
    'permissions': [], 'native_library_alignment': '16 KiB',
    'instrumented_emulator_build': args.abi == 'x86_64',
    'font_source': subprocess.check_output(['git', '-C', str(root / 'craft-fonts'), 'rev-parse', 'HEAD'], text=True).strip(),
}
(artifacts / f'build-{label}.json').write_text(json.dumps(record, indent=2) + '\n')
print(f'APK ready: {apk}\nSHA-256: {digest}', flush=True)
