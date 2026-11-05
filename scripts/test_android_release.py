"""Regression checks for installable, versioned CI releases (no SDK needed)."""
from pathlib import Path
from types import SimpleNamespace
import os
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET
from android_release import ANDROID, ci_version, package_manifest, signing_identity


class AndroidReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / 'AndroidManifest.xml'
        self.original = ('<manifest xmlns:android="http://schemas.android.com/apk/res/android" '
                         'android:versionName="0.12.0-android.15" android:versionCode="120015">'
                         '<application><activity android:name=".Editor" /></application></manifest>')
        self.source.write_text(self.original)
        self.work = self.root / 'build'
        self.work.mkdir()

    def test_packaged_version_changes_without_mutating_source_or_activity(self):
        output = package_manifest(self.source, self.work, SimpleNamespace(version_name='0.12.0-android.ci.5', version_code=1000000005))
        self.assertEqual(self.source.read_text(), self.original)
        manifest = ET.parse(output).getroot()
        self.assertEqual(manifest.attrib[ANDROID + 'versionCode'], '1000000005')
        self.assertEqual(manifest.attrib[ANDROID + 'versionName'], '0.12.0-android.ci.5')
        self.assertEqual(manifest.find('./application/activity').attrib[ANDROID + 'name'], '.Editor')

    def test_invalid_or_partial_version_cannot_produce_a_release(self):
        for name, code in [(None, 4), ('1.0', None), ('1\" bad', 5), ('1.0', 0), ('1.0', 2100000001)]:
            with self.subTest(name=name, code=code), self.assertRaises(ValueError):
                package_manifest(self.source, self.work, SimpleNamespace(version_name=name, version_code=code))

    def test_local_build_preserves_manifest_version(self):
        self.assertEqual(package_manifest(self.source, self.work, SimpleNamespace(version_name=None, version_code=None)), self.source)

    def test_ci_never_generates_an_ephemeral_signing_key(self):
        with patch.dict(os.environ, {'CI': 'true'}, clear=True), self.assertRaises(ValueError):
            signing_identity(self.root, self.root, lambda *args: self.fail('keytool must not run'), 'example')
        self.assertFalse((self.root / '.private').exists())

    def test_configured_missing_keystore_fails_instead_of_replacing_it(self):
        with patch.dict(os.environ, {'CI': 'true', 'ANDROID_KEYSTORE': str(self.root / 'missing.jks')}, clear=True), self.assertRaises(ValueError):
            signing_identity(self.root, self.root, lambda *args: self.fail('keytool must not run'), 'example')

    def test_existing_signing_identity_is_reused(self):
        key = self.root / 'retained.jks'
        key.write_bytes(b'test fixture, not a real key')
        with patch.dict(os.environ, {'CI': 'true', 'ANDROID_KEYSTORE': str(key), 'ANDROID_KEY_ALIAS': 'retained'}, clear=True):
            self.assertEqual(signing_identity(self.root, self.root, lambda *args: self.fail('keytool must not run'), 'example'), (key, 'retained'))

    def test_run_numbers_make_versions_unique_and_upgrade_existing_apps(self):
        cargo = self.root / 'Cargo.toml'
        cargo.write_text('[workspace.package]\nversion = "0.12.0"\n')
        first = ci_version(self.source, 1, cargo)
        second = ci_version(self.source, 2, cargo)
        self.assertEqual(first, ('0.12.0-android.ci.1', 1000000001))
        self.assertGreater(second[1], first[1])
        self.assertGreater(first[1], 120015)
        for invalid in [0, -1, 1100000001]:
            with self.assertRaises(ValueError):
                ci_version(self.source, invalid, cargo)


if __name__ == '__main__':
    unittest.main()
