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
crates/core   纯 Rust 领域核心（地图、师团、交战、经济、AI），无任何 I/O
crates/wasm   wasm-bindgen 外壳：时钟、JSON 动作/状态接口、静态展示数据
web/          Vite + React + TypeScript 前端
```

---

## 二、游戏核心机制（HOI4 经典系统）

1. **欧洲 1939 经典战略战局**：
   - 包含德国（德意志国）、波兰、法国、英国、苏联五大交战国。
   - 拓扑真实还原欧战地理与战略走廊：
     - **西线战场**：伦敦 ↔ 多佛尔海峡 ↔ 加来 ↔ 巴黎 / 科隆 ↔ 马奇诺要塞防线
     - **中欧战略中枢**：柏林、慕尼黑、科隆
     - **东线波兰与苏联前线**：但泽走廊、华沙、克拉科夫 ↔ 明斯克、基辅 ↔ 列宁格勒、莫斯科、斯大林格勒
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
   - 参战的 AI 国家会把空闲军工厂投入步兵生产。
4. **国家投降（Capitulation）与世界紧张度（World Tension）**：
   - 当主要城市胜利点（VP）沦陷超过 80% 时，该国宣布无条件投降：剩余领土由接壤的占领国瓜分，军队解散，交战状态结束，世界紧张度剧增。
   - 玩家同样会投降（战败）；其余列强全部投降则取得胜利。

---

## 三、操作控制指南

| 操作 | 功能说明 |
| :--- | :--- |
| **点击地块** | 选中省份，查看情报与驻军 |
| **右键地块** | 命令当前选中的师团向该地块移动 / 进攻 |
| **Shift + 右键地块** | **全军突击**：当前地块内我方所有师团向目标发动总攻 |
| **`[M]` / `[A]`** | 进入部署模式（单个师团 / 全军），再点击目标地块下达命令，`[Esc]` 取消 |
| **点击师团 / `[Tab]`** | 选中 / 切换当前地块内的我方师团 |
| **`[空格]`** | 暂停 / 继续推演 |
| **`[1] ~ [5]`** | 调节推演速度（1 最慢，5 极速） |
| **`[.]`** | 单步推演 1 小时 |
| **方向键** | 在地图上切换相邻方向的省份 |
| **`[P]` / `[Shift + P]`** | 投入军工生产步兵师 / 装甲师（面板中亦可生产机械化师） |
| **生产线 `×`** | 撤销生产线，释放其军工厂 |
| **外交面板「宣战」** | 对下一个列强（法 → 英 → 苏）宣战，需二次确认 |

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
