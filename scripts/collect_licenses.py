"""Collect installed dependency license texts for the Windows bundle (offline)."""
from pathlib import Path
import json, shutil, subprocess

root = Path(__file__).resolve().parents[1]
dest = root / 'src-tauri/resources/licenses/dependencies'
dest.mkdir(parents=True, exist_ok=True)
data = json.loads(subprocess.check_output(['cargo', 'metadata', '--manifest-path', str(root / 'src-tauri/Cargo.toml'), '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc', '--locked', '--offline'], text=True, encoding='utf-8'))
index = []
for p in data['packages']:
    directory = Path(p['manifest_path']).parent
    prefix = f"rust-{p['name']}-{p['version']}"
    copied = []
    for file in directory.iterdir():
        if file.is_file() and file.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE')) and file.stat().st_size < 200000:
            target = dest / f'{prefix}-{file.name}'
            shutil.copy2(file, target)
            copied.append(target.name)
    index.append(dict(ecosystem='Rust', name=p['name'], version=p['version'], license=p.get('license'), repository=p.get('repository'), texts=copied))
lock = json.loads((root / 'package-lock.json').read_text(encoding='utf-8'))
for key, package in lock.get('packages', {}).items():
    if not key or package.get('dev'):
        continue
    directory = root / key
    if not directory.is_dir():
        continue
    name = key.replace('node_modules/', '').replace('/', '-')
    copied = []
    for file in directory.iterdir():
        if file.is_file() and file.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE')):
            target = dest / f'npm-{name}-{file.name}'
            shutil.copy2(file, target)
            copied.append(target.name)
    index.append(dict(ecosystem='npm', name=name, version=package.get('version'), license=package.get('license'), texts=copied))
(dest / 'index.json').write_text(json.dumps(index, ensure_ascii=False, indent=2), encoding='utf-8')
print(f'Collected notices for {len(index)} dependency records.')
