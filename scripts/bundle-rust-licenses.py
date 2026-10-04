#!/usr/bin/env python3
"""Copy original license and notice texts from the locked Rust dependency sources into APK assets."""
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--locked", "--offline", "--format-version", "1"
], cwd=root))
output = root / "android/app/build/licenseAssets/licenses/rust"
output.mkdir(parents=True, exist_ok=True)
inventory = []
for package in metadata["packages"]:
    if package["name"] == "polotno-core":
        continue
    source = Path(package["manifest_path"]).parent
    entry = {key: package.get(key) for key in ("name", "version", "license", "repository", "authors")}
    dest = output / f'{package["name"]}-{package["version"]}'
    dest.mkdir(exist_ok=True)
    copied = []
    for path in source.rglob("*"):
        if path.is_file() and (path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE")) or path.name.upper() == "UNLICENSE"):
            relative = path.relative_to(source)
            target = dest / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(path.read_bytes())
            copied.append(str(relative))
    if not copied and "Apache-2.0" in (package.get("license") or ""):
        # A few published macro/target crates omit a standalone license file.
        # Their SPDX expression permits Apache-2.0; include its full text and attribution.
        target = dest / "LICENSE-APACHE"
        target.write_bytes((root / "android/app/src/main/assets/licenses/Apache-2.0.txt").read_bytes())
        (dest / "ATTRIBUTION.json").write_text(json.dumps(entry, indent=2) + "\n")
        copied.extend(["LICENSE-APACHE", "ATTRIBUTION.json"])
    entry["files"] = copied
    if not copied and package["name"] != "libsqlite3-sys":
        raise SystemExit(f'Missing license text for {package["name"]}')
    inventory.append(entry)
(output / "inventory.json").write_text(json.dumps(inventory, ensure_ascii=False, indent=2) + "\n")
