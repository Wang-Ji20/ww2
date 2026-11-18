"""数据源：下载到 .cache/ 并读取。

  - Natural Earth 1:10m：一级行政区、居民点、地理区域、陆地（公有领域）
  - aourednik/historical-basemaps：1914/1920/1930/1938 年世界国界（GPL-3.0）
  - geoBoundaries：部分国家的现代二级行政区，用作细分砖块（许可见 GEOBOUNDARIES）
  - OpenHistoricalMap：按日期有效的历史行政区（CC0）

用法：.venv/bin/python -I sources.py   # 下载缺失的文件；OHM 数据在首次生成时按需抓取并缓存
"""

from __future__ import annotations

import json
import re
import time
import urllib.parse
import urllib.request
from pathlib import Path

import geopandas as gpd
import shapely
from shapely.geometry import LineString

HERE = Path(__file__).resolve().parent
CACHE = HERE / ".cache"
UA = {"User-Agent": "ww2-mapgen/0.2 (non-commercial open-source game; https://github.com/ - map generator)"}

NATURAL_EARTH = {
    "ne_10m_admin_1_states_provinces.zip": "cultural/ne_10m_admin_1_states_provinces",
    "ne_10m_populated_places.zip": "cultural/ne_10m_populated_places",
    "ne_10m_geography_regions_polys.zip": "physical/ne_10m_geography_regions_polys",
    "ne_10m_land.zip": "physical/ne_10m_land",
}
HISTORICAL_YEARS = [1914, 1920, 1930, 1938]

# 只使用不带“相同方式共享”数据库条款的许可（排除 ODbL），以便随 GPL 模组再分发
GEOBOUNDARIES = {
    "CHN": "PDDL 1.0",
    "DEU": "dl-de/by-2-0",
    "CZE": "Public Domain",
    "USA": "Public Domain",
    "CAN": "Open Government Licence – Canada 2.0",
    "AUS": "CC BY 4.0",
    "BRA": "CC BY 3.0 IGO",
    "DZA": "Public Domain",
    "EGY": "CC BY 3.0 IGO",
    "SDN": "CC BY 3.0 IGO",
    "COD": "CC BY 3.0 IGO",
}
GB_RELEASE = "https://github.com/wmgeolab/geoBoundaries/raw/9469f09/releaseData/gbOpen/{iso}/ADM2/geoBoundaries-{iso}-ADM2_simplified.geojson"

OVERPASS = "https://overpass-api.openhistoricalmap.org/api/interpreter"


def _download(url: str, path: Path) -> None:
    if path.exists():
        return
    print(f"  下载 {url}", flush=True)
    req = urllib.request.Request(url, headers=UA)
    with urllib.request.urlopen(req, timeout=600) as r:
        data = r.read()
    path.write_bytes(data)


def download_all() -> None:
    CACHE.mkdir(exist_ok=True)
    for name, key in NATURAL_EARTH.items():
        _download(f"https://naciscdn.org/naturalearth/10m/{key}.zip", CACHE / name)
    for y in HISTORICAL_YEARS:
        _download(
            f"https://raw.githubusercontent.com/aourednik/historical-basemaps/master/geojson/world_{y}.geojson",
            CACHE / f"world_{y}.geojson",
        )
    for iso in GEOBOUNDARIES:
        _download(GB_RELEASE.format(iso=iso), CACHE / f"geoboundaries_{iso}_ADM2.geojson")


def historical(year: int) -> gpd.GeoDataFrame:
    h = gpd.read_file(CACHE / f"world_{year}.geojson")
    h["geometry"] = h.geometry.make_valid()
    return h[h.NAME.notna()].reset_index(drop=True)


def geoboundaries(iso: str) -> gpd.GeoDataFrame:
    g = gpd.read_file(CACHE / f"geoboundaries_{iso}_ADM2.geojson")
    g["geometry"] = g.geometry.make_valid()
    return g[["shapeName", "geometry"]].rename(columns={"shapeName": "name"})


# ───────────── OpenHistoricalMap ─────────────


def _overpass(query: str) -> list[dict]:
    body = urllib.parse.urlencode({"data": query}).encode()
    for attempt in range(4):
        try:
            req = urllib.request.Request(OVERPASS, body, headers=UA)
            with urllib.request.urlopen(req, timeout=900) as r:
                return json.loads(r.read())["elements"]
        except Exception as e:  # 服务器繁忙时退避重试
            if attempt == 3:
                raise
            print(f"  Overpass 失败（{e}），{20 * (attempt + 1)} 秒后重试", flush=True)
            time.sleep(20 * (attempt + 1))
    return []


def _parse_date(s: str | None) -> tuple[int, int, int] | None:
    m = re.match(r"^(-?\d{1,4})(?:-(\d{2}))?(?:-(\d{2}))?", s or "")
    return (int(m[1]), int(m[2] or 1), int(m[3] or 1)) if m else None


def alive(tags: dict, date: tuple[int, int, int]) -> bool:
    start, end = _parse_date(tags.get("start_date")), _parse_date(tags.get("end_date"))
    return start is not None and start <= date and (end is None or end > date)


def _polygon(rel: dict):
    lines = [
        LineString([(p["lon"], p["lat"]) for p in m["geometry"]])
        for m in rel.get("members", [])
        if m["type"] == "way" and m.get("role") in ("outer", "") and len(m.get("geometry") or []) >= 2
    ]
    polys = list(shapely.polygonize(lines).geoms)
    if not polys:
        return None
    # polygonize 也会把内环（飞地的洞）围成面：减去 inner 环
    inner = [
        LineString([(p["lon"], p["lat"]) for p in m["geometry"]])
        for m in rel.get("members", [])
        if m["type"] == "way" and m.get("role") == "inner" and len(m.get("geometry") or []) >= 2
    ]
    g = shapely.union_all(polys)
    holes = list(shapely.polygonize(inner).geoms)
    return shapely.make_valid(g.difference(shapely.union_all(holes)) if holes else g)


def ohm_scope(scope: str, levels: list[int], names: list[str], date: tuple[int, int, int]) -> gpd.GeoDataFrame:
    """取某个 1936 年国家（OHM 中的 admin_level=2 名称）范围内、指定级别的全部有效行政区。

    结果缓存到 .cache/ohm/<scope>.geojson；删除缓存文件即可重新抓取。
    """
    out = CACHE / "ohm" / f"{re.sub(r'[^A-Za-z0-9]+', '_', scope)}_{'-'.join(map(str, levels))}.geojson"
    if out.exists():
        return gpd.read_file(out)
    out.parent.mkdir(parents=True, exist_ok=True)
    print(f"  抓取 OHM：{scope} 级别 {levels}", flush=True)

    tops = [e for e in _overpass(f'[out:json][timeout:600];rel["boundary"="administrative"]["admin_level"="2"]["name:en"="{scope}"];out geom;')
            if alive(e["tags"], date)]
    if not tops:
        tops = [e for e in _overpass(f'[out:json][timeout:600];rel["boundary"="administrative"]["admin_level"="2"]["name"="{scope}"];out geom;')
                if alive(e["tags"], date)]
    assert tops, f"OHM 中找不到 {date} 有效的国家 {scope}"
    country = shapely.union_all([g for g in map(_polygon, tops) if g is not None])

    rows = []
    if 2 in levels:
        rows.append(dict(level=2, name=scope, name_en=scope, name_zh=None, license=tops[0]["tags"].get("license", "CC0"),
                         geometry=country))
    finer = [lv for lv in levels if lv > 2]
    if finer:
        minx, miny, maxx, maxy = country.bounds
        lv = "|".join(map(str, finer))
        tagged = _overpass(
            f'[out:json][timeout:600];rel["boundary"="administrative"]["admin_level"~"^({lv})$"]'
            f"({miny},{minx},{maxy},{maxx});out tags center;"
        )
        prepared = shapely.prepared.prep(country)
        ids = [
            e["id"] for e in tagged
            if alive(e["tags"], date) and "center" in e
            and prepared.contains(shapely.Point(e["center"]["lon"], e["center"]["lat"]))
            and (not names or (e["tags"].get("name:en") or e["tags"].get("name")) in names
                 or e["tags"].get("name") in names)
        ]
        for k in range(0, len(ids), 150):
            chunk = ",".join(map(str, ids[k:k + 150]))
            for e in _overpass(f"[out:json][timeout:600];rel(id:{chunk});out geom;"):
                g = _polygon(e)
                if g is None or g.is_empty:
                    continue
                t = e["tags"]
                rows.append(dict(
                    level=int(t["admin_level"]), name=t.get("name"), name_en=t.get("name:en") or t.get("name"),
                    name_zh=t.get("name:zh"), license=t.get("license", "CC0"), geometry=g,
                ))
    gdf = gpd.GeoDataFrame(rows, crs="EPSG:4326")
    gdf.to_file(out, driver="GeoJSON")
    return gdf


if __name__ == "__main__":
    download_all()
