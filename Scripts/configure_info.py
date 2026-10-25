from pathlib import Path
import sys,os,plistlib
p=Path(sys.argv[1]);info=plistlib.loads(p.read_bytes())
info['CFBundleSupportedPlatforms']=['iPhoneSimulator' if sys.argv[2]=='iphonesimulator' else 'iPhoneOS']
info['RespringProgressCloudKit']=sys.argv[2]=='iphoneos' and os.environ.get('PEGGLE_CLOUDKIT_ENABLED','NO').upper() in ('YES','TRUE','1')
info['RespringCloudKitContainer']=os.environ.get('PEGGLE_CLOUDKIT_CONTAINER','iCloud.com.respringapps.peggle')
if os.environ.get('PRODUCT_BUNDLE_IDENTIFIER'):info['CFBundleIdentifier']=os.environ['PRODUCT_BUNDLE_IDENTIFIER']
p.write_bytes(plistlib.dumps(info))
