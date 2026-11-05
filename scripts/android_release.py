"""Version the APK packaging copy and require a stable signing identity in CI."""
from pathlib import Path
import argparse
import os
import re
import xml.etree.ElementTree as ET

ANDROID = '{http://schemas.android.com/apk/res/android}'


def add_version_arguments(parser):
    parser.add_argument('--version-name', help='Override only the packaged versionName')
    parser.add_argument('--version-code', type=int, help='Override only the packaged versionCode')


def package_manifest(source, work, args):
    if args.version_name is None and args.version_code is None:
        return source
    if not args.version_name or args.version_code is None:
        raise ValueError('Supply --version-name and --version-code together')
    if not re.fullmatch(r'[0-9A-Za-z][0-9A-Za-z.+-]{0,99}', args.version_name):
        raise ValueError('Invalid Android version name')
    if not 1 <= args.version_code <= 2100000000:
        raise ValueError('Android version code must be between 1 and 2100000000')
    tree = ET.parse(source)
    tree.getroot().set(ANDROID + 'versionName', args.version_name)
    tree.getroot().set(ANDROID + 'versionCode', str(args.version_code))
    ET.register_namespace('android', ANDROID[1:-1])
    output = work / 'VersionedAndroidManifest.xml'
    tree.write(output, encoding='utf-8', xml_declaration=True)
    return output


def signing_identity(root, java, run, app):
    configured = os.environ.get('ANDROID_KEYSTORE')
    in_ci = os.environ.get('CI', '').lower() == 'true' or os.environ.get('GITHUB_ACTIONS') == 'true'
    if in_ci and not configured:
        raise ValueError('CI requires ANDROID_KEYSTORE; refusing to generate a new signing key')
    keystore = Path(configured) if configured else root / '.private' / f'{app}-test.jks'
    alias = os.environ.get('ANDROID_KEY_ALIAS', f'{app}-test')
    os.environ.setdefault('ANDROID_STORE_PASSWORD', 'android')
    os.environ.setdefault('ANDROID_KEY_PASSWORD', os.environ['ANDROID_STORE_PASSWORD'])
    if configured and not keystore.is_file():
        raise ValueError('Configured Android signing keystore is missing')
    if not keystore.exists():
        keystore.parent.mkdir(parents=True, exist_ok=True)
        run(java / 'keytool.exe', '-genkeypair', '-noprompt', '-keystore', keystore,
            '-storepass:env', 'ANDROID_STORE_PASSWORD', '-keypass:env', 'ANDROID_KEY_PASSWORD',
            '-alias', alias, '-keyalg', 'RSA', '-keysize', '3072', '-validity', '10000',
            '-dname', f'CN={app} Android Local Test, OU=Unofficial Port, O=Local Development, C=CA')
    return keystore, alias


def ci_version(manifest, run_number, cargo_manifest):
    import tomllib
    base = tomllib.loads(Path(cargo_manifest).read_text(encoding='utf-8'))['workspace']['package']['version']
    number = int(run_number)
    if not 1 <= number <= 1100000000:
        raise ValueError('Workflow run number exceeds the allocated Android versionCode range')
    # Read the manifest too: a missing/wrong adapter must fail before a release starts.
    ET.parse(manifest).getroot().attrib[ANDROID + 'versionName']
    return f'{base}-android.ci.{number}', 1000000000 + number


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('manifest', type=Path)
    parser.add_argument('run_number', type=int)
    parser.add_argument('--cargo-manifest', type=Path, default=Path('Cargo.toml'))
    args = parser.parse_args()
    name, code = ci_version(args.manifest, args.run_number, args.cargo_manifest)
    with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
        output.write(f'version={name}\ncode={code}\ntag=android-v{name}\n')
