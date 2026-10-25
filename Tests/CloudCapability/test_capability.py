"""Run on Apple silicon macOS: python3 Tests/CloudCapability/test_capability.py."""
from pathlib import Path
import copy, plistlib, shutil, struct, subprocess, tempfile, unittest
ROOT = Path(__file__).resolve().parents[2]
GOOD = {'com.apple.developer.icloud-services': ['CloudKit'],
        'com.apple.developer.icloud-container-identifiers': ['iCloud.test.peggle'],
        'com.apple.developer.icloud-container-environment': 'Development'}

class CapabilityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='peggle-capability-')
        cls.root = Path(cls.temp.name)
        main = cls.root / 'main.m'
        main.write_text('''#import "cloud_capability.h"
int main(int argc,const char **argv){@autoreleasepool{
 if(argc!=3)return 2;
 BOOL allowed=PeggleCloudEntitlementsAuthorize(PeggleSignedEntitlements(@(argv[1])),@"iCloud.test.peggle");
 return allowed==atoi(argv[2])?0:1;
}}
''')
        cls.driver = cls.root / 'driver'
        subprocess.run(['xcrun','clang','-arch','arm64','-fobjc-arc','-framework','Foundation',
                        '-I',str(ROOT/'Compatibility/ios'),str(main),
                        str(ROOT/'Compatibility/ios/cloud_capability.m'),'-o',str(cls.driver)],check=True)

    @classmethod
    def tearDownClass(cls): cls.temp.cleanup()

    def check(self, entitlements, expected):
        target=self.root/'fixture'
        shutil.copy2(self.driver,target)
        subprocess.run(['codesign','--remove-signature',str(target)],check=True,capture_output=True)
        if entitlements is not None:
            plist=self.root/'entitlements.plist';plist.write_bytes(plistlib.dumps(entitlements))
            subprocess.run(['codesign','--force','--sign','-','--entitlements',str(plist),str(target)],check=True,capture_output=True)
        subprocess.run([str(self.driver),str(target),str(int(expected))],check=True)

    def test_development_and_production(self):
        for environment in ['Development','Production']:
            value=copy.deepcopy(GOOD);value['com.apple.developer.icloud-container-environment']=environment
            self.check(value,True)

    def test_missing_capabilities(self):
        for key in GOOD:
            value=copy.deepcopy(GOOD);del value[key];self.check(value,False)

    def test_wrong_container_service_environment_or_types(self):
        for key,value in [('com.apple.developer.icloud-container-identifiers',['iCloud.someone.else']),
                          ('com.apple.developer.icloud-services',['CloudDocuments']),
                          ('com.apple.developer.icloud-container-environment','Unknown'),
                          ('com.apple.developer.icloud-services','CloudKit'),
                          ('com.apple.developer.icloud-container-identifiers','iCloud.test.peggle')]:
            entitlements=copy.deepcopy(GOOD);entitlements[key]=value;self.check(entitlements,False)

    def test_resigned_without_icloud(self): self.check({'get-task-allow':True},False)
    def test_unsigned(self): self.check(None,False)

    def test_missing_malformed_and_truncated(self):
        target=self.root/'broken'
        subprocess.run([str(self.driver),str(target),'0'],check=True)
        for data in [b'',b'not a Mach-O',struct.pack('<8I',0xfeedfacf,0x100000c,0,2,0xffffffff,0,0,0)]:
            target.write_bytes(data);subprocess.run([str(self.driver),str(target),'0'],check=True)
        self.check(GOOD,True)
        data=(self.root/'fixture').read_bytes()
        for length in [32,64,len(data)-100]:
            target.write_bytes(data[:length]);subprocess.run([str(self.driver),str(target),'0'],check=True)

if __name__=='__main__':unittest.main()
