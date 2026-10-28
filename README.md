# 钢铁雄心4 (Hearts of Iron IV) · 极简二战战略推演 (Rust & Bevy 0.20 TUI)

本项目采用 **Rust 最新工具链 + 最新版 Bevy (0.20) + Ratatui (TUI)** 打造的经典二战大战略游戏《钢铁雄心4》（Hearts of Iron IV）最简推演版本。

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
   - 状态、事件与操作使用清晰的 Enum（`InteractionMode`, `UserAction`, `CombatOutcome`, `DivisionOrder` 等）。
   - 状态机状态转移严格基于编译期强保证的穷尽式 `match`。

4. **Bevy 0.20 ECS 调度作为薄外壳**：
   - Bevy 0.20 仅作为事件循环、时间片调度（ScheduleRunner）以及输入输出中枢，`input_system` 捕获终端事件将其纯函数式映射为不可变 `UserAction`，由领域层处理后写回不可变单例 Resource，再由声明式的 `render_system` 投影至终端。

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
   - 军工厂生产线持续制造新师团，完工后自动部署至首都前线！
4. **国家投降（Capitulation）与世界紧张度（World Tension）**：
   - 当主要城市胜利点（VP）沦陷超过 80% 时，该国宣布无条件投降，领土被占领方分割，世界紧张度剧增。

---

## 三、操作控制指南

游戏启动即全屏 TUI 运行，支持键盘快捷操作：

| 快捷键 | 功能说明 |
| :--- | :--- |
| **`[空格]`** | **暂停 / 继续** 时间推演时钟 |
| **`[1] ~ [5]`** | 调节推演速度（1x 最慢，5x 极速推演） |
| **`[.]`** | 单步推演（步进 1 小时） |
| **`[方向键] / [WASD]`** | 在欧洲战略地图上**切换查看省份** |
| **`[Tab]`** | 在当前省份驻军中**切换选中的师团** |
| **`[M]`** | **进入移驻/进攻模式**：选择目标省份后按 `[Enter]` 出击，`[Esc]` 取消 |
| **`[A]`** | **全军突击**：命令当前省份内我方所有编制向目标地块发动总攻！ |
| **`[P]`** | **投入军工生产**：招募新的步兵师（按 `[Shift + P]` 生产重装甲师） |
| **`[W]`** | **外交决议**：对下一敌对列强（法/英/苏）正式宣战，扩大世界大战！ |
| **`[Q] / [Ctrl+C]`** | 退出游戏并恢复终端 |

---

## 四、编译与运行

```bash
# 直接运行开发版
cargo run

# 或编译高效 Release 版运行
cargo run --release

# 运行自动化测试（覆盖寻路、纯函数战斗、包围、投降等机制）
cargo test
```
