"""Compile the editable Icon Composer document and merge its primary icon metadata."""
from pathlib import Path
import sys,subprocess,plistlib,tempfile,shutil
source=Path(__file__).resolve().parents[2]/'PeggleSourceTest.icon'
app=Path(sys.argv[1]);platform=sys.argv[2]
assert platform in ('iphoneos','iphonesimulator')
with tempfile.TemporaryDirectory(prefix='peggle-icon-') as tmp:
 compiled=Path(tmp)/'compiled';compiled.mkdir();partial=Path(tmp)/'Info.plist'
 subprocess.run(['xcrun','actool','--compile',str(compiled),'--platform',platform,'--minimum-deployment-target','15.0','--target-device','iphone','--target-device','ipad','--app-icon',source.stem,'--output-partial-info-plist',str(partial),str(source)],check=True)
 info=plistlib.loads((app/'Info.plist').read_bytes())
 for key in ['CFBundleIconFile','CFBundleIconFiles','CFBundleIcons','CFBundleIcons~ipad']:info.pop(key,None)
 info.update(plistlib.loads(partial.read_bytes()));(app/'Info.plist').write_bytes(plistlib.dumps(info))
 for f in compiled.iterdir():shutil.copy2(f,app/f.name)
