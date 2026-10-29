"""Generate the static Tauri updater manifest for a GitHub Release.

Usage:
  python scripts/make-latest-json.py --repo OWNER/innote \
    --version 0.6.2 --installer release/Innote-Setup-0.6.2.exe \
    --signature path/to/Innote_0.6.2_x64-setup.exe.sig \
    --notes "..." --output release/latest.json
"""
import argparse
import json
from datetime import datetime, timezone
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--repo", required=True, help="GitHub OWNER/REPO")
p.add_argument("--version", required=True)
p.add_argument("--installer", required=True)
p.add_argument("--signature", required=True)
p.add_argument("--notes", default="")
p.add_argument("--output", required=True)
a = p.parse_args()

installer = Path(a.installer).resolve()
signature = Path(a.signature).read_text(encoding="utf-8").strip()
version = a.version.lstrip("v")
url = f"https://github.com/{a.repo}/releases/download/v{version}/{installer.name}"
manifest = {
    "version": version,
    "notes": a.notes,
    "pub_date": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    "platforms": {
        "windows-x86_64": {
            "signature": signature,
            "url": url,
        }
    },
}
out = Path(a.output)
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(out)