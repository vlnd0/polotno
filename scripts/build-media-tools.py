#!/usr/bin/env python3
"""Build LGPL FFmpeg + BSD OpenH264 for each APK ABI; no network protocols or device access."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import urllib.request

ROOT=Path(__file__).resolve().parent.parent
CACHE=Path(os.environ.get('POLOTNO_MEDIA_CACHE',str(ROOT/'.tools/media')))
CACHE.mkdir(parents=True,exist_ok=True)
SDK=Path(os.environ.get('ANDROID_HOME',os.environ.get('ANDROID_SDK_ROOT','')))
if not str(SDK) or str(SDK)=='.':
    SDK=Path(next(line.split('=',1)[1] for line in (ROOT/'local.properties').read_text().splitlines() if line.startswith('sdk.dir=')))
NDK=Path(os.environ.get('ANDROID_NDK_HOME',str(SDK/'ndk/27.0.12077973')))
HOST='darwin-x86_64' if os.uname().sysname=='Darwin' else 'linux-x86_64'
BIN=NDK/'toolchains/llvm/prebuilt'/HOST/'bin'

SOURCES=[('ffmpeg','https://ffmpeg.org/releases/ffmpeg-8.0.3.tar.xz','6136812ea6d4e68bdba27e33c2a94382711cdf4f8602ffef056ff792bd6f9818','ffmpeg-8.0.3'),('openh264','https://github.com/cisco/openh264/archive/refs/tags/v2.6.0.tar.gz','558544ad358283a7ab2930d69a9ceddf913f4a51ee9bf1bfb9e377322af81a69','openh264-2.6.0')]
for name,url,checksum,folder in SOURCES:
    archive=CACHE/(name+('.tar.xz' if name=='ffmpeg' else '.tar.gz'))
    if not archive.exists():urllib.request.urlretrieve(url,archive)
    if hashlib.sha256(archive.read_bytes()).hexdigest()!=checksum:raise SystemExit('Source checksum mismatch: '+name)
    if not (CACHE/folder).exists():
        with tarfile.open(archive) as package:package.extractall(CACHE,filter='data')

ffmpeg=CACHE/'ffmpeg-8.0.3'
for abi,arch,triple in [('armeabi-v7a','arm','armv7a-linux-androideabi'),('arm64-v8a','arm64','aarch64-linux-android'),('x86_64','x86_64','x86_64-linux-android')]:
    work=CACHE/abi;work.mkdir(exist_ok=True)
    output=ROOT/'android/app/build/mediaJniLibs'/abi;output.mkdir(parents=True,exist_ok=True)
    stamp=work/'settings-v2-zlib'
    if stamp.exists() and (output/'libpolotno_ffmpeg.so').exists() and (output/'libpolotno_ffprobe.so').exists():continue
    source=work/'openh264'
    if not source.exists():shutil.copytree(CACHE/'openh264-2.6.0',source,ignore=shutil.ignore_patterns('*.o','*.a','*.so','*.d'))
    prefix=work/'openh264-prefix'
    env=os.environ.copy();env.pop('LDFLAGS',None);env.pop('CPPFLAGS',None)
    with (work/'openh264.log').open('w') as log:
        subprocess.run(['make','-j6','OS=android','ARCH='+arch,'NDKROOT='+str(NDK),'TARGET=android-26','NDKLEVEL=26','USE_ASM=No','PREFIX='+str(prefix),'libraries','install-static'],cwd=source,env=env,stdout=log,stderr=log,check=True)
    pkg=work/'pkg-config'
    pkg.write_text("#!/usr/bin/env python3\nimport sys\na=sys.argv[1:]\nif '--version' in a:print('1.9.0')\nelif '--modversion' in a:print('2.6.0')\nelif any(s.startswith('--cflags') for s in a):print("+repr('-I'+str(prefix/'include'))+")\nelif any(s.startswith('--libs') for s in a):print("+repr('-L'+str(prefix/'lib')+' -lopenh264 -lc++_static -lc++abi -latomic')+")\n")
    pkg.chmod(0o755)
    build=work/'ffmpeg';build.mkdir(exist_ok=True)
    configure=[str(ffmpeg/'configure'),'--enable-cross-compile','--target-os=android','--arch='+('aarch64' if arch=='arm64' else arch),'--cc='+str(BIN/(triple+'26-clang')),'--cxx='+str(BIN/(triple+'26-clang++')),'--ld='+str(BIN/(triple+'26-clang++'))]
    for tool in ['ar','nm','ranlib','strip']:configure+=['--'+tool+'='+str(BIN/('llvm-'+tool))]
    configure+=['--disable-shared','--enable-static','--enable-pic','--disable-doc','--disable-debug','--disable-avdevice','--disable-network','--disable-autodetect','--disable-x86asm','--enable-zlib','--enable-libopenh264','--pkg-config='+str(pkg),'--extra-cflags=-fPIE','--extra-ldflags=-pie -static-libstdc++ -Wl,-z,max-page-size=16384']
    with (work/'ffmpeg.log').open('w') as log:
        subprocess.run(configure,cwd=build,env=env,stdout=log,stderr=log,check=True)
        subprocess.run(['make','-j6','ffmpeg','ffprobe'],cwd=build,env=env,stdout=log,stderr=log,check=True)
    for name in ['ffmpeg','ffprobe']:shutil.copy2(build/name,output/('libpolotno_'+name+'.so'))
    stamp.write_text('FFmpeg 8.0.3, OpenH264 2.6.0, Android zlib\n')
    print('Media tools ready: '+abi,flush=True)

licenses=ROOT/'android/app/build/mediaLicenseAssets/licenses/media';licenses.mkdir(parents=True,exist_ok=True)
for name in ['COPYING.LGPLv2.1','COPYING.LGPLv3','LICENSE.md']:shutil.copy2(ffmpeg/name,licenses/('FFmpeg-'+name))
shutil.copy2(CACHE/'openh264-2.6.0/LICENSE',licenses/'OpenH264-LICENSE.txt')
