"""Prepare pinned native-only preview resources. Python is a build tool only."""
from pathlib import Path
import hashlib
import json
import shutil
import urllib.request
import zipfile

ROOT=Path(__file__).resolve().parents[1]
manifest=json.loads((ROOT/'scripts/moonshine-resources.json').read_text(encoding='utf-8'))
cache=ROOT/'.tools/latency-research'
cache.mkdir(parents=True,exist_ok=True)
resources=ROOT/'src-tauri/resources'
runtime=resources/'live-runtime';runtime.mkdir(exist_ok=True)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def fetch(url,path,expected):
    path.parent.mkdir(parents=True,exist_ok=True)
    if path.exists() and sha(path)==expected:return
    partial=path.with_suffix(path.suffix+'.partial')
    with urllib.request.urlopen(url,timeout=60) as response,partial.open('wb') as output:
        shutil.copyfileobj(response,output)
    if sha(partial)!=expected:raise ValueError('Checksum mismatch: '+path.name)
    partial.replace(path)
sdk=manifest['sdk'];wheel=cache/sdk['filename'];fetch(sdk['url'],wheel,sdk['sha256'])
with zipfile.ZipFile(wheel) as archive:
    for name in ['moonshine.dll','onnxruntime.dll']:
        with archive.open('moonshine_voice/'+name) as source,(runtime/name).open('wb') as out:shutil.copyfileobj(source,out)
inventory=[]
for model in manifest['models']:
    language=model['language'];target=resources/'live-models'/language
    target.mkdir(parents=True,exist_ok=True)
    for file in model['files']:
        # Reuse the verified research download when available, otherwise download.
        candidates=list((cache/'moonshine-models').rglob(f"tiny-streaming-{language}/quantized_*/{file['name']}"))
        original=cache/'moonshine-originals'/language/file['name']
        if not original.exists() and candidates and sha(candidates[0])==file['sha256']:
            original.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(candidates[0],original)
        fetch(file['url'],original,file['sha256'])
        dest=target/file['name']
        shutil.copy2(original,dest)
        if file['name']=='streaming_config.json' and model['overrides']:
            cfg=json.loads(original.read_text(encoding='utf-8'))
            assert cfg['total_lookahead']==16
            cfg.update(model['overrides'])
            dest.write_text(json.dumps(cfg,indent=2)+'\n',encoding='utf-8')
        inventory.append({'path':f'live-models/{language}/{file["name"]}',
            'upstream_sha256':file['sha256'],'installed_sha256':sha(dest),
            'changes':model['overrides'] if file['name']=='streaming_config.json' else {}})
license_dir=resources/'licenses/moonshine';license_dir.mkdir(parents=True,exist_ok=True)
(license_dir/'resources.json').write_text(json.dumps({'source':manifest,'installed':inventory},indent=2)+'\n',encoding='utf-8')
print('Prepared verified Moonshine native DLLs and Vietnamese/English streaming models')
