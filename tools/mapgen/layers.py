"""数据表中的几何表达式。

  hb<年份>:<名称>            historical-basemaps 该年份中 NAME 为此名称的多边形（并集）
  ne:<国家代码>              Natural Earth 一级行政区中该现代国家的全部
  ne:<国家代码>/<字段>=<值|值>  其中字段（name / iso / region）取这些值的行政区
  ohm:<名称>                 已抓取的 OpenHistoricalMap 行政区（name 或 name:en）

运算：" + " 为并（优先级最低）；每一项内部用 " & "（交）与 " - "（差）自左向右组合。
例：hb1938:Romania & ne:MDA + hb1938:Romania & ne:UKR/name=Odessa
"""

from __future__ import annotations

import re
from functools import lru_cache

import geopandas as gpd
import shapely

NE_FIELDS = {"name": "name", "iso": "iso_3166_2", "region": "region"}


class Layers:
    def __init__(self, adm1: gpd.GeoDataFrame, historical: dict[int, gpd.GeoDataFrame], ohm: gpd.GeoDataFrame):
        self.adm1 = adm1
        self.historical = historical
        self.ohm = ohm
        self.atom = lru_cache(maxsize=None)(self._atom)

    def _atom(self, token: str):
        kind, _, arg = token.partition(":")
        if kind.startswith("hb"):
            h = self.historical[int(kind[2:])]
            sel = h[h.NAME == arg]
        elif kind == "ne":
            iso, _, cond = arg.partition("/")
            sel = self.adm1[self.adm1.adm0_a3 == iso]
            if cond:
                field, _, values = cond.partition("=")
                sel = sel[sel[NE_FIELDS[field]].isin(values.split("|"))]
        elif kind == "ohm":
            sel = self.ohm[(self.ohm.name == arg) | (self.ohm.name_en == arg)]
        else:
            raise ValueError(f"未知的图层 {token!r}")
        if sel.empty:
            raise ValueError(f"表达式 {token!r} 没有匹配任何多边形")
        return shapely.make_valid(shapely.union_all(sel.geometry.values))

    def eval(self, expr: str):
        terms = []
        for term in expr.split(" + "):
            parts = re.split(r" ([&-]) ", term.strip())
            g = self.atom(parts[0])
            for op, token in zip(parts[1::2], parts[2::2]):
                other = self.atom(token)
                g = g.intersection(other) if op == "&" else g.difference(other)
            terms.append(g)
        return shapely.make_valid(shapely.union_all(terms))
