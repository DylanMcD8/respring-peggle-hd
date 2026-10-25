from pathlib import Path
import sys,shutil
p=Path(sys.argv[1])
assert p.name in ("Peggle.app","PeggleHD.app") and p.parent.name in ("iphoneos","iphonesimulator") and not p.is_symlink()
if p.exists():shutil.rmtree(p)
p.mkdir(parents=True)
