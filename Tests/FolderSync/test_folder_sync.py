"""Native coordinated I/O tests using the actual Rust save-file merge engine.
Build Tests/FolderSync/Cargo.toml, then pass its static library as argv[1].
"""
from pathlib import Path
import concurrent.futures, copy, json, os, shutil, subprocess, sys, tempfile, time, unittest, uuid
ROOT=Path(__file__).resolve().parents[2]
LIBRARY=Path(sys.argv.pop(1)).resolve()
def document(stage=0,trophy=1,nights=11):
    def campaign(s,t):return dict(checkpoint=dict(stage=s,level=0),furthest_stage=s,furthest_level=0,adventure_completed=False,trophies=[t],cleared_levels=[],aced_levels=[])
    return dict(format='RespringPeggleProgress',schema=2,scope='adventure-progress',profile_label='Local player',classic=campaign(stage,trophy),nights=campaign(nights,trophy))
class FolderSyncTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.build=tempfile.TemporaryDirectory(prefix='peggle-folder-native-');cls.driver=Path(cls.build.name)/'exchange'
        subprocess.run(['xcrun','clang','-arch','arm64','-fobjc-arc','-framework','Foundation','-framework','Security',
            '-I',str(ROOT/'Compatibility/ios'),str(ROOT/'Tests/FolderSync/native_driver.m'),str(ROOT/'Compatibility/ios/folder_sync.m'),str(ROOT/'Compatibility/ios/cloud_capability.m'),str(LIBRARY),'-o',str(cls.driver)],check=True)
    @classmethod
    def tearDownClass(cls):cls.build.cleanup()
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory(prefix='peggle-folder-case-');self.root=Path(self.tmp.name);self.folder=self.root/'shared';self.folder.mkdir();self.a=str(uuid.uuid4());self.b=str(uuid.uuid4())
    def tearDown(self):self.tmp.cleanup()
    def exchange(self,doc,writer=None,folder=None):
        token=str(uuid.uuid4());src=self.root/(token+'.input');out=self.root/(token+'.output');src.write_text(json.dumps(dict(ok=True,document=doc,profile='Local player')))
        subprocess.run([str(self.driver),str(folder or self.folder),writer or self.a,str(src),str(out)],check=True,timeout=35)
        return json.loads(out.read_text())
    def path(self,writer):return self.folder/('Peggle-Save-'+writer+'.json')
    def test_cross_installation_union_and_checkpoint(self):
        self.assertTrue(self.exchange(document(1,10))['ok']);r=self.exchange(document(3,20),self.b);self.assertTrue(r['ok']);self.assertEqual(r['document']['classic']['trophies'],[10,20]);r=self.exchange(document(1,10));self.assertEqual(r['document']['classic']['checkpoint']['stage'],3);self.assertEqual(r['document']['classic']['trophies'],[10,20]);self.assertEqual(len(list(self.folder.iterdir())),2)
    def test_offline_writers_converge(self):
        remote=self.root/'offline';remote.mkdir();self.exchange(document(1,10));self.exchange(document(4,20,13),self.b,remote)
        shutil.copy2(remote/('Peggle-Save-'+self.b+'.json'),self.folder)
        r=self.exchange(document(1,10));self.assertEqual(r['document']['classic']['trophies'],[10,20]);self.assertEqual(r['document']['nights']['checkpoint']['stage'],13)
        shutil.copy2(self.path(self.a),remote);r=self.exchange(document(4,20,13),self.b,remote);self.assertEqual(r['document']['classic']['trophies'],[10,20])
    def test_concurrent_writers_retain_both(self):
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            futures=[pool.submit(self.exchange,document(s,t),writer) for s,t,writer in [(1,10,self.a),(3,20,self.b)]]
            self.assertTrue(all(f.result()['ok'] for f in futures))
        self.assertEqual(len(list(self.folder.iterdir())),2);self.assertEqual(self.exchange(document())['document']['classic']['trophies'],[1,10,20])
    def test_unchanged_does_not_rewrite(self):
        self.exchange(document());before=self.path(self.a).stat().st_mtime_ns;self.exchange(document());self.assertEqual(before,self.path(self.a).stat().st_mtime_ns)
    def test_stale_local_never_rolls_back_own_file(self):
        self.exchange(document(4,30));r=self.exchange(document(0,1));self.assertEqual(r['document']['classic']['checkpoint']['stage'],4);self.assertEqual(r['document']['classic']['trophies'],[1,30])
    def test_names_are_not_written(self):
        self.exchange(document());self.assertEqual(json.loads(self.path(self.a).read_text())['profile_label'],'Peggle')
    def test_invalid_future_or_damaged_remote_does_not_write(self):
        self.exchange(document());before=self.path(self.a).read_bytes()
        for payload in [b'{bad',json.dumps({**document(),'schema':99}).encode(),b'[]']:
            self.path(self.b).write_bytes(payload);self.assertFalse(self.exchange(document(3,20))['ok']);self.assertEqual(before,self.path(self.a).read_bytes())
    def test_oversized_remote_does_not_write(self):
        self.path(self.b).write_bytes(b'x'*(512*1024+1));self.assertFalse(self.exchange(document())['ok']);self.assertFalse(self.path(self.a).exists())
    def test_symlink_is_rejected(self):
        other=self.root/'unrelated.json';other.write_text(json.dumps(document(4,20)));self.path(self.b).symlink_to(other);self.assertFalse(self.exchange(document())['ok']);self.assertFalse(self.path(self.a).exists())
    def test_unrelated_files_untouched(self):
        other=self.folder/'Family.json';other.write_text('unrelated');self.assertTrue(self.exchange(document())['ok']);self.assertEqual(other.read_text(),'unrelated')
    def test_invalid_writer_and_source_rejected(self):
        self.assertFalse(self.exchange(document(),'../../bad')['ok']);self.assertFalse(self.exchange({})['ok']);self.assertEqual(list(self.folder.iterdir()),[])
    def test_writer_count_boundary(self):
        for _ in range(128):self.path(str(uuid.uuid4())).write_text(json.dumps(document()))
        self.assertFalse(self.exchange(document())['ok']);self.assertEqual(len(list(self.folder.iterdir())),128)
        existing=next(self.folder.iterdir()).stem.removeprefix('Peggle-Save-')
        self.assertTrue(self.exchange(document(2,20),existing)['ok'])
    def test_missing_folder_fails(self):self.assertFalse(self.exchange(document(),folder=self.root/'missing')['ok'])
if __name__=='__main__':unittest.main()
