"""Prepare pinned NeMo native DLLs/notices; never downloads meeting audio."""
from pathlib import Path
import hashlib,json,shutil,urllib.request,zipfile,runpy
root=Path(__file__).resolve().parents[1];m=json.loads((root/'scripts/nemo-resources.json').read_text());sdk=m['sdk'];cache=root/'.tools/latency-research';cache.mkdir(parents=True,exist_ok=True);archive=cache/sdk['filename']
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
if not archive.exists() or sha(archive)!=sdk['sha256']:
 partial=archive.with_suffix('.partial')
 with urllib.request.urlopen(sdk['url'],timeout=60) as r,partial.open('wb') as f:shutil.copyfileobj(r,f)
 assert sha(partial)==sdk['sha256'],'SDK checksum mismatch';partial.replace(archive)
resources=root/'src-tauri/resources';native=resources/'nemo-runtime';native.mkdir(exist_ok=True);licenses=resources/'licenses/nemo';licenses.mkdir(exist_ok=True)
with zipfile.ZipFile(archive) as z:
 for name in z.namelist():
  if name.endswith('/'):continue
  target=None
  if '/bin/' in name and Path(name).name in sdk['dlls']:target=native/Path(name).name
  elif '/share/licenses/nemo-speech/' in name:target=licenses/name.split('/share/licenses/nemo-speech/',1)[1]
  if target:
   assert target.resolve().is_relative_to(resources.resolve());target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(z.read(name))
for name in sdk['dlls']:assert (native/name).is_file(),name
convert=runpy.run_path(str(root/'scripts/nemo-vad.py'))['convert']
assert convert(resources/'ggml-silero-v5.1.2.bin',native/'silero-v5.1.2.gguf') == '5e2b8adfe52d8a6e89e47a3cd5075915b10c771a5c6654f05a5b69d89c1c26fb'
(licenses/'resources.json').write_text(json.dumps(m,indent=2)+'\n',encoding='utf-8');print('Prepared verified NeMo runtime; weights are downloaded separately by the app')
