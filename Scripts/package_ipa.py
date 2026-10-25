#!/usr/bin/env python3
"""Make a public, unsigned IPA without local provisioning or account metadata."""
import argparse,hashlib,plistlib,shutil,subprocess,tempfile,zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('app',type=Path)
p.add_argument('--output',type=Path,default=Path('release-output/PeggleHD.ipa'))
a=p.parse_args()
with tempfile.TemporaryDirectory(prefix='peggle-public-') as tmp:
 app=Path(tmp)/'Payload/PeggleHD.app';shutil.copytree(a.app,app)
 info=plistlib.loads((app/'Info.plist').read_bytes());binary=app/info['CFBundleExecutable']
 subprocess.run(['codesign','--remove-signature',str(app)],check=True)
 for f in app.rglob('embedded.mobileprovision'):f.unlink()
 for f in list(app.rglob('_CodeSignature')):
  if f.is_dir():shutil.rmtree(f)
 for f in app.rglob('iTunesMetadata.plist'):f.unlink()
 # The game is nested in the host IPA; clean the nested archive too.
 for ipa in app.rglob('*.ipa'):
  with zipfile.ZipFile(ipa) as z:
   entries=[(i,z.read(i)) for i in z.infolist() if i.filename.startswith('Payload/') and not any(s in i.filename for s in ('embedded.mobileprovision','_CodeSignature/','SC_Info/','iTunesMetadata.plist'))]
  with zipfile.ZipFile(ipa,'w',zipfile.ZIP_DEFLATED,strict_timestamps=False) as z:
   for i,b in entries:z.writestr(i,b)
 a.output.parent.mkdir(parents=True,exist_ok=True)
 with zipfile.ZipFile(a.output,'w',zipfile.ZIP_DEFLATED,strict_timestamps=False) as z:
  for f in sorted(app.rglob('*')):
   if f.is_file() and f.name!='.DS_Store' and not f.name.startswith('._'):z.write(f,f.relative_to(Path(tmp)))
 with zipfile.ZipFile(a.output) as z:assert z.testzip() is None
print(hashlib.sha256(a.output.read_bytes()).hexdigest()+'  '+a.output.name)
