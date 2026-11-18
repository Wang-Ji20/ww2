# 钢铁雄心4 (Hearts of Iron IV) · 极简二战战略推演 (Rust + WebAssembly + React)

本项目以 **Rust 纯函数领域核心编译为 WebAssembly，配合 React + TypeScript 网页界面**，打造经典二战大战略游戏《钢铁雄心4》（Hearts of Iron IV）的最简推演版本。整个游戏在浏览器内运行，无需后端服务器。

---

## 一、架构特色：面向“函数式编程”（Functional Programming）的设计

本项目在设计上严格贯彻了函数式编程理念与 **“纯函数核心，命令式外壳”（Functional Core, Imperative Shell）** 架构模式：

1. **不可变数据与纯函数演进管线（Pure State Evolution Pipeline）**：
   - 所有的宏观大战略状态推进逻辑全部收敛在纯函数流水线中：
     ```rust
     state
         .pipeline_advance_time(dt_hours)
         .pipeline_advance_combats(dt_hours)
         .pipeline_advance_movements(dt_hours)
         .pipeline_advance_economy(dt_hours)
         .pipeline_check_capitulations()
         .pipeline_ai_decisions()
     ```
   - 每个子函数输入不可变状态与环境参数，返回全新的不可变状态或变换结果，**核心计算完全无副作用**。

2. **高阶函数与迭代器管道（Higher-Order Functions & Pipelines）**：
   - 欧战前线战况评估、伤害分摊、兵力衰减、组织度恢复、邻接寻路等操作，广泛采用 `filter`、`map`、`fold`、`zip`、`find` 等高阶函数组合，杜绝命令式可变游标。

3. **代数数据类型（ADTs）与详尽模式匹配**：
   - 状态、事件与操作使用清晰的 Enum（`UserAction`, `CombatOutcome`, `DivisionOrder`, `GameOutcome` 等）。
   - 状态机状态转移严格基于编译期强保证的穷尽式 `match`。

4. **浏览器作为命令式外壳**：
   - `crates/wasm` 通过 wasm-bindgen 暴露 `Game`：`tick(dt)` 推进真实时间（逐小时步进、单帧最多 24 小时），`act(json)` 应用 `UserAction`，`stateJson()` 输出完整状态快照。
   - React 前端（`web/`）以 `requestAnimationFrame` 驱动时钟，将鼠标/键盘输入映射为 `UserAction`，并把状态快照声明式渲染为 SVG 战略地图与各信息面板。

### 目录结构

```
crates/core       纯 Rust 领域核心（模组加载与校验、地图、师团、交战、经济、AI），无任何 I/O
crates/wasm       wasm-bindgen 外壳：时钟、模组装载、JSON 动作/状态接口
web/              Vite + React + TypeScript 前端（Canvas 世界地图、开局界面、各信息面板）
web/public/mods/  模组：地图、国家、兵种、地形与剧本全部是可替换的数据文件
tools/mapgen/     world-1936 地图生成器（Python），由开放的历史与现代地理数据生成约 1000 个省份
research/         二战前夕世界省份划分的研究项目（数据源普查、1936 年版图、划分规则）
```

核心不再硬编码任何国家、省份或兵种：`CountryId` 是 1–3 位的国家标签（如 `"GER"`），`ProvinceId` 是数字编号，地形与兵种是数据表。
开局时前端读取模组文件交给核心合并、校验，核心构建出整局共享的静态世界（`worldJson()`，启动时读取一次），
每帧只下发动态状态（`stateJson()`：各省控制者、师团、交战、国家经济）。

---

## 二、游戏核心机制（HOI4 经典系统）

1. **1936 年世界（world-1936 模组）**：
   - 约 1000 个省份覆盖全球，国界与行政区划为 1936 年 1 月 1 日的状态：奥地利、捷克斯洛伐克（含苏台德）、阿尔巴尼亚、埃塞俄比亚、
     但泽自由市独立，梅梅尔属立陶宛，萨尔已回归德国；中国分为南京政府、冀察、冀东、晋绥、山东、粤、桂、滇、川康、西北马家与陕北红军。
   - 省份以 1936 年的行政单位为基础（普鲁士各省与各县、苏台德各区、波兰各省、中国 1936 年的省……），苏台德、梅梅尔、但泽、萨尔、
     阿尔萨斯-洛林、比萨拉比亚等日后易手的地区是独立的省份；伦敦、巴黎按都会区成省。划分依据见 `research/`。
   - 全部国家可操控；英联邦、共产国际、日满三个阵营，列强为德、意、日、英、法、苏、美。
   - 地球是圆的：地图东西向无缝环绕（可以从西伯利亚一直向东拖到阿拉斯加），跨越 180° 经线的行军箭头与航线走较短的一侧；白令海峡与太平洋航线相连。
   - 行军距离按省份中心的真实大圆距离 × 地形行军倍率计算，A* 寻路；300 公里内的海峡自动连通，另有 23 条远洋航线（无海军系统时的跨洋通道）。
   - 开局战争：意大利 ↔ 埃塞俄比亚（意军已占领提格雷）、南京政府 ↔ 陕北红军。宣战时双方阵营成员自动参战。
   - 另附 `europe-mini` 模组：原版 17 个省份的欧洲地图，以节点图显示。
2. **师团军事与战术交战（Land Combat）**：
   - 包含步兵师（Infantry）、装甲师（Panzer）、摩托化/机械化师（Motorized）。
   - **组织度（Organization） vs 兵力耐久（Strength）**：
     - 攻方攻击力 vs 守方防御（超出防御值触发 4x 命中）。
     - 地形修正：山地、要塞（如马奇诺）提供极高防御加成，平原适合装甲闪电战。
     - 溃退与**包围歼灭（Encirclement & Overrun）**：防守部队组织度打空若无友方退路，则被直接全歼！
3. **宏观工业与生产（Economy & Production）**：
   - 政治点数（PP）、人力池（Manpower）、民用工厂（CIC）、军用工厂（MIC）。
   - 军工厂生产线持续制造新师团，完工后自动部署至首都（首都失守则部署到仍控制的本土）！
   - 每条生产线占用军工厂，总数不能超过本国军工厂；师团完工时消耗人力，人力不足时产线停工等待。
   - 参战的 AI 国家会把空闲军工厂投入默认兵种（兵种表中的第一个）生产；前线部队进攻接壤的敌方省份，后方空闲部队自动开赴最近的前线。
4. **国家投降（Capitulation）与世界紧张度（World Tension）**：
   - 当主要城市胜利点（VP）沦陷超过 80% 时，该国宣布无条件投降：剩余领土由接壤的占领国瓜分，军队解散，交战状态结束，世界紧张度剧增。
   - 玩家同样会投降（战败）；玩家阵营以外的列强全部投降则取得胜利。

---

## 三、操作控制指南

| 操作 | 功能说明 |
| :--- | :--- |
| **滚轮 / 拖动** | 缩放 / 平移地图（右下角亦有缩放按钮） |
| **点击地块** | 选中省份，查看情报与驻军 |
| **右键地块** | 命令当前选中的师团向该地块移动 / 进攻 |
| **Shift + 右键地块** | **全军突击**：当前地块内我方所有师团向目标发动总攻 |
| **`[M]` / `[A]`** | 进入部署模式（单个师团 / 全军），再点击目标地块下达命令，`[Esc]` 取消 |
| **点击师团 / `[Tab]`** | 选中 / 切换当前地块内的我方师团 |
| **`[空格]`** | 暂停 / 继续推演 |
| **`[1] ~ [5]`** | 调节推演速度（1 最慢，5 极速） |
| **`[.]`** | 单步推演 1 小时 |
| **方向键** | 在地图上切换相邻方向的省份 |
| **`[P]` / `[Shift + P]`** | 投入军工生产兵种表中的第一个 / 最后一个兵种（面板中可生产所有兵种） |
| **生产线 `×`** | 撤销生产线，释放其军工厂 |
| **外交面板「宣战」** | 对选中地块的控制国宣战，需二次确认；对方阵营成员一同参战 |
| **主菜单** | 返回开局界面，切换模组、剧本或操控的国家 |

---

## 四、编译与运行

需要 Rust（含 `wasm32-unknown-unknown` 目标）、Node.js 与 pnpm：

```bash
rustup target add wasm32-unknown-unknown

# 以下命令均在仓库根目录执行（pnpm workspace，前端位于 web/）
pnpm install         # 会安装 wasm-pack（已在 pnpm-workspace.yaml 的 allowBuilds 中放行其安装脚本）
pnpm dev             # 编译 wasm（debug）并启动开发服务器
pnpm build           # 编译 wasm（release）并输出静态站点到 web/dist，可部署到任意静态托管
pnpm test            # Rust 测试（寻路、战斗、包围、投降、JSON 接口与时钟等）+ 前端类型检查
```

---

## 五、模组（Mod）

模组位于 `web/public/mods/<id>/`，`web/public/mods/index.json` 列出开局界面可选的模组与默认选项。

```
mods/
  index.json             { "mods": [...], "default": { "mod": "world-1936", "scenario": "1936" } }
  base/                  基础规则：地形与兵种
    mod.json
    rules.json
  world-1936/            depends: ["base"]
    mod.json             清单：id、名称、版本、依赖、数据文件列表、地图轮廓
    countries.json       国家：标签、名称、颜色
    provinces.json       省份：编号、名称、地形、胜利点、工业、基建、邻接、中心经纬度
    scenarios/1936.json  剧本：开局日期、玩家、领土、占领区、各国经济、开局师团、战争、阵营、列强
    map.topojson         省份轮廓（只用于绘制，核心不读取）
```

`mod.json` 示例：

```json
{
  "id": "my-mod",
  "name": "我的模组",
  "version": "1.0.0",
  "description": "……",
  "depends": ["world-1936"],
  "data": ["patch.json"],
  "map": null
}
```

- **数据文件**可以包含 `terrains`、`unit_types`、`countries`、`provinces`、`scenarios` 中的任意几个分区。
- **叠加**：按依赖顺序加载，后加载的条目按主键（`id`，国家为 `tag`）与已有条目**深度合并**，只需写出要改的字段；
  `{"id": 42, "_delete": true}` 删除条目。例如把一个省份改成要塞、再加一个骑兵兵种：
  ```json
  { "provinces": [{ "id": 42, "terrain": "Fortified" }],
    "unit_types": [{ "id": "Cavalry", "name": "骑兵师", "icon": "[Cav]", "max_organization": 50, "soft_attack": 20,
                     "hard_attack": 2, "defense": 20, "breakthrough": 10, "speed_kmh": 7,
                     "production_cost": 40, "manpower_cost": 8000 }] }
  ```
- **地图轮廓**：使用最后一个提供 `map` 的模组；所有模组都不提供时，地图退化为以省份中心为圆心的节点图。
- **校验**：开局时核心检查所有引用（地形、邻接、领土、首都、师团驻地、兵种、阵营……），出错时在开局界面列出全部错误。
- 邻接关系只需写在一侧，核心会自动补成双向。

### 重新生成 world-1936

```bash
cd tools/mapgen
uv venv .venv && uv pip install --python .venv/bin/python -r pyproject.toml
.venv/bin/python -I sources.py         # 下载数据到 .cache/
.venv/bin/python -I mapgen.py          # 输出到 web/public/mods/world-1936
```

国家、1936 年归属、中国分省、必须独立的地区、占领区、地名、海运航线与地形修正都是 `tools/mapgen/data/` 下的 CSV，改完重新运行即可。
生成器最后会检查验收用例（见 `research/notes/partition-principles.md` §6）。

### 数据来源与许可

- [OpenHistoricalMap](https://www.openhistoricalmap.org/)：1936 年有效的历史行政区（CC0）。
- [Natural Earth](https://www.naturalearthdata.com/)：一级行政区、居民点、地理区域、陆地（公有领域）。
- [aourednik/historical-basemaps](https://github.com/aourednik/historical-basemaps)：1914、1930、1938 年世界国界（GPL-3.0）。
- [geoBoundaries](https://www.geoboundaries.org/)：中国的县与部分国家的二级行政区（PDDL、公有领域、CC BY 等，不含 ODbL）。
  `world-1936` 的地图数据派生自 historical-basemaps，因此按 GPL-3.0 发布；各来源的许可写在其 `mod.json` 中。
