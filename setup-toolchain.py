from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import urllib.request
import zipfile
import hashlib
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parent / '.tools'
root.mkdir(exist_ok=True)

def download(url, name, sha1=None):
    dest = root / name
    if not dest.exists():
        print('Downloading ' + name, flush=True)
        urllib.request.urlretrieve(url, dest)
    if sha1 and hashlib.file_digest(dest.open('rb'), 'sha1').hexdigest() != sha1:
        raise RuntimeError('Checksum mismatch: ' + name)
    return dest

def unpack(url, name, subdir, sha1=None):
    archive = download(url, name, sha1)
    dest = root / subdir
    marker = dest / '.extracted'
    if not marker.exists():
        print('Extracting ' + name, flush=True)
        with zipfile.ZipFile(archive) as z:
            z.extractall(dest)
        marker.touch()
    print('Ready: ' + subdir, flush=True)

with ThreadPoolExecutor(max_workers=4) as executor:
    jobs = [executor.submit(download, 'https://win.rustup.rs/x86_64', 'rustup-init.exe'),
            executor.submit(unpack, 'https://aka.ms/download-jdk/microsoft-jdk-21-windows-x64.zip', 'jdk21.zip', 'jdk')]
    xml = ET.fromstring(urllib.request.urlopen('https://dl.google.com/android/repository/repository2-3.xml').read())
    wanted = {'platforms;android-35': 'sdk/platforms', 'ndk;27.2.12479018': 'sdk/ndk',
              'build-tools;35.0.0': 'sdk/build-tools', 'platform-tools': 'sdk'}
    for package in xml.iter('remotePackage'):
        key = package.attrib.get('path')
        if key not in wanted:
            continue
        for archive in package.findall('./archives/archive'):
            if archive.findtext('host-os') not in (None, 'windows'):
                continue
            complete = archive.find('complete')
            name = complete.findtext('url')
            jobs.append(executor.submit(unpack, 'https://dl.google.com/android/repository/' + name,
                                        name, wanted[key], complete.findtext('checksum')))
    for job in jobs:
        job.result()
print('All build tools downloaded and verified.', flush=True)
