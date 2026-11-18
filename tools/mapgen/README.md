# world-1936 地图生成器

由开放的历史与现代地理数据生成 `web/public/mods/world-1936`：约 1000 个省份、1936 年 1 月 1 日的国界与行政区划、开局部署。
设计依据与调研记录见 [`research/`](../../research/README.md)（尤其是 `notes/partition-principles.md`）。

## 运行

```bash
cd tools/mapgen
uv venv .venv && uv pip install --python .venv/bin/python -r pyproject.toml
.venv/bin/python -I sources.py      # 下载 Natural Earth、historical-basemaps、geoBoundaries 到 .cache/
.venv/bin/python -I mapgen.py       # 首次运行时按需抓取 OpenHistoricalMap 并缓存到 .cache/ohm/
.venv/bin/python -I mapgen.py --out /tmp/w   # 输出到其他目录预览
```

生成结果是确定的（固定随机种子）。删除 `.cache/ohm/` 下的文件可重新抓取 OHM 数据。

## 流程

1. **砖块**：以 Natural Earth 现代一级行政区为底：
   - 德、美、加、澳、巴西、阿尔及利亚、埃及、苏丹、刚果换成 geoBoundaries 二级行政区，使大省份能沿真实界线切分；
   - 中国换成县，按 `china_1936.csv` 归入 1936 年的省（察哈尔、绥远、热河、西康、宁夏……）并决定归属（南京政府、各地方实力派、中共、冀东、满洲国）；
   - `ohm_layers.csv` 中的国家换成 OpenHistoricalMap 在 1936-01-01 有效的行政区（普鲁士各省与各县、苏台德各区、波兰各省、但泽、梅梅尔……），
     较大的单位再按被替换的现代砖块细分（软边界）。
2. **归属**：historical-basemaps 1938 年国界（`owners_1938.csv` 映射）按 `owners_1936.csv` 回溯到 1936 年；细碎砖块整块归多数方。
   开局占领区见 `occupation_1936.csv`。
3. **地区**：`regions_1936.csv` 中必须独立的地区（苏台德、梅梅尔、但泽、萨尔、阿尔萨斯-洛林、比萨拉比亚……）与都会区（伦敦、巴黎）。
   砖块面积过半落入者归入该地区；省份合并不跨地区，`single` 地区整体成一个省份。
4. **合并**：同归属、同控制者、同地区的相邻砖块自小到大合并；1936 年的大行政单位（OHM 顶级单位、中国的省）为硬边界；
   权重不足或面积小于最小面积（欧洲 4000 km²，人口稀疏处按比例放大）的必须合并；海岛并入 600 公里内的同国省份。
5. **切分**：过大的省份沿砖块边界按加权中位数二分；只有单一砖块过大时才退回 k-means + Voronoi。
6. **属性**：命名（1936 年地名，见 `city_names_1936.csv`；`modern_cities.csv` 中 1936 年尚不存在的城市不参与）、地形、胜利点、工业、基建。
7. **邻接**：陆地接壤；陆地连通分量之间 300 公里内自动连接海峡；`sea_lanes.csv` 中的远洋航线。
8. **验收**：断言 `partition-principles.md` §6 的验收用例（梅梅尔、但泽、萨尔、伦敦、巴黎为单一省份，苏台德属捷克斯洛伐克，
   欧洲无过小省份，无现代地名），不通过则失败。
9. **输出**：开局部署（交战国边境优先）、模组 JSON、共享边界简化后的 TopoJSON。

## 数据表（`data/`）

| 文件 | 内容 |
| :--- | :--- |
| `countries.csv` | 国家标签、名称、颜色、阵营、是否列强、人力、工厂、开局师团数、首都 |
| `factions.csv` | 阵营 |
| `ohm_layers.csv` | 采用 OpenHistoricalMap 行政区的国家、级别与排除项 |
| `owners_1938.csv` | 1938 年国界数据中的名称 → 国家标签 |
| `owners_1936.csv` | 回溯到 1936 年的归属变化（几何表达式，语法见 `layers.py`） |
| `owners_modern.csv` | 现代国家代码 → 国家标签（国界数据未覆盖时回退） |
| `china_1936.csv` | 中国各县 → 1936 年的省与归属 |
| `occupation_1936.csv` | 开局占领区 |
| `regions_1936.csv` | 必须独立的地区与都会区 |
| `city_names_1936.csv` | 现代城市名 → 1936 年名称 |
| `modern_cities.csv` | 1936 年尚不存在的城市 |
| `sea_lanes.csv` | 远洋航线 |
| `terrain_overrides.csv` | 人工地形修正（马奇诺防线、普里皮亚季沼泽……） |

## 许可

- historical-basemaps：GPL-3.0。生成的 `world-1936` 地图数据是其派生作品，按 GPL-3.0 发布。
- OpenHistoricalMap：CC0（个别对象另有许可，生成器会把实际用到的许可写入 `mod.json`）。
- Natural Earth：公有领域。
- geoBoundaries：只使用非 ODbL 的国家，各国许可见 `sources.py` 中的 `GEOBOUNDARIES`，并写入 `mod.json`。
