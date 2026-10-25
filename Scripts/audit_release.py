#!/usr/bin/env python3
"""Audit a public Peggle IPA and its unchanged sanitized guest payload."""
from pathlib import Path
import argparse,hashlib,io,json,plistlib,struct,zipfile
p=argparse.ArgumentParser(description=__doc__);p.add_argument('ipa',type=Path);a=p.parse_args()
repo=Path(__file__).resolve().parents[1]
forbidden={'embedded.mobileprovision','_CodeSignature','SC_Info','iTunesMetadata.plist','.DS_Store'}
with zipfile.ZipFile(a.ipa) as z:
 assert z.testzip() is None
 assert not any(forbidden.intersection(Path(n).parts) or any(x.startswith('._') for x in Path(n).parts) for n in z.namelist())
 roots=[n for n in z.namelist() if n.startswith('Payload/') and len(Path(n).parts)==3 and n.endswith('/Info.plist')];assert len(roots)==1
 root=roots[0].removesuffix('Info.plist');info=plistlib.loads(z.read(roots[0]));exe=z.read(root+info['CFBundleExecutable'])
 assert struct.unpack_from('<II',exe)==(0xfeedfacf,0x100000c),'Expected ARM64 Mach-O host'
 pos=32;platform=None
 for _ in range(struct.unpack_from('<I',exe,16)[0]):
  cmd,size=struct.unpack_from('<II',exe,pos)
  assert cmd!=0x1d,'Host still has a code signature'
  if cmd==0x32:platform=struct.unpack_from('<I',exe,pos+8)[0]
  pos+=size
 assert platform==2,'Expected iOS device host, not simulator'
 assert info['CFBundleSupportedPlatforms']==['iPhoneOS']
 assert info['CFBundleVersion']==json.loads((repo/'distribution.json').read_text())['build']
 assert str(info['RespringProgressCloudKit']).lower() in ('false','no','0'),'Default unsigned release must not claim CloudKit provisioning'
 assert info['CFBundleIcons']['CFBundlePrimaryIcon']['CFBundleIconName']=='PeggleSourceTest'
 with zipfile.ZipFile(io.BytesIO(z.read(root+'Peggle.ipa'))) as guest:
  assert guest.testzip() is None
  actual={i.filename:hashlib.sha256(guest.read(i)).hexdigest() for i in guest.infolist() if not i.is_dir()}
  expected=json.loads((repo/'Original/payload-sha256.json').read_text());assert actual==expected
 assert any(n.startswith(root+'ThirdPartyNotices/') for n in z.namelist())
 print(json.dumps({'ipa':a.ipa.name,'sha256':hashlib.file_digest(a.ipa.open('rb'),'sha256').hexdigest(),'bytes':a.ipa.stat().st_size,'name':info['CFBundleDisplayName'],'version':info['CFBundleShortVersionString'],'build':info['CFBundleVersion'],'guest_entries_verified':len(actual),'arm64_device':True,'unsigned':True,'signing_metadata_excluded':True},indent=2))
