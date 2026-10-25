"""Recreate the sanitized nested game archive from tracked payload files."""
from pathlib import Path
import sys,json,hashlib,zipfile
root=Path(__file__).resolve().parents[1]/"Original"
pins=json.loads((root/"payload-sha256.json").read_text())
with zipfile.ZipFile(sys.argv[1],"w",zipfile.ZIP_DEFLATED,compresslevel=6) as z:
 for name,digest in sorted(pins.items()):
  data=(root/name).read_bytes()
  if hashlib.sha256(data).hexdigest()!=digest:raise SystemExit("Original payload changed: "+name)
  entry=zipfile.ZipInfo(name,(2026,1,1,0,0,0));entry.compress_type=zipfile.ZIP_DEFLATED
  entry.external_attr=0o100644<<16;z.writestr(entry,data)
