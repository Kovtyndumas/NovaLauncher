#!/usr/bin/env python3
"""Build Nova Rust. macOS packaging needs no Node, Java, or Finder automation."""
from pathlib import Path
import os, shutil, subprocess, platform, plistlib, tempfile, json, sys, time
root=Path(__file__).resolve().parent.parent
os.chdir(root)
version=json.loads((root/"tauri.conf.json").read_text())["version"]
env=os.environ.copy()
cargo=shutil.which('cargo')
if not cargo:
    tools=Path.home()/'.local/share/nova-rust-tools'
    candidate=tools/'cargo/bin/cargo'
    if candidate.is_file():
        cargo=str(candidate);env['CARGO_HOME']=str(tools/'cargo');env['RUSTUP_HOME']=str(tools/'rustup')
if not cargo: raise SystemExit('Install stable Rust from https://rustup.rs first.')
if '--skip-build' not in sys.argv:
    subprocess.run([cargo,'build','--release','--locked'],env=env,check=True)
if platform.system()!='Darwin':
    env.setdefault('TAURI_BUNDLER_TOOLS_GITHUB_MIRROR_TEMPLATE','https://github.com/<owner>/<repo>/releases/download/<version>/<asset>?nova_bundle='+str(time.time_ns()))
    subprocess.run([cargo,'tauri','build','--bundles','nsis' if os.name=='nt' else 'appimage,deb'],env=env,check=True)
    raise SystemExit(0)
out=root/'dist';out.mkdir(exist_ok=True)
app=out/'Nova Rust.app'
if app.exists():shutil.rmtree(app)
mac=app/'Contents/MacOS';res=app/'Contents/Resources';mac.mkdir(parents=True);res.mkdir()
shutil.copy2(root/'target/release/nova-rust',mac/'nova-rust')
shutil.copy2(root/'icons/icon.icns',res/'icon.icns')
info={'CFBundleName':'Nova Rust','CFBundleDisplayName':'Nova Rust','CFBundleExecutable':'nova-rust','CFBundleIdentifier':'dev.ktvx.nova.rust','CFBundleIconFile':'icon.icns','CFBundleVersion':version.split('-')[0],'CFBundleShortVersionString':version,'CFBundlePackageType':'APPL','LSMinimumSystemVersion':'11.0','LSApplicationCategoryType':'public.app-category.games','NSHighResolutionCapable':True,'NSPrincipalClass':'NSApplication','NSHumanReadableCopyright':'Created By Ktvx'}
with (app/'Contents/Info.plist').open('wb')as f:plistlib.dump(info,f)
subprocess.run(['codesign','--force','--sign','-','--timestamp=none',str(app)],check=True)
subprocess.run(['codesign','--verify','--deep','--strict',str(app)],check=True)
arch='arm64' if platform.machine()=='arm64' else 'x64'
dmg=out/f'Nova-Rust-{version}-{arch}.dmg'
with tempfile.TemporaryDirectory(prefix='nova-rust-dmg-')as tmp:
    stage=Path(tmp)
    subprocess.run(['ditto',str(app),str(stage/app.name)],check=True)
    (stage/'Applications').symlink_to('/Applications')
    command=['hdiutil','create','-ov','-volname','Nova Rust','-srcfolder',str(stage),'-format','UDZO',str(dmg)]
    for attempt in range(3):
        result=subprocess.run(command)
        if result.returncode==0: break
        if attempt==2: result.check_returncode()
        time.sleep(5)
subprocess.run(['hdiutil','verify',str(dmg)],check=True)
print('Ready:',dmg)
