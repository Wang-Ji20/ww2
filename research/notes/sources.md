# R1 · 数据源普查

> 更新：2026-10-29。结论：**OpenHistoricalMap（CC0）为主、Natural Earth（公有领域）补缺、historical-basemaps（GPL-3.0）定国界**。

## 候选数据源

| 数据源 | 内容 | 许可 | 能否再分发 | 结论 |
| :--- | :--- | :--- | :--- | :--- |
| [OpenHistoricalMap](https://www.openhistoricalmap.org/) | 带 `start_date`/`end_date` 的历史行政区边界，OSM 格式 | CC0（个别对象另标，见下） | 能 | **主力**：中欧、英属印度、美洲覆盖好；法国、苏联、中国、非洲稀疏 |
| [aourednik/historical-basemaps](https://github.com/aourednik/historical-basemaps) | 世界国界，年份 1914、1920、1930、1938…… | GPL-3.0 | 能（派生作品同为 GPL） | 现用于 1938 国界；1936 年没有快照，需由 1930 + 1938 + 人工修正推得 |
| [Natural Earth](https://www.naturalearthdata.com/) 1:10m | 现代一级行政区、居民点、地理区域、陆地 | 公有领域 | 能 | 补缺：历史上稳定的单位（法国省、日本县、意大利省、美国州）；海岸线裁切 |
| [MPIDR Population History GIS Collection](https://censusmosaic.demog.berkeley.edu/data/historical-gis-files)（Mosaic） | 欧洲 1930 年行政区、德国 1871–1931、波兰 1931 | 仅限非商业科研，需注册，不明确允许再分发 | **否** | 不用于生成；可作人工核对的参考 |
| [CShapes 2.0](https://icr.ethz.ch/data/cshapes/) | 1886–2019 国家边界（含但泽等） | CC BY-NC-SA 4.0 | 与 GPL 不兼容 | 不用 |
| [geoBoundaries](https://www.geoboundaries.org/) | 现代 ADM1–ADM3 | CC BY 4.0 | 能 | 备选：需要比 Natural Earth 更细的现代单位来拼历史地区时使用 |
| 图书馆扫描地图（LoC、Princeton 等） | 1937 年英国陆军部地图等，栅格 | 多为公有领域 | 能 | 只作人工核对与地名参考，不做矢量化 |

搜索中未找到其他开放许可的 1930 年代行政区矢量数据（[Mosaic](https://mosaic.ipums.org/historical-gis-datafiles) 与 [CShapes](https://demo.ldproxy.net/cshapes/collections) 均为非商业许可）。

## OpenHistoricalMap 详情

- **许可**：数据库整体 CC0；对象可带 `license=*` 标签另行声明（[OHM 许可说明](https://wiki.openstreetmap.org/wiki/OpenHistoricalMap/License)）。
  1936 年有效的 admin_level 2–6 关系中：CC0/未标注 5071 个，**CC-BY（苏格兰国家图书馆）40 个**（苏格兰各郡）。
  → 生成器必须读取 `license` 标签：CC-BY 对象在模组 `license` 字段中署名，或改用 Natural Earth 替代。
- **获取**：Overpass `https://overpass-api.openhistoricalmap.org/api/interpreter`。默认 Python User-Agent 会被拒（403），须自报 UA。
- **覆盖度**（1936-01-01 有效，`scripts/ohm_coverage.py` → [ohm-coverage.md](ohm-coverage.md)）：

  | 地区 | 情况 |
  | :--- | :--- |
  | 德国 | 全部普鲁士省、各邦（L4）；部分行政区（L5）；东部各县（L6）较全 |
  | 波兰 | 16 个省（L4）全；部分县 |
  | 捷克斯洛伐克 | 波希米亚、摩拉维亚-西里西亚、喀尔巴阡罗斯（L4）；**苏台德各区（Bezirk）与捷克各政治区（L5）约 95 个** |
  | 但泽、梅梅尔、萨尔 | 都有，且有按日期区分的版本（梅梅尔区 = Memel + Heydekrug + Pogegen 三县） |
  | 意大利、西班牙、奥地利、比利时、丹麦 | 大区/邦/省基本齐全 |
  | 英国 | 英格兰与威尔士、苏格兰（L4）；**County of London**（1889–1965）与部分郡（L5） |
  | 法国 | **仅约 10 个省**——用 Natural Earth 现代省补（1936 年的省与现代基本一致，巴黎地区除外：1936 年为塞纳、塞纳-瓦兹、塞纳-马恩三省） |
  | 苏联 | 部分州/自治共和国（L4 约 38 个）；1936 年 12 月宪法前后的变动需注意 |
  | 英属印度 | 各省与主要土邦（L4 约 25 + 巴基斯坦部分 11） |
  | 中国、日本、非洲 | 稀疏：满洲国诸省有；中国本部、日本本土与殖民地、非洲殖民地基本没有 |
  | 美洲 | 各国一级行政区基本齐全 |

- **几何质量**（`scripts/ohm_geometry_check.py` 抽查）：关系均能拼成有效多边形。面积与参考值相差 0–10%，差异主要来自边界包含的潟湖/海湾水域
  （东普鲁士 40 087 vs 36 996 km²，但泽 2 280 vs 1 966 km²）→ **须用 Natural Earth 陆地裁切**。

## 已知数据缺陷

| 数据 | 缺陷 | 处理 |
| :--- | :--- | :--- |
| historical-basemaps `world_1938` | 东普鲁士约 60% 落在 Poland 多边形内（阿伦施泰因行政区 100%）。旧 `world-1939` 地图很可能因此把阿伦施泰因画给了波兰 | OHM 覆盖的国家以 OHM 1936 年的单位为准涂色（`ohm_layers.csv` 的 `tag` 列）；生成器验收中加入归属抽查点 |
| historical-basemaps `world_1938` | 是慕尼黑协定与第一次维也纳仲裁**之后**的快照（苏台德、南斯洛伐克、扎奥尔济已易手）、德奥已合并、意属东非已成立 | 用 `world_1930` 回溯（`owners_1936.csv`）；二者之差正好给出苏台德等地区的范围 |
| historical-basemaps | 没有唐努图瓦 | 用 Natural Earth 现代图瓦共和国近似 |
| OHM 丹麦 | 索罗郡几何残缺（24 km²）；冰岛以 admin_level 4 挂在丹麦下 | 排除，由 Natural Earth 补；冰岛按独立国家处理 |
| OHM 摩拉维亚-西里西亚 | 没有区一级（L5）单位，无法单独切出切申、摩拉维亚的苏台德部分 | 用 geoBoundaries 捷克现代区（公有领域）细分 |
| geoBoundaries 波兰、斯洛伐克、立陶宛、匈牙利 | 许可为 ODbL（数据库相同方式共享） | 不使用 |

## 待办

- [ ] 普查 admin_level 7–8（县级）在德国、波兰的完整度：省份可能需要“县 → 省份”的二次组合。
- [ ] 检查 OHM 各国 L2 边界在 1936-01-01 是否与 historical-basemaps 一致，确定国界以谁为准。
- [ ] 中国 1936 年分省：评估 historical-basemaps 1930/1938 + Natural Earth 现代省界能否拼出（察哈尔、绥远、热河、西康、宁夏、青海）。
