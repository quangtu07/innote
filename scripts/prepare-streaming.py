"""Fetch pinned native streaming resources. Python is only a build tool, never shipped.

Run from any directory: python scripts/prepare-streaming.py
Downloads are public models/runtime only; no meeting audio is sent anywhere.
"""
import hashlib
import json
from pathlib import Path
import shutil
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
CACHE = ROOT / ".tools" / "vosk"
CACHE.mkdir(parents=True, exist_ok=True)
RESOURCES = ROOT / "src-tauri" / "resources"
PACKAGES = [
    ("runtime.whl", "https://files.pythonhosted.org/packages/c0/4c/deb0861f7da9696f8a255f1731bb73e9412cca29c4b3888a3fcb2a930a59/vosk-0.3.45-py3-none-win_amd64.whl", "6994ddc68556c7e5730c3b6f6bad13320e3519b13ce3ed2aa25a86724e7c10ac"),
    ("model-vi.zip", "https://alphacephei.com/vosk/models/vosk-model-small-vn-0.4.zip", "efe5c8494212110471a79befc48c79da679e5b1fc52a4ffb500222ff86d622e5"),
    ("model-en.zip", "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip", "30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498"),
]
manifest = []
for name, url, expected in PACKAGES:
    cached = CACHE / name
    if not cached.exists() or hashlib.sha256(cached.read_bytes()).hexdigest() != expected:
        partial = cached.with_suffix(".partial")
        with urllib.request.urlopen(url, timeout=60) as response, partial.open("wb") as output:
            shutil.copyfileobj(response, output)
        if hashlib.sha256(partial.read_bytes()).hexdigest() != expected:
            raise ValueError(f"Checksum mismatch: {name}")
        partial.replace(cached)
    with zipfile.ZipFile(cached) as archive:
        for member in archive.infolist():
            if member.is_dir():
                continue
            if name == "runtime.whl":
                if not member.filename.startswith("vosk/") or not member.filename.endswith(".dll"):
                    continue
                target = RESOURCES / "streaming-runtime" / Path(member.filename).name
            else:
                target = RESOURCES / "streaming-models" / member.filename
            if not target.resolve().is_relative_to(RESOURCES.resolve()):
                raise ValueError("Unsafe archive path")
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(member) as source, target.open("wb") as output:
                shutil.copyfileobj(source, output)
    manifest.append({"archive": name, "url": url, "sha256": expected})
manifest_path = RESOURCES / "licenses" / "streaming" / "sources.json"
manifest_path.parent.mkdir(parents=True, exist_ok=True)
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
print("Verified streaming runtime and Vietnamese/English models")
