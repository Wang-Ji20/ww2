"""生成 world-1936 模组：约 1000 个省份的 1936 年世界地图。

设计依据见 research/notes/partition-principles.md。数据源见 sources.py（先运行 sources.py 下载）。

流程：
  1. 砖块：Natural Earth 现代一级行政区为底；部分国家换成 geoBoundaries 二级行政区（中国为县，按 china_1936.csv 归入 1936 年的省）；
     ohm_layers.csv 中的国家换成 OpenHistoricalMap 1936-01-01 有效的行政区（普鲁士各省、苏台德各区、波兰各省……）
  2. 归属：1938 年国界按 owners_1936.csv 回溯到 1936 年；开局占领区；必须独立的地区（regions_1936.csv）
  3. 合并：同归属、同控制者、同地区的相邻砖块自小到大合并；1936 年的大行政单位是硬边界；小于最小面积的必须合并；
     过大的省份沿砖块边界二分，单一砖块才退回 Voronoi 切分
  4. 命名（1936 年地名）、地形、胜利点、工业、基建、邻接、海峡与航线、开局部署；验收检查后输出

用法：.venv/bin/python -I mapgen.py [--target 950] [--out ../../web/public/mods/world-1936]
"""

from __future__ import annotations

import argparse
import csv
import sys
import heapq
import json
import math
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path

import geopandas as gpd
import numpy as np
import pandas as pd
import shapely
import topojson
from shapely.geometry import MultiPolygon, Point, Polygon
from shapely.ops import nearest_points

# -I（隔离模式）不会把脚本所在目录加入 sys.path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import sources  # noqa: E402
from layers import Layers  # noqa: E402

HERE = Path(__file__).resolve().parent
CACHE = sources.CACHE
DATA = HERE / "data"

DATE = (1936, 1, 1)
EQUAL_AREA = "EPSG:6933"
SEED = 1936

# 合并/切分阈值（相对平均权重 W）
MERGE_BELOW = 0.55
MERGE_CAP = 1.9
SPLIT_ABOVE = 1.8
MAX_SPLIT = 14
# 欧洲省份的最小面积（km²）；其他地区按 region_density 放大
MIN_AREA_EUROPE = 4000
# 大于此面积（km²）的 OHM 单位再按被替换的现代砖块细分（软边界），以便沿真实界线切分
SUBDIVIDE_KM2 = 8000
# 细分出的切块小于此面积（km²）则并回最大的一块
SUBDIVIDE_MIN_KM2 = 250
# 较大的细分砖块按面积切分归属，小于此面积的整块归属多数方（避免国界数据错位产生碎屑）
SLIVER_BRICK_KM2 = 20000
# 孤岛并入同国省份的最大跨海距离（公里）
ISLAND_MERGE_KM = 600
# 小于该面积（km²）且无法并入同国省份的孤岛直接舍弃
DROP_ISLAND_KM2 = 3000
# 自动海峡连接的最大跨海距离（公里）
AUTO_STRAIT_KM = 300
# 输出轮廓的简化容差（度）
SIMPLIFY_DEG = 0.03

# OHM 行政区名称前后缀：命名时去掉
UNIT_AFFIXES = [
    "Politický okres ", "Regierungsbezirk ", "Statutarstadt ", "Statutární město ", "Stadtkreis ", "Landkreis ",
    "Bezirk ", "Kreis ", "Provinz ", "Province of ", "Freistaat ", "Free State of ", "Województwo ", "Wojewodztwo ",
    "województwo ", "Land ", " Amt", " County", " Voivodeship",
]

# 热带雨林大致范围（经度区间，纬度区间）
JUNGLE_BOXES = [
    ((-80, -44), (-15, 9)),  # 亚马孙
    ((8, 32), (-8, 6)),  # 刚果盆地
    ((95, 155), (-10, 20)),  # 东南亚与新几内亚
    ((-92, -76), (7, 18)),  # 中美洲
]


def region_density(lon: float, lat: float) -> float:
    """每平方公里的权重系数：欧洲最细，西伯利亚、撒哈拉、北美内陆最粗"""
    if lat > 66:
        return 0.04
    if 87 <= lon <= 119 and lat > 41.5:
        return 0.05  # 蒙古高原
    if 78 <= lon <= 103 and 28 <= lat <= 40:
        return 0.05  # 青藏高原
    if -25 <= lon <= 30 and 35 <= lat <= 72:
        return 1.0  # 欧洲
    if 30 < lon <= 60 and 44 <= lat <= 66:
        return 0.42  # 苏联欧洲部分
    if 25 <= lon <= 63 and 12 <= lat < 44:
        return 0.2  # 安纳托利亚、高加索、中东
    if -18 <= lon <= 35 and 15 <= lat < 37:
        return 0.09  # 北非
    if -20 <= lon <= 55 and lat < 15:
        return 0.075  # 撒哈拉以南非洲
    if 60 <= lon <= 92 and 5 <= lat <= 37:
        return 0.22  # 南亚
    if 92 < lon <= 146 and 18 <= lat <= 54:
        return 0.3  # 中国、朝鲜、日本
    if 92 < lon <= 160 and -11 <= lat < 18:
        return 0.2  # 东南亚
    if 46 < lon <= 180 and 37 <= lat <= 66:
        return 0.05  # 西伯利亚、中亚
    if lon < -50 and lat >= 15:
        return 0.1  # 北美
    if -120 <= lon <= -30 and lat < 15:
        return 0.06  # 拉丁美洲
    return 0.05  # 大洋洲及其他


def min_area(lon: float, lat: float) -> float:
    return MIN_AREA_EUROPE / math.sqrt(region_density(lon, lat))


def city_weight(pop: float) -> float:
    """城市人口对省份权重的加成（等价于欧洲的平方公里数），开方以压低超大城市"""
    return 6000.0 * math.sqrt(max(pop, 0) / 1e6)


def city_vp(pop: float) -> int:
    if pop >= 8e6:
        return 8
    if pop >= 3e6:
        return 5
    if pop >= 1e6:
        return 3
    if pop >= 3e5:
        return 1
    return 0


def read_csv(name: str) -> list[dict]:
    with open(DATA / name, encoding="utf-8") as f:
        lines = [line for line in f if not line.startswith("#")]
    return list(csv.DictReader(lines))


def log(msg: str) -> None:
    print(msg, flush=True)


def km2(geoms) -> np.ndarray:
    return gpd.GeoSeries(list(geoms), crs="EPSG:4326").to_crs(EQUAL_AREA).area.to_numpy() / 1e6


def polygonal(g):
    if isinstance(g, (Polygon, MultiPolygon)):
        return g
    polys = [p for p in getattr(g, "geoms", []) if isinstance(p, (Polygon, MultiPolygon))]
    return shapely.union_all(polys) if polys else Polygon()


def haversine_km(a: tuple[float, float], b: tuple[float, float]) -> float:
    (lon1, lat1), (lon2, lat2) = a, b
    p1, p2 = math.radians(lat1), math.radians(lat2)
    h = math.sin((p2 - p1) / 2) ** 2 + math.cos(p1) * math.cos(p2) * math.sin(math.radians(lon2 - lon1) / 2) ** 2
    return 2 * 6371 * math.asin(min(1, math.sqrt(h)))


def unit_label(name: str | None) -> str | None:
    if not name:
        return name
    for a in UNIT_AFFIXES:
        name = name.removeprefix(a) if not a.startswith(" ") else name.removesuffix(a)
    return name.strip()


# ───────────── 1. 砖块 ─────────────


def load_admin1() -> gpd.GeoDataFrame:
    cols = ["adm1_code", "name", "name_zh", "name_en", "name_alt", "gn_name", "woe_name", "adm0_a3", "region", "iso_3166_2"]
    a = gpd.read_file(CACHE / "ne_10m_admin_1_states_provinces.zip", columns=cols)
    a = a[a.adm0_a3 != "ATA"].copy()
    a["geometry"] = a.geometry.make_valid().map(polygonal)
    a = a[a.geometry.bounds.maxy > -60]
    a["name_zh"] = a.name_zh.fillna(a.name)
    a["name_en"] = a.name_en.fillna(a.name)
    return a.reset_index(drop=True)


def ne_bricks(adm1: gpd.GeoDataFrame) -> gpd.GeoDataFrame:
    return gpd.GeoDataFrame(dict(
        src="ne", adm0=adm1.adm0_a3, u1=adm1.adm1_code, u2=None, u3=None, cn=None,
        name_zh=adm1.name_zh, name_en=adm1.name_en, geometry=adm1.geometry,
    ), crs="EPSG:4326")


def replace_country(bricks: gpd.GeoDataFrame, iso: str, finer: gpd.GeoDataFrame) -> gpd.GeoDataFrame:
    """把某国的 Natural Earth 砖块换成更细的行政区（裁切到原轮廓），原一级行政区作为软上级"""
    mask = (bricks.adm0 == iso) & (bricks.src == "ne")
    base = bricks[mask].reset_index(drop=True)
    footprint = shapely.union_all(base.geometry.values)
    clipped = finer.copy()
    clipped["geometry"] = clipped.geometry.intersection(footprint).map(polygonal)
    clipped = clipped[~clipped.geometry.is_empty]
    # 原轮廓中细分数据没有覆盖的部分作为填充砖块
    rest = footprint.difference(shapely.union_all(clipped.geometry.values))
    filler = [p for p in getattr(polygonal(rest), "geoms", [polygonal(rest)]) if not p.is_empty]
    # 海岸线概括程度不同留下的碎屑不作为砖块
    filler = [p for p, a in zip(filler, km2(filler) if filler else []) if a > 20]
    rows = gpd.GeoDataFrame(
        {"name": list(clipped.name) + [None] * len(filler)},
        geometry=list(clipped.geometry) + filler, crs="EPSG:4326",
    )
    parent = gpd.sjoin(
        gpd.GeoDataFrame(geometry=rows.geometry.representative_point(), crs="EPSG:4326"),
        base[["u1", "name_zh", "name_en", "geometry"]], predicate="within",
    ).groupby(level=0).first()
    out = gpd.GeoDataFrame(dict(
        src="gb", adm0=iso,
        u1=parent.u1.reindex(rows.index).to_numpy(), u2=rows.name, u3=None, cn=None,
        name_zh=parent.name_zh.reindex(rows.index).to_numpy(), name_en=parent.name_en.reindex(rows.index).to_numpy(),
        geometry=rows.geometry,
    ), crs="EPSG:4326")
    out = out[out.u1.notna() | (km2(out.geometry) > 50)]
    return pd.concat([bricks[~mask], out], ignore_index=True)


def assign_china(bricks: gpd.GeoDataFrame, ne_names: dict[str, str]) -> None:
    """中国各县按 china_1936.csv 归入 1936 年的省：u1 为省（硬边界），cn 为规则编号"""
    rules = read_csv("china_1936.csv")
    idx = bricks.index[bricks.adm0 == "CHN"]
    reps = bricks.loc[idx].geometry.representative_point()
    modern = bricks.loc[idx, "u1"].map(ne_names)
    taken = pd.Series(False, index=idx)
    for r in rules:
        m = ~taken
        if r["modern"]:
            m &= modern.isin(r["modern"].split("|"))
        if r["poly"]:
            polys = [Polygon([tuple(map(float, p.split())) for p in ring.split(";")]) for ring in r["poly"].split("|")]
            area = shapely.union_all(polys)
            m &= reps.within(area)
        if r["pts"]:
            pts = [Point(*map(float, p.split())) for p in r["pts"].split(";")]
            geoms = bricks.loc[idx].geometry
            m &= geoms.map(lambda g: any(g.contains(p) for p in pts))
        hit = m[m].index
        bricks.loc[hit, ["u1", "cn", "name_zh", "name_en"]] = [f"cn:{r['id']}", r["id"], r["name"], r["name_en"]]
        taken[hit] = True
    left = taken[~taken].index
    if len(left):
        log(f"  ⚠ {len(left)} 个中国县未匹配 china_1936.csv 的任何规则："
            f"{[(bricks.u2[i], bricks.u1[i], round(reps[i].x, 2), round(reps[i].y, 2)) for i in left]}")


def ohm_units(scopes: list[dict]) -> gpd.GeoDataFrame:
    frames = []
    for s in scopes:
        levels = [int(x) for x in s["levels"].split("|")]
        names = [n for n in s["names"].split("|") if n]
        g = sources.ohm_scope(s["scope"], levels, names, DATE)
        exclude = {n for n in s["exclude"].split("|") if n}
        g = g[~g.name.isin(exclude) & ~g.name_en.isin(exclude)].copy()
        g["scope"] = s["scope"]
        g["geometry"] = g.geometry.make_valid().map(polygonal)
        frames.append(g)
    return gpd.GeoDataFrame(pd.concat(frames, ignore_index=True), crs="EPSG:4326")


def subdivide(g, replaced: gpd.GeoDataFrame) -> list[tuple[str | None, object]]:
    """大的 OHM 单位按被替换的现代砖块切开；过小的切块并回最大的一块"""
    if km2([g])[0] < SUBDIVIDE_KM2:
        return [(None, g)]
    idx = replaced.sindex.query(g, predicate="intersects")
    parts = [(replaced.name_en.iloc[i], polygonal(g.intersection(replaced.geometry.iloc[i]))) for i in idx]
    parts = [(n, p) for n, p in parts if not p.is_empty]
    if len(parts) <= 1:
        return [(None, g)]
    areas = km2([p for _, p in parts])
    big = [(n, p) for (n, p), a in zip(parts, areas) if a >= SUBDIVIDE_MIN_KM2]
    if len(big) <= 1:
        return [(None, g)]
    rest = [p for (_, p), a in zip(parts, areas) if a < SUBDIVIDE_MIN_KM2] + [polygonal(g.difference(shapely.union_all([p for _, p in parts])))]
    k = max(range(len(big)), key=lambda i: big[i][1].area)
    big[k] = (big[k][0], polygonal(shapely.union_all([big[k][1], *rest])))
    return big


def apply_ohm(bricks: gpd.GeoDataFrame, units: gpd.GeoDataFrame, scopes: list[dict]) -> gpd.GeoDataFrame:
    """把各 OHM 范围内的砖块换成 1936 年的行政区：细级别优先，粗级别补其余部分；只保留陆地"""
    for s in scopes:
        su = units[units.scope == s["scope"]].reset_index(drop=True)
        if su.empty:
            continue
        top = su.level.min()
        footprint = shapely.union_all(su[su.level == top].geometry.values)
        hit = bricks.sindex.query(footprint, predicate="intersects")
        replaced = bricks.iloc[hit][["name_en", "geometry"]].reset_index(drop=True)
        land = shapely.union_all(bricks.geometry.iloc[hit].values).intersection(footprint)
        # 原砖块挖去该范围
        cut = bricks.geometry.iloc[hit].difference(footprint).map(polygonal)
        bricks.loc[bricks.index[hit], "geometry"] = cut.to_numpy()
        bricks = bricks[~bricks.geometry.is_empty & (bricks.geometry.area > 1e-7)]

        parents = {lv: su[su.level == lv] for lv in sorted(su.level.unique())}

        def parent_name(pt, lv) -> str | None:
            p = parents.get(lv)
            if p is None:
                return None
            hits = p[p.geometry.contains(pt)]
            return hits.name_en.iloc[0] if len(hits) else None

        taken = None
        rows = []
        for lv in sorted(su.level.unique(), reverse=True):
            for u in su[su.level == lv].itertuples():
                g = u.geometry.intersection(land)
                if taken is not None:
                    g = g.difference(taken)
                g = polygonal(g)
                if g.is_empty or g.area < 1e-6:
                    continue
                taken = g if taken is None else shapely.union(taken, g)
                pt = g.representative_point()
                path = [parent_name(pt, l) for l in sorted(parents)]
                path = [p for p in path if p] or [u.name_en]
                path += [None] * (3 - len(path))
                row = dict(
                    src="ohm", adm0=None, u1=f"ohm:{path[0]}", u2=path[1], u3=path[2], cn=None,
                    name_zh=u.name_zh if isinstance(u.name_zh, str) else unit_label(u.name),
                    name_en=unit_label(u.name_en), geometry=g,
                )
                rows += [dict(row, u3=row["u3"] or sub, geometry=part) for sub, part in subdivide(g, replaced)]
        # 范围内未被任何单位覆盖的陆地（数据缺口）保留为填充砖块
        rest = polygonal(land.difference(taken)) if taken is not None else land
        for p in getattr(rest, "geoms", [rest]):
            if not p.is_empty and p.area > 1e-4:
                rows.append(dict(src="ohm", adm0=None, u1=None, u2=None, u3=None, cn=None,
                                 name_zh=None, name_en=None, geometry=p))
        bricks = pd.concat([bricks, gpd.GeoDataFrame(rows, crs="EPSG:4326")], ignore_index=True)
        log(f"  OHM {s['scope']}：{len(rows)} 个砖块")
    return gpd.GeoDataFrame(bricks, crs="EPSG:4326").reset_index(drop=True)


# ───────────── 2. 归属、控制与地区 ─────────────


def owner_layer(h1938: gpd.GeoDataFrame, layers: Layers, countries: dict, scopes: list[dict]) -> gpd.GeoDataFrame:
    by_key = {r["key"]: r["tag"] for r in read_csv("owners_1938.csv")}
    tags, unmapped = [], Counter()
    for r in h1938.itertuples():
        tag = by_key.get(r.NAME) or by_key.get(r.SUBJECTO)
        if tag is None:
            unmapped[r.NAME] += 1
        tags.append(tag)
    if unmapped:
        log(f"  1938 数据中未映射、改用现代国家回退的多边形：{dict(unmapped)}")
    h = h1938.assign(tag=tags)
    h = h[h.tag.notna()].dissolve(by="tag").reset_index()[["tag", "geometry"]]
    # 各国多边形互不重叠：小国优先保留
    h = h.assign(area=h.geometry.area).sort_values("area").reset_index(drop=True)
    taken, geoms = None, []
    for g in h.geometry:
        geoms.append(g if taken is None else g.difference(taken))
        taken = g if taken is None else shapely.union(taken, g)
    paint = list(zip(h.tag, geoms))
    for o in read_csv("owners_1936.csv"):
        assert o["tag"] in countries, o
        g = layers.eval(o["expr"])
        paint = [(t, x.difference(g)) for t, x in paint] + [(o["tag"], g)]
    # OHM 各范围的顶级单位按 1936 年的所属国涂色（最后涂，优先级最高）
    for s in scopes:
        su = layers.ohm[layers.ohm.scope == s["scope"]]
        g = shapely.union_all(su[su.level == su.level.min()].geometry.values)
        paint = [(t, x.difference(g)) for t, x in paint] + [(s["tag"], g)]
    out = gpd.GeoDataFrame({"tag": [t for t, _ in paint]}, geometry=[polygonal(g) for _, g in paint], crs="EPSG:4326")
    out = out[~out.geometry.is_empty].dissolve(by="tag").reset_index()
    bad = set(out.tag) - set(countries)
    assert not bad, f"owners_1938.csv 引用了未定义的国家 {bad}"
    return out


def build_pieces(bricks: gpd.GeoDataFrame, owners: gpd.GeoDataFrame) -> gpd.GeoDataFrame:
    """砖块 × 1936 年所属国；细分砖块整块归多数方，大砖块按面积切分（真实国界穿过现代行政区）"""
    fallback = {r["adm0"]: r["tag"] for r in read_csv("owners_modern.csv")}
    bricks = bricks.assign(bid=range(len(bricks)))
    # 没有现代国家代码的砖块（OHM）按位置补上，用于回退归属
    missing = bricks.adm0.isna()
    if missing.any():
        ne = gpd.read_file(CACHE / "ne_10m_admin_1_states_provinces.zip", columns=["adm0_a3"])
        pts = gpd.GeoDataFrame(geometry=bricks[missing].geometry.representative_point(), crs="EPSG:4326")
        j = gpd.sjoin_nearest(pts, ne, max_distance=1.0).groupby(level=0).first()
        bricks.loc[missing, "adm0"] = j.adm0_a3.reindex(bricks[missing].index).to_numpy()
    inter = gpd.overlay(bricks, owners, how="intersection", keep_geom_type=True)
    rest = gpd.overlay(bricks, owners[["geometry"]], how="difference", keep_geom_type=True)
    rest["tag"] = rest.adm0.map(fallback)
    unknown = sorted(set(rest[rest.tag.isna()].adm0.dropna()))
    assert not unknown, f"owners_modern.csv 缺少现代国家代码 {unknown}"
    pieces = gpd.GeoDataFrame(pd.concat([inter, rest], ignore_index=True), crs="EPSG:4326")
    pieces = pieces[pieces.tag.notna()]
    pieces["area_km2"] = km2(pieces.geometry)
    pieces = pieces[pieces.area_km2 > 0.5]

    brick_area = pieces.groupby("bid").area_km2.transform("sum")
    major = pieces.loc[pieces.groupby("bid").area_km2.idxmax()].set_index("bid").tag
    fine = pieces.src.isin(["ohm", "gb"]) & (brick_area < SLIVER_BRICK_KM2)
    sliver = fine | ((pieces.area_km2 < 0.15 * brick_area) & (pieces.area_km2 < 2500))
    pieces.loc[sliver, "tag"] = pieces.loc[sliver, "bid"].map(major)
    # 中国各县的归属由 china_1936.csv 决定
    china = {r["id"]: r["tag"] for r in read_csv("china_1936.csv")}
    cn = pieces.cn.notna()
    pieces.loc[cn, "tag"] = pieces.loc[cn, "cn"].map(china)

    keep = ["bid", "tag", "src", "adm0", "u1", "u2", "u3", "cn", "name_zh", "name_en"]
    pieces = pieces.dissolve(by=["bid", "tag"], as_index=False, aggfunc="first")[keep + ["geometry"]]
    pieces["geometry"] = pieces.geometry.map(polygonal)
    pieces["area_km2"] = km2(pieces.geometry)
    return pieces[pieces.area_km2 > 0.5].reset_index(drop=True)


def label_by_majority(pieces: gpd.GeoDataFrame, geom, threshold: float = 0.5) -> pd.Series:
    """面积过半落在 geom 内的碎片"""
    hits = pieces.sindex.query(geom, predicate="intersects")
    out = pd.Series(False, index=pieces.index)
    if len(hits):
        sub = pieces.geometry.iloc[hits]
        frac = sub.intersection(geom).area / sub.area
        out.iloc[hits] = (frac > threshold).to_numpy()
    return out


# ───────────── 3. 合并与切分 ─────────────


@dataclass
class Cluster:
    tag: str
    controller: str
    region: str | None
    hard: str | None
    pieces: list[int]
    weight: float
    area: float
    units: Counter = field(default_factory=Counter)
    locked: bool = False
    alive: bool = True


def piece_weights(pieces: gpd.GeoDataFrame, cities: gpd.GeoDataFrame) -> np.ndarray:
    reps = pieces.geometry.representative_point()
    base = np.array([a * region_density(p.x, p.y) for a, p in zip(pieces.area_km2, reps)])
    joined = gpd.sjoin(cities[["pop", "geometry"]], pieces[["geometry"]], predicate="within")
    bonus = joined.groupby("index_right")["pop"].apply(lambda s: sum(city_weight(p) for p in s))
    return base + bonus.reindex(range(len(pieces)), fill_value=0).to_numpy()


def piece_adjacency(geoms) -> list[set[int]]:
    values = np.array(list(geoms), dtype=object)
    tree = shapely.STRtree(values)
    left, right = tree.query(values, predicate="dwithin", distance=0.005)
    adj = [set() for _ in range(len(values))]
    for a, b in zip(left, right):
        if a != b:
            adj[a].add(b)
            adj[b].add(a)
    return adj


def merge_pieces(pieces: gpd.GeoDataFrame, weights: np.ndarray, W: float, single: set[str]) -> list[Cluster]:
    """自小到大贪心合并。必须同归属、同控制者、同地区；1936 年大行政单位（hard）为硬边界；
    优先与同一上级单位的邻居合并。权重低于 MERGE_BELOW·W 或面积低于最小面积的簇需要合并。
    mode=single 的地区预先整体合并为一个锁定的簇。"""
    adj = piece_adjacency(pieces.geometry)
    reps = pieces.geometry.representative_point()
    limits = np.array([min_area(p.x, p.y) for p in reps])

    clusters: list[Cluster] = []
    owner = [-1] * len(pieces)
    groups: dict[tuple, int] = {}
    for i, r in enumerate(pieces.itertuples()):
        units = Counter({u: weights[i] for u in (r.u1, r.u2, r.u3) if isinstance(u, str)})
        if r.region in single:
            key = (r.region, r.tag, r.controller)
            if key in groups:
                c = clusters[groups[key]]
                c.pieces.append(i)
                c.weight += weights[i]
                c.area += r.area_km2
                c.units.update(units)
                owner[i] = groups[key]
                continue
            groups[key] = len(clusters)
        clusters.append(Cluster(r.tag, r.controller, r.region, r.hard, [i], weights[i], r.area_km2, units,
                                locked=r.region in single))
        owner[i] = len(clusters) - 1
    cadj = [set() for _ in clusters]
    for a, nb in enumerate(adj):
        for b in nb:
            if owner[a] != owner[b]:
                cadj[owner[a]].add(owner[b])

    def needs_merge(c: Cluster) -> tuple[bool, bool]:
        lim = limits[c.pieces[0]]
        small = c.area < lim
        return small or c.weight < MERGE_BELOW * W, small

    def compatible(c: Cluster, o: Cluster, forced: bool) -> bool:
        if not o.alive or o.locked or (o.tag, o.controller, o.region) != (c.tag, c.controller, c.region):
            return False
        if c.hard != o.hard and not forced:
            return False
        return forced or c.weight + o.weight <= MERGE_CAP * W

    heap = [(c.weight, i) for i, c in enumerate(clusters)]
    heapq.heapify(heap)
    while heap:
        w, i = heapq.heappop(heap)
        c = clusters[i]
        if not c.alive or c.locked or w != c.weight:
            continue
        need, small = needs_merge(c)
        if not need:
            continue
        options = [j for j in cadj[i] if compatible(c, clusters[j], False)]
        if not options and small:
            options = [j for j in cadj[i] if compatible(c, clusters[j], True)]
        if not options:
            continue

        def score(j: int):
            o = clusters[j]
            shared = sum(min(c.units[u], o.units[u]) > 0 for u in c.units)
            return (-(o.hard == c.hard), -shared, o.weight, j)

        j = min(options, key=score)
        t = clusters[j]
        t.pieces += c.pieces
        t.weight += c.weight
        t.area += c.area
        t.units.update(c.units)
        c.alive = False
        for k in cadj[i]:
            if k != j:
                cadj[k].discard(i)
                cadj[k].add(j)
                cadj[j].add(k)
        cadj[j].discard(i)
        cadj[j].discard(j)
        heapq.heappush(heap, (t.weight, j))
    return [c for c in clusters if c.alive]


def merge_islands(clusters: list[Cluster], pieces: gpd.GeoDataFrame, W: float) -> list[Cluster]:
    """没有同国陆地邻居的小簇（海岛）并入 ISLAND_MERGE_KM 以内最近的同国、同控制者的簇"""
    geoms = [shapely.union_all(pieces.geometry.iloc[c.pieces].values) for c in clusters]
    tree = shapely.STRtree(geoms)
    order = sorted(range(len(clusters)), key=lambda i: clusters[i].weight)
    target = list(range(len(clusters)))

    def root(i: int) -> int:
        while target[i] != i:
            i = target[i]
        return i

    for i in order:
        c = clusters[i]
        pt = geoms[i].representative_point()
        small = c.area < min_area(pt.x, pt.y) or c.weight < MERGE_BELOW * W
        if not small or c.locked or root(i) != i:
            continue
        best = None
        for j in tree.query(geoms[i], predicate="dwithin", distance=ISLAND_MERGE_KM / 80):
            j = root(int(j))
            o = clusters[j]
            if j == i or o.locked or (o.tag, o.controller, o.region) != (c.tag, c.controller, c.region):
                continue
            pa, pb = nearest_points(geoms[i], geoms[j])
            d = haversine_km((pa.x, pa.y), (pb.x, pb.y))
            if d <= ISLAND_MERGE_KM and (best is None or d < best[0]):
                best = (d, j)
        if best is None:
            continue
        j = best[1]
        o = clusters[j]
        o.pieces += c.pieces
        o.weight += c.weight
        o.area += c.area
        o.units.update(c.units)
        c.alive = False
        target[i] = j
        geoms[j] = shapely.union(geoms[j], geoms[i])
    return [c for c in clusters if c.alive]


def bisect_pieces(idx: list[int], weights: np.ndarray, reps: list[Point], k: int) -> list[list[int]]:
    """沿较长轴按加权中位数递归二分，使切分沿砖块边界"""
    if k <= 1 or len(idx) <= 1:
        return [idx]
    xs = np.array([reps[i].x for i in idx])
    ys = np.array([reps[i].y for i in idx])
    coord = xs * math.cos(math.radians(ys.mean())) if np.ptp(xs) * math.cos(math.radians(ys.mean())) > np.ptp(ys) else ys
    order = np.argsort(coord)
    w = np.array([weights[i] for i in idx])[order]
    k1 = k // 2
    cut = int(np.searchsorted(np.cumsum(w), w.sum() * k1 / k)) + 1
    cut = min(max(cut, 1), len(idx) - 1)
    left = [idx[o] for o in order[:cut]]
    right = [idx[o] for o in order[cut:]]
    return bisect_pieces(left, weights, reps, k1) + bisect_pieces(right, weights, reps, k - k1)


def split_geometry(geom, k: int, rng: np.random.Generator) -> list:
    """单一砖块过大时的退路：在等面积投影下撒点做 k-means，以 Voronoi 单元切分"""
    # 投影后可能出现自相交，先修复
    proj = polygonal(shapely.make_valid(gpd.GeoSeries([geom], crs="EPSG:4326").to_crs(EQUAL_AREA).iloc[0]))
    minx, miny, maxx, maxy = proj.bounds
    pts = []
    prepared = shapely.prepared.prep(proj)
    while len(pts) < k * 120:
        cand = np.column_stack([rng.uniform(minx, maxx, 4000), rng.uniform(miny, maxy, 4000)])
        pts.extend(c for c in cand if prepared.contains(Point(c)))
    pts = np.array(pts[: k * 120])
    centers = pts[rng.choice(len(pts), k, replace=False)]
    for _ in range(40):
        d = ((pts[:, None, :] - centers[None, :, :]) ** 2).sum(-1)
        label = d.argmin(1)
        centers = np.array([pts[label == i].mean(0) if (label == i).any() else centers[i] for i in range(k)])
    cells = shapely.voronoi_polygons(shapely.MultiPoint(centers), extend_to=proj.envelope.buffer(1e6))
    parts = [proj.intersection(cell) for cell in cells.geoms]
    parts = [p for p in parts if not p.is_empty and p.area > 0]
    back = gpd.GeoSeries(parts, crs=EQUAL_AREA).to_crs("EPSG:4326")
    return [shapely.make_valid(g) for g in back]


# ───────────── 4. 省份属性 ─────────────


def classify_terrain(provs: gpd.GeoDataFrame, regions: gpd.GeoDataFrame, big_cities: gpd.GeoDataFrame) -> list[str]:
    ea = provs.to_crs(EQUAL_AREA)
    reg = regions.to_crs(EQUAL_AREA)
    frac = {}
    shapes = ea.geometry.make_valid().buffer(0)
    for cla, key in [("Range/mtn", "mtn"), ("Desert", "desert"), ("Wetlands", "marsh"), ("Tundra", "tundra")]:
        layer = reg[reg.FEATURECLA == cla]
        union = shapely.union_all(layer.geometry.make_valid().buffer(0).values)
        frac[key] = (shapes.intersection(union).area / shapes.area).fillna(0).to_numpy()
    urban_idx = set(gpd.sjoin(big_cities[["geometry"]], provs[["geometry"]], predicate="within").index_right)

    out = []
    for i, r in enumerate(provs.itertuples()):
        lon, lat = r.center
        if i in urban_idx and r.area_km2 < 45000:
            t = "Urban"
        elif frac["mtn"][i] > 0.4:
            t = "Mountain"
        elif abs(lat) > 64 or frac["tundra"][i] > 0.4:
            t = "Tundra"
        elif frac["desert"][i] > 0.35:
            t = "Desert"
        elif frac["marsh"][i] > 0.3:
            t = "Marsh"
        elif any(x0 <= lon <= x1 and y0 <= lat <= y1 for (x0, x1), (y0, y1) in JUNGLE_BOXES):
            t = "Jungle"
        elif frac["mtn"][i] > 0.15:
            t = "Hills"
        elif (55 <= lat <= 64 and (lon > 10 or lon < -50)) or (46 <= lat < 55 and lon < -60):
            t = "Forest"  # 北方针叶林带
        else:
            t = "Plains"
        out.append(t)
    return out


def largest_remainder(total: int, scores: list[float]) -> list[int]:
    if total <= 0 or not scores:
        return [0] * len(scores)
    s = sum(scores) or 1.0
    raw = [total * x / s for x in scores]
    base = [int(x) for x in raw]
    for i in sorted(range(len(raw)), key=lambda i: base[i] - raw[i])[: total - sum(base)]:
        base[i] += 1
    return base


# ───────────── 主流程 ─────────────


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--target", type=int, default=950, help="目标权重份数")
    ap.add_argument("--out", type=Path, default=HERE.parent.parent / "web/public/mods/world-1936")
    args = ap.parse_args()
    rng = np.random.default_rng(SEED)

    countries = {r["tag"]: r for r in read_csv("countries.csv")}
    factions = read_csv("factions.csv")
    regions_def = read_csv("regions_1936.csv")
    single = {r["id"] for r in regions_def if r["mode"] == "single"}

    log("1/7 载入数据")
    adm1 = load_admin1()
    hist = {y: sources.historical(y) for y in sources.HISTORICAL_YEARS}
    scopes = read_csv("ohm_layers.csv")
    units = ohm_units(scopes)
    layers = Layers(adm1, hist, units)
    renames = {r["modern"]: (r["name"], r["name_en"]) for r in read_csv("city_names_1936.csv")}
    modern_cities = {r["name"] for r in read_csv("modern_cities.csv")}
    cities = gpd.read_file(
        CACHE / "ne_10m_populated_places.zip", columns=["NAME", "NAME_ZH", "POP_MAX", "ADM0_A3"]
    ).rename(columns={"POP_MAX": "pop"})
    cities = cities[(cities["pop"] >= 1e5) & ~cities.NAME.isin(modern_cities)].reset_index(drop=True)
    geo_regions = gpd.read_file(CACHE / "ne_10m_geography_regions_polys.zip", columns=["FEATURECLA", "NAME"])

    log("2/7 构建砖块")
    bricks = ne_bricks(adm1)
    ne_names = dict(zip(adm1.adm1_code, adm1.name))
    for iso in sources.GEOBOUNDARIES:
        bricks = replace_country(bricks, iso, sources.geoboundaries(iso))
    assign_china(bricks, ne_names)
    bricks = apply_ohm(bricks, units, scopes)
    log(f"  砖块 {len(bricks)} 个（{dict(Counter(bricks.src))}）")

    log("3/7 1936 年归属、占领区与地区")
    owners = owner_layer(hist[1938], layers, countries, scopes)
    pieces = build_pieces(bricks, owners)
    pieces["controller"] = pieces.tag
    for o in read_csv("occupation_1936.csv"):
        m = label_by_majority(pieces, layers.eval(o["expr"]))
        if not m.any():
            log(f"  ⚠ 占领区未匹配任何碎片：{o}")
        pieces.loc[m, "controller"] = o["controller"]
    pieces["region"] = None
    for r in regions_def:
        m = label_by_majority(pieces, layers.eval(r["expr"])) & pieces.region.isna()
        if not m.any():
            log(f"  ⚠ 地区 {r['id']} 未匹配任何碎片")
        pieces.loc[m, "region"] = r["id"]
    missing = set(countries) - set(pieces.tag)
    assert not missing, f"这些国家在地图上没有领土：{missing}"

    # 1936 年的大行政单位（OHM 顶级单位、中国的省）是硬边界；太小的单位不设限，以便与邻居合并
    unit_area = pieces.groupby("u1").area_km2.sum()
    reps = pieces.geometry.representative_point()
    hard = [
        u if isinstance(u, str) and u.startswith(("ohm:", "cn:")) and unit_area[u] >= min_area(p.x, p.y) else None
        for u, p in zip(pieces.u1, reps)
    ]
    pieces["hard"] = hard
    log(f"  碎片 {len(pieces)} 个")

    log("4/7 合并与切分")
    weights = piece_weights(pieces, cities)
    W = weights.sum() / args.target
    clusters = merge_islands(merge_pieces(pieces, weights, W, single), pieces, W)
    log(f"  合并后 {len(clusters)} 个")

    piece_reps = list(reps)
    rows = []
    for c in clusters:
        dominant = max(c.pieces, key=lambda p: weights[p])
        base = dict(tag=c.tag, controller=c.controller, region=c.region, dominant=dominant,
                    adm0=pieces.adm0.iloc[dominant])
        k = min(MAX_SPLIT, round(c.weight / W)) if c.weight > SPLIT_ABOVE * W and not c.locked else 1
        if k > 1 and len(c.pieces) > 1:
            for part in bisect_pieces(c.pieces, weights, piece_reps, min(k, len(c.pieces))):
                geom = polygonal(shapely.union_all(pieces.geometry.iloc[part].values))
                rows.append(dict(base, dominant=max(part, key=lambda p: weights[p]), geometry=geom))
        else:
            geom = polygonal(shapely.union_all(pieces.geometry.iloc[c.pieces].values))
            parts = split_geometry(geom, k, rng) if k > 1 else [geom]
            for part in parts:
                rows.append(dict(base, geometry=polygonal(part)))
    provs = gpd.GeoDataFrame(rows, crs="EPSG:4326")
    provs["area_km2"] = km2(provs.geometry)

    # 舍弃没有任何陆地邻居的微小孤岛（无海军时无法到达）
    adj = piece_adjacency(provs.geometry)
    keep = [not (r.area_km2 < DROP_ISLAND_KM2 and not adj[i] and r.region not in single) for i, r in enumerate(provs.itertuples())]
    dropped = Counter(provs.tag[[not k for k in keep]])
    provs = provs[keep].reset_index(drop=True)
    log(f"  切分后 {len(provs)} 个省份（舍弃微小孤岛 {sum(dropped.values())} 个）")
    missing = set(countries) - set(provs.tag)
    assert not missing, f"这些国家只剩被舍弃的孤岛：{missing}"

    log("5/7 命名、地形、胜利点与工业")
    reps = provs.geometry.representative_point()
    provs["center"] = [(round(p.x, 3), round(p.y, 3)) for p in reps]
    # 按纬度带自北向南、带内自西向东编号，相邻省份编号相近
    provs["band"] = [-int(c[1] // 5) for c in provs.center]
    provs = provs.sort_values(["band", "center"], key=lambda s: s if s.name == "band" else s.map(lambda c: c[0]))
    provs = provs.reset_index(drop=True)
    provs["id"] = range(1, len(provs) + 1)

    region_names = {r["id"]: (r["name"], r["name_en"]) for r in regions_def}
    region_count = Counter(provs.region.dropna())
    within = gpd.sjoin(cities, provs[["geometry"]], predicate="within")
    city_by_prov = within.groupby("index_right")
    names, names_en, vps, pops = [], [], [], []
    for i, r in enumerate(provs.itertuples()):
        piece = pieces.iloc[r.dominant]
        group = city_by_prov.get_group(i) if i in city_by_prov.groups else None
        top = group.sort_values("pop", ascending=False).iloc[0] if group is not None else None
        if r.region in single or (isinstance(r.region, str) and region_count[r.region] == 1):
            zh, en = region_names[r.region]
        elif top is not None:
            zh, en = renames.get(top.NAME, (top.NAME_ZH if isinstance(top.NAME_ZH, str) else top.NAME, top.NAME))
        else:
            zh = piece.name_zh if isinstance(piece.name_zh, str) else None
            en = piece.name_en if isinstance(piece.name_en, str) else None
            zh = zh or en or f"#{r.id}"
            en = en or zh
        names.append(zh)
        names_en.append(en)
        pops.append(0.0 if group is None else float(group["pop"].sum()))
        vps.append(1 + (0 if group is None else min(25, sum(city_vp(p) for p in group["pop"]))))
    # 同名省份加方位后缀（中文名与外文名分别处理：译名相同而原名不同时只改中文名）
    def disambiguate(labels: list[str], fmt) -> None:
        by_name = defaultdict(list)
        for i, n in enumerate(labels):
            by_name[n].append(i)
        for n, idx in by_name.items():
            if len(idx) < 2:
                continue
            cx = np.mean([provs.center[i][0] for i in idx])
            cy = np.mean([provs.center[i][1] for i in idx])
            for i in idx:
                dx, dy = provs.center[i][0] - cx, provs.center[i][1] - cy
                d = ("east" if dx > 0 else "west") if abs(dx) > abs(dy) else ("north" if dy > 0 else "south")
                labels[i] = fmt(n, d)
            seen = Counter()
            for i in idx:
                seen[labels[i]] += 1
                if seen[labels[i]] > 1:
                    labels[i] = f"{labels[i]} {seen[labels[i]]}"

    zh_dir = {"east": "东", "west": "西", "north": "北", "south": "南"}
    disambiguate(names, lambda n, d: f"{n}·{zh_dir[d]}")
    disambiguate(names_en, lambda n, d: f"{n} ({d.capitalize()})")
    provs["name"], provs["name_en"], provs["vp"], provs["pop"] = names, names_en, vps, pops

    big_cities = cities[cities["pop"] >= 2.5e6]
    provs["terrain"] = classify_terrain(provs, geo_regions, big_cities)
    tree = shapely.STRtree(provs.geometry.values)

    def province_at(lon: float, lat: float, tag: str | None = None) -> int | None:
        pt = Point(lon, lat)
        hits = [i for i in tree.query(pt, predicate="intersects") if tag is None or provs.tag.iloc[i] == tag]
        if hits:
            return int(hits[0])
        cand = [i for i in tree.query(pt.buffer(4)) if tag is None or provs.tag.iloc[i] == tag]
        return min(cand, key=lambda i: provs.geometry.iloc[i].distance(pt)) if cand else None

    for o in read_csv("terrain_overrides.csv"):
        i = province_at(float(o["lon"]), float(o["lat"]))
        if i is not None:
            provs.loc[i, "terrain"] = o["terrain"]

    capitals = {}
    for tag, c in countries.items():
        i = province_at(float(c["capital_lon"]), float(c["capital_lat"]), tag)
        assert i is not None, f"{tag} 找不到首都省份"
        capitals[tag] = i
        provs.loc[i, "vp"] += 20 if c["major"] == "1" else 10
        if provs.region.iloc[i] not in single:
            provs.loc[i, "name"] = c["capital"]

    # 本土：与首都同属一个现代国家，或在首都 1500 公里以内的本国省份；工业只分布在本土
    core = []
    for r in provs.itertuples():
        cap = provs.iloc[capitals[r.tag]]
        core.append(r.adm0 == cap.adm0 or haversine_km(r.center, cap.center) < 1500)
    provs["core"] = core
    provs["civ"] = 0
    provs["mil"] = 0
    for tag, c in countries.items():
        idx = [i for i in range(len(provs)) if provs.tag.iloc[i] == tag and provs.core.iloc[i]]
        scores = [1e5 + provs["pop"].iloc[i] for i in idx]
        for col, total in [("civ", int(c["civ"])), ("mil", int(c["mil"]))]:
            for i, n in zip(idx, largest_remainder(total, scores)):
                provs.loc[i, col] = n
    provs["infra"] = [
        int(min(10, max(1, round(2 + 5 * region_density(*r.center) + (1.5 if r.core and countries[r.tag]["major"] == "1" else 0)))))
        for r in provs.itertuples()
    ]

    log("6/7 邻接、海峡与航线")
    adj = piece_adjacency(provs.geometry)
    links = {(min(a, b), max(a, b)) for a, nb in enumerate(adj) for b in nb}

    def components() -> list[int]:
        comp = [-1] * len(provs)
        nbr = defaultdict(set)
        for a, b in links:
            nbr[a].add(b)
            nbr[b].add(a)
        n = 0
        for s in range(len(provs)):
            if comp[s] >= 0:
                continue
            stack = [s]
            comp[s] = n
            while stack:
                u = stack.pop()
                for v in nbr[u]:
                    if comp[v] < 0:
                        comp[v] = n
                        stack.append(v)
            n += 1
        return comp

    # 陆地连通分量之间，距离 AUTO_STRAIT_KM 以内者以最近的一对省份相连
    comp = components()
    best: dict[tuple[int, int], tuple[float, int, int]] = {}
    geoms = provs.geometry.values
    for a in range(len(provs)):
        for b in tree.query(geoms[a], predicate="dwithin", distance=AUTO_STRAIT_KM / 80):
            if comp[a] >= comp[b]:
                continue
            pa, pb = nearest_points(geoms[a], geoms[b])
            d = haversine_km((pa.x, pa.y), (pb.x, pb.y))
            key = (comp[a], comp[b])
            if d <= AUTO_STRAIT_KM and (key not in best or d < best[key][0]):
                best[key] = (d, a, int(b))
    straits = {(min(a, b), max(a, b)) for _, a, b in best.values()}
    lanes = set()
    for lane in read_csv("sea_lanes.csv"):
        a = province_at(float(lane["lon1"]), float(lane["lat1"]))
        b = province_at(float(lane["lon2"]), float(lane["lat2"]))
        if a is None or b is None or a == b:
            log(f"  ⚠ 航线 {lane['name']} 端点不在任何省份")
            continue
        lanes.add((min(a, b), max(a, b)))
    links |= straits | lanes
    comp = components()
    sizes = Counter(comp)
    log(f"  陆地邻接 {len(links) - len(straits | lanes)} 条，自动海峡 {len(straits)} 条，远洋航线 {len(lanes)} 条")
    log(f"  连通分量 {len(sizes)} 个，最大 {sizes.most_common(1)[0][1]} 个省份")
    isolated = [i for i in range(len(provs)) if sizes[comp[i]] < 5]
    if isolated:
        log(f"  孤立省份：{', '.join(provs.name.iloc[i] for i in isolated[:30])}")
    neighbors = defaultdict(list)
    for a, b in sorted(links):
        neighbors[a].append(int(provs.id.iloc[b]))
        neighbors[b].append(int(provs.id.iloc[a]))

    log("7/7 验收、开局部署与输出")
    acceptance(provs, adj, single, renames)
    wars = [("ITA", "ETH"), ("CHI", "CCP")]
    divisions = deploy_divisions(provs, countries, neighbors, capitals, wars)
    write_mod(args.out, provs, countries, factions, neighbors, capitals, divisions, wars, units)
    report(provs, args.out)


def acceptance(provs: gpd.GeoDataFrame, adj: list[set[int]], single: set[str], renames: dict) -> None:
    """研究笔记 partition-principles.md §6 的验收用例"""
    errors = []

    def at(lon: float, lat: float):
        hit = provs[provs.geometry.contains(Point(lon, lat))]
        return hit.iloc[0] if len(hit) else None

    def expect_single(region: str, tag: str, lon: float, lat: float):
        rows = provs[provs.region == region]
        p = at(lon, lat)
        if len(rows) != 1 or p is None or p.region != region or p.tag != tag:
            errors.append(f"{region} 应为 {tag} 的单一省份，实际 {rows[['name', 'tag']].values.tolist()}")

    expect_single("memel", "LIT", 21.45, 55.45)
    expect_single("danzig", "DNZ", 18.65, 54.35)
    expect_single("saar", "GER", 7.0, 49.25)
    expect_single("london", "ENG", -0.13, 51.51)
    expect_single("paris", "FRA", 2.35, 48.86)
    # 1936-01-01 的归属抽查（历史底图曾把东普鲁士划给波兰）
    for lon, lat, tag, place in [
        (20.51, 54.71, "GER", "柯尼斯堡"), (20.48, 53.78, "GER", "阿伦施泰因"), (16.93, 52.41, "POL", "波森"),
        (25.28, 54.69, "POL", "维尔诺"), (23.90, 54.90, "LIT", "考纳斯"), (16.37, 48.21, "AUT", "维也纳"),
        (14.42, 50.08, "CZE", "布拉格"), (14.03, 50.66, "CZE", "奥西希"), (17.11, 48.15, "CZE", "布拉迪斯拉发"),
        (22.30, 48.62, "CZE", "乌日霍罗德"), (7.75, 48.58, "FRA", "斯特拉斯堡"), (28.86, 47.01, "ROM", "基希讷乌"),
        (23.60, 46.77, "ROM", "克卢日"), (24.03, 49.84, "POL", "利沃夫"), (38.75, 9.03, "ETH", "亚的斯亚贝巴"),
        (38.93, 15.33, "ITA", "阿斯马拉"), (19.82, 41.33, "ALB", "地拉那"), (116.40, 39.90, "HBC", "北平"),
        (118.78, 32.06, "CHI", "南京"), (112.55, 37.87, "SXI", "太原"), (125.32, 43.88, "MAN", "新京"),
        (116.65, 39.91, "EHB", "通州"), (111.65, 40.82, "SXI", "归绥"), (91.13, 29.65, "TIB", "拉萨"),
        (121.60, 38.91, "JAP", "大连"), (113.26, 23.13, "GDC", "广州"), (94.45, 51.72, "TAN", "克孜勒"),
        (27.14, 38.42, "TUR", "士麦那"), (41.27, 39.90, "TUR", "埃尔祖鲁姆"), (40.22, 37.91, "TUR", "迪亚巴克尔"),
        (36.16, 36.20, "FRA", "安条克"), (-21.94, 64.15, "ICE", "雷克雅未克"), (35.93, 31.95, "ENG", "安曼"),
        (44.21, 15.35, "YEM", "萨那"), (45.03, 12.79, "ENG", "亚丁"), (58.41, 23.59, "OMA", "马斯喀特"),
        (113.50, 52.03, "SOV", "赤塔"), (-5.37, 35.57, "SPR", "得土安"), (-73.25, -3.75, "PRU", "伊基托斯"),
        (21.17, 55.71, "LIT", "梅梅尔"), (29.0, 61.0, "FIN", "维堡附近"),
    ]:
        p = at(lon, lat)
        if p is None or p.tag != tag:
            errors.append(f"{place} 应属 {tag}，实际 {None if p is None else p.tag}")
    sudeten = provs[provs.region == "sudetenland"]
    if sudeten.empty or set(sudeten.tag) != {"CZE"}:
        errors.append(f"苏台德省份应全部属于捷克斯洛伐克：{sudeten[['name', 'tag']].values.tolist()}")
    london = at(-0.13, 51.51)
    if london is not None and london.area_km2 < 1500:
        errors.append(f"伦敦省面积过小：{london.area_km2:.0f} km²")
    paris = at(2.35, 48.86)
    if paris is not None and paris.area_km2 < 10000:
        errors.append(f"巴黎省应覆盖塞纳、塞纳-瓦兹、塞纳-马恩：{paris.area_km2:.0f} km²")
    # 最小面积：只允许独立地区、没有同归属陆地邻居的省份（孤岛、飞地、微型国家）例外
    small = []
    for i, r in enumerate(provs.itertuples()):
        lon, lat = r.center
        if r.area_km2 >= min_area(lon, lat) or r.region in single:
            continue
        if any((provs.tag.iloc[j], provs.controller.iloc[j], provs.region.iloc[j]) == (r.tag, r.controller, r.region) for j in adj[i]):
            if region_density(lon, lat) >= 1.0:
                small.append(f"{r.name}({r.area_km2:.0f})")
    if small:
        errors.append(f"欧洲有 {len(small)} 个省份小于最小面积：{', '.join(small[:20])}")
    modern = {k for k, (_, en) in renames.items() if k != en}
    stale = sorted(set(provs.name_en.str.replace(r" \(.*\)$| \d+$", "", regex=True)) & modern)
    if stale:
        errors.append(f"省名仍是现代地名：{stale}")
    for e in errors:
        log(f"  ✗ {e}")
    assert not errors, "验收未通过"
    log("  验收通过")


def deploy_divisions(provs, countries, neighbors, capitals, wars) -> list[dict]:
    """边境省份优先：与交战国接壤 > 与列强接壤 > 其他边境；装甲与摩托化部队一半留在首都"""
    id_to_idx = {int(pid): i for i, pid in enumerate(provs.id)}
    enemies = defaultdict(set)
    for a, b in wars:
        enemies[a].add(b)
        enemies[b].add(a)
    unit_names = {"Infantry": "步兵师", "Motorized": "摩托化师", "Panzer": "装甲师"}
    out = []
    for tag, c in countries.items():
        own = [i for i in range(len(provs)) if provs.controller.iloc[i] == tag]

        def priority(i: int) -> float:
            score = 0.0
            for nid in neighbors[i]:
                other = provs.controller.iloc[id_to_idx[nid]]
                if other == tag:
                    continue
                if other in enemies[tag]:
                    score = max(score, 100)
                elif countries[other]["major"] == "1":
                    score = max(score, 10)
                else:
                    score = max(score, 1)
            return score + provs.vp.iloc[i] / 100

        border = sorted((i for i in own if priority(i) >= 1), key=lambda i: (-priority(i), int(provs.id.iloc[i])))
        if enemies[tag]:
            border = [i for i in border if priority(i) >= 100] or border
        cap = capitals[tag]
        slots = border or [cap]
        counter = Counter()

        def add(unit: str, loc: int) -> None:
            counter[unit] += 1
            out.append(dict(
                name=f"第{counter[unit]}{unit_names[unit]}",
                country=tag, location=int(provs.id.iloc[loc]), unit_type=unit,
            ))

        for unit, col in [("Panzer", "arm"), ("Motorized", "mot")]:
            for k in range(int(c[col])):
                add(unit, cap if k % 2 == 0 else slots[k // 2 % len(slots)])
        n_inf = int(c["inf"])
        home = max(1, n_inf // 6)
        for k in range(n_inf):
            add("Infantry", cap if k < home else slots[(k - home) % len(slots)])
    return out


def write_mod(out: Path, provs, countries, factions, neighbors, capitals, divisions, wars, units) -> None:
    out.mkdir(parents=True, exist_ok=True)
    (out / "scenarios").mkdir(exist_ok=True)

    def dump(path: Path, obj) -> None:
        with open(path, "w", encoding="utf-8") as f:
            json.dump(obj, f, ensure_ascii=False, separators=(",", ":"))
            f.write("\n")

    ohm_licenses = sorted(set(units.license.fillna("CC0")))
    gb = "；".join(f"{iso} {lic}" for iso, lic in sources.GEOBOUNDARIES.items())
    mod = {
        "id": "world-1936",
        "name": "世界 1936",
        "version": "2.0.0",
        "description": f"{len(provs)} 个省份的 1936 年世界地图，以 1936 年的行政区划为基础，由 tools/mapgen 生成。",
        "depends": ["base"],
        "data": ["countries.json", "provinces.json", "scenarios/1936.json"],
        "map": "map.topojson",
        "license": (
            "GPL-3.0-or-later：国界派生自 aourednik/historical-basemaps（GPL-3.0）。"
            f"历史行政区来自 OpenHistoricalMap 贡献者（{'、'.join(ohm_licenses)}）；"
            "现代行政区、城市与地理区域来自 Natural Earth（公有领域）；"
            f"细分行政区来自 geoBoundaries（{gb}）。"
        ),
    }
    with open(out / "mod.json", "w", encoding="utf-8") as f:
        json.dump(mod, f, ensure_ascii=False, indent=2)
        f.write("\n")

    dump(out / "countries.json", {"countries": [
        {"tag": t, "name": c["name"], "name_en": c["name_en"], "color": c["color"]} for t, c in countries.items()
    ]})
    dump(out / "provinces.json", {"provinces": [
        {
            "id": int(r.id), "name": r.name, "name_en": r.name_en, "terrain": r.terrain,
            "victory_points": int(r.vp), "civ_factories": int(r.civ), "mil_factories": int(r.mil),
            "infrastructure": int(r.infra), "neighbors": sorted(neighbors[i]), "center": list(r.center),
        }
        for i, r in enumerate(provs.itertuples())
    ]})

    owners = defaultdict(list)
    occupied = defaultdict(list)
    for r in provs.itertuples():
        owners[r.tag].append(int(r.id))
        if r.controller != r.tag:
            occupied[r.controller].append(int(r.id))
    scenario = {
        "id": "1936",
        "name": "1936 · 风暴前夜",
        "description": "1936年1月1日。意大利正在入侵埃塞俄比亚；莱茵兰仍是非军事区；西班牙内战尚未爆发；中国处于分裂之中。",
        "start_date": {"year": 1936, "month": 1, "day": 1, "hour": 6},
        "player": "GER",
        "owners": owners,
        "controllers": occupied,
        "countries": [
            {
                "tag": t, "capital": int(provs.id.iloc[capitals[t]]), "manpower": float(c["manpower"]),
                "civ_factories": int(c["civ"]), "mil_factories": int(c["mil"]),
                **({"production": [
                    {"unit_type": "Panzer", "factories": 10, "progress": 10.0},
                    {"unit_type": "Infantry", "factories": 10, "progress": 20.0},
                ]} if t == "GER" else {}),
            }
            for t, c in countries.items()
        ],
        "divisions": divisions,
        "wars": [list(w) for w in wars],
        "factions": [
            {"id": f["id"], "name": f["name"], "members": [t for t, c in countries.items() if c["faction"] == f["id"]]}
            for f in factions
        ],
        "majors": [t for t, c in countries.items() if c["major"] == "1"],
        "world_tension": 15.0,
        "intro_logs": [
            "【世界快讯】1936年1月1日：意大利军队在埃塞俄比亚北部推进，国际联盟的制裁收效甚微。",
            "【欧洲】莱茵兰仍是非军事区；奥地利、捷克斯洛伐克与但泽自由市维持独立。",
            "【远东】南京国民政府与各地方实力派并立，红军已抵达陕北；冀东已在日本扶植下宣布“自治”。",
        ],
    }
    dump(out / "scenarios" / "1936.json", {"scenarios": [scenario]})

    simplified = shapely.coverage_simplify(provs.geometry.values, SIMPLIFY_DEG)
    shapes = gpd.GeoDataFrame({"id": provs.id.astype(int)}, geometry=list(simplified), crs="EPSG:4326")
    topo = topojson.Topology(shapes, prequantize=1e5, topology=True, object_name="provinces")
    with open(out / "map.topojson", "w", encoding="utf-8") as f:
        f.write(topo.to_json())


def report(provs, out: Path) -> None:
    log(f"\n省份总数 {len(provs)}")
    per = Counter(provs.tag)
    log("各国省份数：" + "  ".join(f"{t}:{per[t]}" for t in sorted(per, key=lambda t: -per[t])))
    log("地形分布：" + "  ".join(f"{k}:{v}" for k, v in Counter(provs.terrain).most_common()))
    for f in sorted(out.rglob("*.json*")):
        log(f"  {f.relative_to(out)}  {f.stat().st_size / 1024:.0f} KB")


if __name__ == "__main__":
    main()
