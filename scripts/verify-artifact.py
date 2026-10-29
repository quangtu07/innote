"""Verify extracted NSIS and portable files against the configured resources.
Does not install, execute, or change the payload. Writes a checksum report.
"""
import argparse,hashlib,json,tomllib
from pathlib import Path

p=argparse.ArgumentParser()
p.add_argument('--payload',required=True);p.add_argument('--portable',required=True)
p.add_argument('--installer',required=True);p.add_argument('--output',required=True)
a=p.parse_args();root=Path(__file__).resolve().parents[1];base=root/'src-tauri'
config=json.loads((base/'tauri.conf.json').read_text(encoding='utf-8'))
version=config['version']
assert json.loads((root/'package.json').read_text(encoding='utf-8'))['version']==version
assert tomllib.loads((base/'Cargo.toml').read_text(encoding='utf-8'))['package']['version']==version
payload=Path(a.payload).resolve();portable=Path(a.portable).resolve()
digest=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
verified=[]
for source,target in config['bundle']['resources'].items():
    source=(base/source).resolve()
    pairs=[(f,Path(target)/f.relative_to(source)) for f in source.rglob('*') if f.is_file()] if source.is_dir() else [(source,Path(target))]
    for f,target in pairs:
        expected=digest(f)
        for folder in [payload,portable]:
            actual=folder/target
            assert actual.is_file(),str(actual)
            assert digest(actual)==expected,str(actual)
        verified.append({'path':str(target),'bytes':f.stat().st_size,'sha256':expected})
exe=base/'target/release/innote.exe';sha=digest(exe)
for folder in [payload,portable]:assert digest(folder/exe.name)==sha,str(folder/exe.name)
manifest=json.loads((root/'scripts/nemo-resources.json').read_text(encoding='utf-8'))
assert digest(payload/'nemo-runtime'/manifest['vad']['filename'])==manifest['vad']['sha256']
for folder in [payload,portable]:
    assert not list(folder.rglob('nemotron*.gguf')),'Large Nemotron weights must be a separate download'
installer=Path(a.installer).resolve()
report={'version':version,'mapped_resources_verified':len(verified),'binary_identical':True,
        'nemotron_weights_not_bundled':True,'resources':verified,
        'artifacts':{str(path):{'bytes':path.stat().st_size,'sha256':digest(path)}
                     for path in [installer,portable/exe.name]}}
Path(a.output).write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps({k:v for k,v in report.items() if k!='resources'},ensure_ascii=False))
