"""Point the updater at a public GitHub repository.

Usage:
  python scripts/configure-github.py --repo OWNER/innote
"""
import argparse
import json
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--repo", required=True, help="GitHub OWNER/REPO")
a = p.parse_args()
repo = a.repo.strip().strip("/")
if repo.count("/") != 1:
    raise SystemExit("--repo must look like OWNER/innote")

path = Path(__file__).resolve().parents[1] / "src-tauri" / "tauri.conf.json"
config = json.loads(path.read_text(encoding="utf-8"))
config.setdefault("plugins", {}).setdefault("updater", {})["endpoints"] = [
    f"https://github.com/{repo}/releases/latest/download/latest.json"
]
path.write_text(json.dumps(config, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(f"Updater endpoint: {config['plugins']['updater']['endpoints'][0]}")