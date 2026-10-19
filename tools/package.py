"""打包本项目已验证 NRO；默认 INI 内嵌，首次运行在存档根目录生成。"""
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile
import hashlib
import json
import re

ROOT = Path(__file__).resolve().parent.parent
name = re.search(r'^name = "([^"]+)"', (ROOT / "Cargo.toml").read_text(encoding="utf-8"), re.M)[1]
plugin_id = name.replace("-", "_").removeprefix("mhgu_")
nro = ROOT / "dist" / (plugin_id + ".nro")
verified = json.loads((ROOT / "target/build-verification.json").read_text(encoding="utf-8"))
assert hashlib.sha256(nro.read_bytes()).hexdigest() == verified["sha256"], "NRO changed; run bash test.sh"
entries = {f"romfs/nativeNX/plugins/{plugin_id}.nro": nro, "README.md": ROOT / "README.md"}
if plugin_id == "target_health_display":
    entries.update({
        "romfs/nativeNX/forge/fonts/NotoSansSC-Medium.otf": ROOT / "assets/romfs/nativeNX/forge/fonts/NotoSansSC-Medium.otf",
        "licenses/NotoSansSC-LICENSE.txt": ROOT / "assets/licenses/NotoSansSC-LICENSE.txt",
        "docs/target-health-font-source.json": ROOT / "assets/font-source.json",
    })
target = nro.with_suffix(".zip")
with ZipFile(target, "w", ZIP_DEFLATED) as archive:
    for entry, source in entries.items():
        archive.write(source, entry)
with ZipFile(target) as archive:
    assert archive.testzip() is None
    assert set(archive.namelist()) == set(entries)
    for entry, source in entries.items():
        assert archive.read(entry) == source.read_bytes()
report = {"zip": str(target), "bytes": target.stat().st_size, "sha256": hashlib.sha256(target.read_bytes()).hexdigest(), "entries": list(entries)}
(ROOT / "target/package-verification.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, ensure_ascii=False, indent=2))
