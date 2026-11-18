"""抽查 OpenHistoricalMap 边界关系能否拼成有效多边形，并与已知面积比对。

用法：../tools/mapgen/.venv/bin/python -I scripts/ohm_geometry_check.py
"""

from __future__ import annotations

import json
import urllib.parse
import urllib.request

import geopandas as gpd
import shapely
from shapely.geometry import LineString

OVERPASS = "https://overpass-api.openhistoricalmap.org/api/interpreter"
UA = {"User-Agent": "ww2-mapgen-research/0.1 (non-commercial game map research)"}

# (名称, 1936 年面积 km²，参考值来自维基百科条目)
SAMPLES = [
    ("Free City of Danzig", 1966),
    ("Memel", None),
    ("Saarland", 1912),
    ("County of London", 303),
    ("Province of East Prussia", 36996),
    ("Bezirk Reichenberg", None),
    ("Wilno Voivodeship", 29011),
    ("Subcarpathian Rus'", 12639),
]


def fetch(name: str) -> list[dict]:
    q = f'[out:json][timeout:120];rel["boundary"="administrative"]["name:en"="{name}"];out geom;'
    q2 = f'[out:json][timeout:120];rel["boundary"="administrative"]["name"="{name}"];out geom;'
    for query in (q, q2):
        req = urllib.request.Request(OVERPASS, urllib.parse.urlencode({"data": query}).encode(), headers=UA)
        with urllib.request.urlopen(req, timeout=180) as r:
            els = json.loads(r.read())["elements"]
        if els:
            return els
    return []


def polygon_of(rel: dict):
    lines = [
        LineString([(p["lon"], p["lat"]) for p in m["geometry"]])
        for m in rel.get("members", [])
        if m["type"] == "way" and m.get("role") in ("outer", "") and len(m.get("geometry") or []) >= 2
    ]
    polys = list(shapely.polygonize(lines).geoms)
    return shapely.union_all(polys) if polys else None


for name, expected in SAMPLES:
    for rel in fetch(name):
        t = rel["tags"]
        g = polygon_of(rel)
        km2 = gpd.GeoSeries([g], crs=4326).to_crs("EPSG:6933").area.iloc[0] / 1e6 if g is not None else 0
        ref = f"（参考 {expected}）" if expected else ""
        print(f"{name:28s} {t.get('start_date', '?'):>10s}–{t.get('end_date', ''):<10s} "
              f"lvl={t.get('admin_level')} 面积 {km2:9.0f} km²{ref} valid={g is not None and g.is_valid}")
