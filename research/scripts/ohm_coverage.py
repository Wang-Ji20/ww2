"""OpenHistoricalMap 行政区覆盖度普查：在给定日期仍有效的行政区，按现代国家与行政级别统计。

用法（借用 mapgen 的 Python 环境与 Natural Earth 缓存）：
  ../tools/mapgen/.venv/bin/python -I scripts/ohm_coverage.py [--date 1936-01-01] [--refresh]
输出：data/ohm_tags.json（原始缓存，已忽略）与 notes/ohm-coverage.md
"""

from __future__ import annotations

import argparse
import json
import re
import urllib.parse
import urllib.request
from collections import Counter, defaultdict
from pathlib import Path

import geopandas as gpd
from shapely.geometry import Point

HERE = Path(__file__).resolve().parent.parent
CACHE = HERE / "data"
NE_ADMIN1 = HERE.parent / "tools/mapgen/.cache/ne_10m_admin_1_states_provinces.zip"
OVERPASS = "https://overpass-api.openhistoricalmap.org/api/interpreter"
QUERY = '[out:json][timeout:600];rel["boundary"="administrative"]["admin_level"~"^[2-6]$"];out tags center;'


def fetch(refresh: bool) -> list[dict]:
    path = CACHE / "ohm_tags.json"
    if refresh or not path.exists():
        CACHE.mkdir(exist_ok=True)
        body = urllib.parse.urlencode({"data": QUERY}).encode()
        req = urllib.request.Request(OVERPASS, body, headers={"User-Agent": "ww2-mapgen-research/0.1 (non-commercial game map research)"})
        with urllib.request.urlopen(req, timeout=900) as r:
            path.write_bytes(r.read())
    return json.loads(path.read_text())["elements"]


def parse_date(s: str | None) -> tuple[int, int, int] | None:
    """OHM 日期为 YYYY、YYYY-MM 或 YYYY-MM-DD；缺失的月日按最早处理"""
    m = re.match(r"^(-?\d{1,4})(?:-(\d{2}))?(?:-(\d{2}))?", s or "")
    if not m:
        return None
    return int(m[1]), int(m[2] or 1), int(m[3] or 1)


def alive(tags: dict, date: tuple[int, int, int]) -> bool:
    start, end = parse_date(tags.get("start_date")), parse_date(tags.get("end_date"))
    return start is not None and start <= date and (end is None or end > date)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--date", default="1936-01-01")
    ap.add_argument("--refresh", action="store_true")
    args = ap.parse_args()
    date = parse_date(args.date)

    els = [e for e in fetch(args.refresh) if "center" in e and alive(e["tags"], date)]
    adm = gpd.read_file(NE_ADMIN1, columns=["adm0_a3", "admin"])
    pts = gpd.GeoDataFrame(
        {"i": range(len(els))},
        geometry=[Point(e["center"]["lon"], e["center"]["lat"]) for e in els],
        crs="EPSG:4326",
    )
    joined = gpd.sjoin(pts, adm, predicate="within").drop_duplicates("i").set_index("i")

    by_country: dict[str, Counter] = defaultdict(Counter)
    names: dict[str, list[str]] = defaultdict(list)
    licenses = Counter()
    for i, e in enumerate(els):
        t = e["tags"]
        country = joined.admin.get(i, "(海上/未定位)")
        level = t.get("admin_level")
        by_country[country][level] += 1
        licenses[t.get("license", "(未标注，默认 CC0)")] += 1
        if level in ("4", "5"):
            names[country].append(t.get("name:en") or t.get("name", "?"))

    lines = [
        f"# OpenHistoricalMap 行政区覆盖度（{args.date}）",
        "",
        f"由 `research/scripts/ohm_coverage.py` 生成。统计 admin_level 2–6、在 {args.date} 有效的边界关系，",
        "按关系中心点落入的**现代**国家归类（只用于粗略定位）。",
        "",
        f"有效关系共 {len(els)} 个。许可证：" + "；".join(f"{k} × {v}" for k, v in licenses.most_common()),
        "",
        "| 现代国家 | L2 | L3 | L4 | L5 | L6 | L4/L5 示例 |",
        "| :--- | ---: | ---: | ---: | ---: | ---: | :--- |",
    ]
    for country in sorted(by_country, key=lambda c: -sum(by_country[c].values())):
        c = by_country[country]
        sample = "、".join(sorted(set(names[country]))[:12])
        lines.append(f"| {country} | {c['2']} | {c['3']} | {c['4']} | {c['5']} | {c['6']} | {sample} |")
    out = HERE / "notes/ohm-coverage.md"
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{len(els)} 个有效关系，{len(by_country)} 个国家 → {out}")


if __name__ == "__main__":
    main()
