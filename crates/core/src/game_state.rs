use std::collections::BTreeMap;
use crate::combat::{Combat, CombatId, CombatOutcome};
use crate::country::{Country, ProductionLine};
use crate::division::{Division, DivisionId, DivisionOrder};
use crate::map::WorldMap;
use crate::types::{CountryId, GameDate, ProvinceId, UnitType};

/// 战报最多保留的条数，防止日志无限增长
pub const MAX_LOGS: usize = 100;

/// 每一跳行军的距离（公里）
const HOP_DISTANCE_KM: f32 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum GameOutcome {
    /// 其余列强全部投降
    Victory,
    /// 玩家国家投降
    Defeat,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type", content = "args")]
pub enum UserAction {
    TogglePause,
    SetSpeed(u8),
    StepSingleHour,
    SelectProvince(ProvinceId),
    NavigateMap(i32, i32), // 相对移动光标
    SelectNextDivision,
    SelectDivision(DivisionId),
    OrderMoveSelected(ProvinceId),
    OrderAllUnitsInProvince(ProvinceId, ProvinceId), // 某省所有师团进攻目标
    QueueProduction(UnitType),
    CancelProduction(usize), // 生产队列下标
    DeclareWarNextEnemy,
}

#[derive(Clone, serde::Serialize)]
pub struct GameState {
    pub map: WorldMap,
    pub countries: BTreeMap<CountryId, Country>,
    pub divisions: BTreeMap<DivisionId, Division>,
    pub combats: BTreeMap<CombatId, Combat>,
    pub outcome: Option<GameOutcome>,
    pub date: GameDate,
    pub speed: u8,
    pub is_paused: bool,
    pub world_tension: f32,
    pub selected_province: ProvinceId,
    pub selected_division: Option<DivisionId>,
    pub player_country: CountryId,
    pub logs: Vec<String>,
    pub next_division_id: u32,
    pub next_combat_id: u32,
}

impl GameState {
    /// 构造二战 1939 开局场景
    pub fn new_1939_scenario() -> Self {
        let map = WorldMap::new_europe_1939();
        let mut countries = BTreeMap::new();

        // 德意志国（轴心国领袖）
        let mut germany = Country::new(CountryId::Germany, 1_500_000.0, 30, 45);
        germany = germany.declare_war_on(CountryId::Poland);
        germany.production_queue.push(ProductionLine {
            unit_type: UnitType::Panzer,
            assigned_factories: 15,
            progress: 30.0,
        });
        germany.production_queue.push(ProductionLine {
            unit_type: UnitType::Infantry,
            assigned_factories: 10,
            progress: 20.0,
        });
        countries.insert(CountryId::Germany, germany);

        // 波兰
        let mut poland = Country::new(CountryId::Poland, 600_000.0, 10, 8);
        poland = poland.declare_war_on(CountryId::Germany);
        countries.insert(CountryId::Poland, poland);

        // 法国
        let france = Country::new(CountryId::France, 1_100_000.0, 22, 18);
        countries.insert(CountryId::France, france);

        // 英国
        let uk = Country::new(CountryId::UnitedKingdom, 900_000.0, 25, 20);
        countries.insert(CountryId::UnitedKingdom, uk);

        // 苏联
        let soviet = Country::new(CountryId::SovietUnion, 3_000_000.0, 40, 35);
        countries.insert(CountryId::SovietUnion, soviet);

        // 部署开局部队 (Divisions)
        let mut divisions = BTreeMap::new();
        let mut div_id = 100u32;

        let mut spawn_div = |name: &str, country: CountryId, loc: ProvinceId, u_type: UnitType| {
            div_id += 1;
            divisions.insert(
                div_id,
                Division::new(div_id, name.to_string(), country, loc, u_type),
            );
        };

        // 德国国防军
        spawn_div("第1装甲师 [古德里安]", CountryId::Germany, ProvinceId::Berlin, UnitType::Panzer);
        spawn_div("第2装甲师", CountryId::Germany, ProvinceId::Berlin, UnitType::Panzer);
        spawn_div("第3装甲师", CountryId::Germany, ProvinceId::EastPrussia, UnitType::Panzer);
        spawn_div("第1步兵师", CountryId::Germany, ProvinceId::Berlin, UnitType::Infantry);
        spawn_div("第2步兵师", CountryId::Germany, ProvinceId::Berlin, UnitType::Infantry);
        spawn_div("第4步兵师", CountryId::Germany, ProvinceId::Cologne, UnitType::Infantry);
        spawn_div("第5步兵师", CountryId::Germany, ProvinceId::Cologne, UnitType::Infantry);
        spawn_div("第7步兵师", CountryId::Germany, ProvinceId::Munich, UnitType::Infantry);
        spawn_div("第9步兵师", CountryId::Germany, ProvinceId::EastPrussia, UnitType::Infantry);
        spawn_div("第10摩托化师", CountryId::Germany, ProvinceId::Munich, UnitType::Motorized);

        // 波兰陆军
        spawn_div("莫德林集团军", CountryId::Poland, ProvinceId::Warsaw, UnitType::Infantry);
        spawn_div("华沙防卫师", CountryId::Poland, ProvinceId::Warsaw, UnitType::Infantry);
        spawn_div("波莫瑞集团军", CountryId::Poland, ProvinceId::Danzig, UnitType::Infantry);
        spawn_div("克拉科夫骑兵师", CountryId::Poland, ProvinceId::Krakow, UnitType::Motorized);
        spawn_div("波兹南步兵师", CountryId::Poland, ProvinceId::Krakow, UnitType::Infantry);

        // 法国陆军
        spawn_div("马奇诺第1要塞要塞师", CountryId::France, ProvinceId::Maginot, UnitType::Infantry);
        spawn_div("马奇诺第2要塞师", CountryId::France, ProvinceId::Maginot, UnitType::Infantry);
        spawn_div("巴黎第1装甲师 [戴高乐]", CountryId::France, ProvinceId::Paris, UnitType::Panzer);
        spawn_div("法兰西第10步兵师", CountryId::France, ProvinceId::Paris, UnitType::Infantry);
        spawn_div("加来防卫军", CountryId::France, ProvinceId::Calais, UnitType::Infantry);

        // 英国远征军 (BEF)
        spawn_div("皇家近卫装甲师", CountryId::UnitedKingdom, ProvinceId::London, UnitType::Panzer);
        spawn_div("第1步兵师", CountryId::UnitedKingdom, ProvinceId::London, UnitType::Infantry);
        spawn_div("海峡步兵师", CountryId::UnitedKingdom, ProvinceId::Dover, UnitType::Infantry);

        // 苏联红军
        spawn_div("近卫第1装甲师 [朱可夫]", CountryId::SovietUnion, ProvinceId::Moscow, UnitType::Panzer);
        spawn_div("近卫第2步兵师", CountryId::SovietUnion, ProvinceId::Moscow, UnitType::Infantry);
        spawn_div("基辅第44步兵师", CountryId::SovietUnion, ProvinceId::Kiev, UnitType::Infantry);
        spawn_div("基辅第7装甲师", CountryId::SovietUnion, ProvinceId::Kiev, UnitType::Panzer);
        spawn_div("明斯克步兵师", CountryId::SovietUnion, ProvinceId::Minsk, UnitType::Infantry);
        spawn_div("西区摩托化师", CountryId::SovietUnion, ProvinceId::Minsk, UnitType::Motorized);
        spawn_div("列宁格勒防卫军", CountryId::SovietUnion, ProvinceId::Leningrad, UnitType::Infantry);
        spawn_div("斯大林格勒突击师", CountryId::SovietUnion, ProvinceId::Stalingrad, UnitType::Infantry);

        let logs = vec![
            "【世界快讯】1939年9月1日：德意志国对波兰宣战！第二次世界大战爆发！".to_string(),
            "【最高统帅部】请调配装甲部队，沿但泽走廊与克拉科夫突破波兰防线！".to_string(),
        ];

        Self {
            map,
            countries,
            divisions,
            combats: BTreeMap::new(),
            outcome: None,
            date: GameDate::start_date(),
            speed: 3,
            is_paused: true,
            world_tension: 25.0,
            selected_province: ProvinceId::Berlin,
            selected_division: None,
            player_country: CountryId::Germany,
            logs,
            next_division_id: div_id + 1,
            next_combat_id: 1,
        }
    }

    /// 函数式处理单个用户动作
    pub fn handle_action(self, action: UserAction) -> Self {
        self.apply_action(action).with_capped_logs()
    }

    fn apply_action(mut self, action: UserAction) -> Self {
        match action {
            UserAction::TogglePause => {
                // 对局结束后保持暂停
                let paused = !self.is_paused || self.outcome.is_some();
                Self {
                    is_paused: paused,
                    ..self
                }
            }
            UserAction::SetSpeed(spd) => {
                let spd = spd.clamp(1, 5);
                Self {
                    speed: spd,
                    ..self
                }
            }
            UserAction::StepSingleHour => {
                self.step_pipeline(1.0)
            }
            UserAction::SelectProvince(id) => {
                // 自动选择该省份的第一支友方部队
                let div_in_prov = self.divisions.values()
                    .filter(|d| d.location == id && d.country == self.player_country)
                    .map(|d| d.id)
                    .next();

                Self {
                    selected_province: id,
                    selected_division: div_in_prov.or(self.selected_division),
                    ..self
                }
            }
            UserAction::NavigateMap(dx, dy) => {
                let current_prov = &self.map.provinces[&self.selected_province];
                let current_pos = current_prov.map_pos;

                // 寻找在指定方向上最近的省份
                let closest_id = self.map.provinces.values()
                    .filter(|p| {
                        let sx = (p.map_pos.0 as i32 - current_pos.0 as i32).signum();
                        let sy = (p.map_pos.1 as i32 - current_pos.1 as i32).signum();
                        (dx != 0 && sx == dx) || (dy != 0 && sy == dy)
                    })
                    .min_by_key(|p| {
                        let dist_x = (p.map_pos.0 as i32 - current_pos.0 as i32).abs();
                        let dist_y = (p.map_pos.1 as i32 - current_pos.1 as i32).abs();
                        dist_x * 2 + dist_y * 3
                    })
                    .map(|p| p.id);

                if let Some(target_id) = closest_id {
                    self.apply_action(UserAction::SelectProvince(target_id))
                } else {
                    self
                }
            }
            UserAction::SelectNextDivision => {
                let prov_units: Vec<DivisionId> = self.divisions.values()
                    .filter(|d| d.location == self.selected_province && d.country == self.player_country)
                    .map(|d| d.id)
                    .collect();

                if prov_units.is_empty() {
                    return self;
                }

                let next_idx = match self.selected_division {
                    Some(cur_id) => {
                        prov_units.iter().position(|&id| id == cur_id)
                            .map_or(0, |idx| (idx + 1) % prov_units.len())
                    }
                    None => 0,
                };

                let new_selected = prov_units.get(next_idx).copied();
                Self {
                    selected_division: new_selected,
                    ..self
                }
            }
            UserAction::SelectDivision(div_id) => {
                // 只允许选中我方师团，否则后续命令会作用到敌军身上
                let location = self.divisions.get(&div_id)
                    .filter(|d| d.country == self.player_country)
                    .map(|d| d.location);
                match location {
                    Some(location) => Self {
                        selected_province: location,
                        selected_division: Some(div_id),
                        ..self
                    },
                    None => self,
                }
            }
            UserAction::OrderMoveSelected(target_prov) => {
                if let Some(div_id) = self.selected_division {
                    self.order_unit_to_move(div_id, target_prov)
                } else {
                    self
                }
            }
            UserAction::OrderAllUnitsInProvince(from_prov, target_prov) => {
                let units_to_order: Vec<DivisionId> = self.divisions.values()
                    .filter(|d| d.location == from_prov && d.country == self.player_country)
                    .map(|d| d.id)
                    .collect();

                units_to_order.into_iter().fold(self, |acc, d_id| {
                    acc.order_unit_to_move(d_id, target_prov)
                })
            }
            UserAction::QueueProduction(unit_type) => {
                let queued = self.countries[&self.player_country].queue_production(unit_type);
                match queued {
                    Some(updated) => {
                        self.countries.insert(self.player_country, updated);
                    }
                    None => self.logs.push("【军工】所有军工厂均已分配，请先撤销一条生产线。".to_string()),
                }
                self
            }
            UserAction::CancelProduction(index) => {
                let updated = self.countries[&self.player_country].cancel_production(index);
                self.countries.insert(self.player_country, updated);
                self
            }
            UserAction::DeclareWarNextEnemy => {
                // 查找尚未宣战的国家（法国、英国、苏联）
                let player_enemies = &self.countries[&self.player_country].enemies;
                let candidates = [CountryId::France, CountryId::UnitedKingdom, CountryId::SovietUnion];
                let next_target = candidates.iter().copied().find(|&c| {
                    c != self.player_country
                        && !player_enemies.contains(&c)
                        && !self.countries[&c].has_capitulated
                });
                if let Some(target) = next_target {
                    let mut countries = self.countries;
                    if let Some(player) = countries.get_mut(&self.player_country) {
                        *player = player.declare_war_on(target);
                    }
                    if let Some(tgt) = countries.get_mut(&target) {
                        *tgt = tgt.declare_war_on(self.player_country);
                    }
                    let mut logs = self.logs;
                    logs.push(format!("⚡【世界大战】{} 对 {} 宣战！战火全面升级！", self.player_country.name(), target.name()));
                    Self {
                        countries,
                        world_tension: (self.world_tension + 20.0).min(100.0),
                        logs,
                        ..self
                    }
                } else {
                    let mut logs = self.logs;
                    logs.push("【外交】所有主要列强均已处于交战状态！".to_string());
                    Self { logs, ..self }
                }
            }
        }
    }

    /// 纯函数：向师团下达移动/进攻指令
    fn order_unit_to_move(mut self, div_id: DivisionId, target_prov: ProvinceId) -> Self {
        let (current_loc, order, name) = match self.divisions.get(&div_id) {
            Some(d) => (d.location, d.order.clone(), d.name.clone()),
            None => return self,
        };

        if current_loc == target_prov {
            return self;
        }

        // 寻路：必须可达
        let Some(path) = self.map.find_path(current_loc, target_prov) else {
            return self;
        };

        match order {
            DivisionOrder::Fighting { is_attacker: false, .. } => {
                // 守军被敌军牵制，无法脱离战斗
                self.logs.push(format!("【命令】{} 正在坚守阵地，无法脱离战斗！", name));
                return self;
            }
            DivisionOrder::Fighting { is_attacker: true, combat_province, .. } => {
                if path.first() == Some(&combat_province) {
                    // 继续进攻同一地块，只更新胜利后的行军路线
                    if let Some(div) = self.divisions.get_mut(&div_id) {
                        div.order = DivisionOrder::Fighting {
                            combat_province,
                            is_attacker: true,
                            resume_path: path[1..].to_vec(),
                        };
                    }
                    return self;
                }
                // 攻方撤出原有战斗，改赴新目标
                for combat in self.combats.values_mut().filter(|c| c.province == combat_province) {
                    combat.attackers.retain(|&id| id != div_id);
                }
            }
            DivisionOrder::Idle | DivisionOrder::Moving { .. } => {}
        }

        if let Some(div) = self.divisions.get_mut(&div_id) {
            div.order = DivisionOrder::Moving {
                path,
                progress_km: 0.0,
                distance_km: HOP_DISTANCE_KM,
            };
        }
        self.logs.push(format!(
            "【命令】{} 开始向 {} 进军！",
            name,
            self.map.provinces[&target_prov].name
        ));

        self
    }

    /// 函数式流水线步进整个世界（Functional State Evolution Pipeline）
    /// 超过 1 小时的步长拆成逐小时结算，使行军、交战与 AI 调度与步长无关
    pub fn step_pipeline(self, dt_hours: f32) -> Self {
        if dt_hours <= 1.0 {
            return self.step_once(dt_hours);
        }
        let whole = dt_hours.floor();
        let state = (0..whole as u32).fold(self, |s, _| s.step_once(1.0));
        let rest = dt_hours - whole;
        if rest > 0.0 { state.step_once(rest) } else { state }
    }

    fn step_once(self, dt_hours: f32) -> Self {
        if self.outcome.is_some() {
            return self;
        }
        self
            .pipeline_advance_time(dt_hours)
            .pipeline_advance_combats(dt_hours)
            .pipeline_advance_movements(dt_hours)
            .pipeline_advance_economy(dt_hours)
            .pipeline_check_capitulations()
            .pipeline_check_outcome()
            .pipeline_ai_decisions()
            .with_capped_logs()
    }

    fn with_capped_logs(mut self) -> Self {
        let excess = self.logs.len().saturating_sub(MAX_LOGS);
        self.logs.drain(..excess);
        self
    }

    /// 1. 推进时钟
    fn pipeline_advance_time(self, dt_hours: f32) -> Self {
        let new_date = self.date.advance_hours(dt_hours.round() as u32);
        Self {
            date: new_date,
            ..self
        }
    }

    /// 2. 纯函数结算交战（HOI4 陆战推演）
    fn pipeline_advance_combats(mut self, dt_hours: f32) -> Self {
        let mut finished_combats = Vec::new();
        let mut combat_updates = BTreeMap::new();
        let mut division_damages = Vec::new();

        // 纯函数计算每个战斗的结果
        for (&c_id, combat) in &self.combats {
            let terrain = self.map.provinces[&combat.province].terrain;
            let report = combat.step(&self.divisions, terrain, dt_hours);

            division_damages.extend(report.damages);

            match report.outcome {
                CombatOutcome::Ongoing => {
                    let mut updated_combat = combat.clone();
                    updated_combat.progress = report.new_progress;
                    combat_updates.insert(c_id, updated_combat);
                }
                outcome => finished_combats.push((c_id, outcome, combat.clone())),
            }
        }

        // 应用伤害
        for damage in division_damages {
            if let Some(div) = self.divisions.get_mut(&damage.division_id) {
                *div = div.take_damage(damage.org_damage, damage.hp_damage);
            }
        }

        // 结算结束的战斗
        for (c_id, outcome, combat) in finished_combats {
            combat_updates.remove(&c_id);
            self.combats.remove(&c_id);

            let prov_name = self.map.provinces[&combat.province].name;

            match outcome {
                CombatOutcome::AttackerVictory => {
                    self.logs.push(format!(
                        "★【战报大捷】攻方取得 {} 战役胜利！守军全线崩溃溃退！",
                        prov_name
                    ));

                    // 1. 变更控制权
                    if let Some(prov) = self.map.provinces.get_mut(&combat.province) {
                        *prov = prov.with_controller(combat.attacker_country);
                    }

                    // 2. 守军退却或包围歼灭 (Retreat / Encirclement)
                    let friendly_retreat_prov = self.map.provinces[&combat.province]
                        .neighbors
                        .iter()
                        .find(|&&n| self.map.provinces[&n].controller == combat.defender_country)
                        .copied();

                    for &def_id in &combat.defenders {
                        if let Some(div) = self.divisions.get_mut(&def_id) {
                            if let Some(retreat_dest) = friendly_retreat_prov {
                                *div = div.with_location(retreat_dest)
                                    .with_order(DivisionOrder::Idle);
                            } else {
                                // 无路可退：包围全歼！(Overrun / Wiped Out)
                                self.logs.push(format!(
                                    "☠【包围歼灭】守军 {} 陷入绝境被全歼！",
                                    div.name
                                ));
                                *div = div.take_damage(999.0, 999.0);
                            }
                        }
                    }

                    // 3. 攻方进驻目标地块，有后续路线的继续行军
                    for &att_id in &combat.attackers {
                        if let Some(div) = self.divisions.get_mut(&att_id) {
                            let next_order = match &div.order {
                                DivisionOrder::Fighting { resume_path, .. } if !resume_path.is_empty() => {
                                    DivisionOrder::Moving {
                                        path: resume_path.clone(),
                                        progress_km: 0.0,
                                        distance_km: HOP_DISTANCE_KM,
                                    }
                                }
                                _ => DivisionOrder::Idle,
                            };
                            *div = div.with_location(combat.province).with_order(next_order);
                        }
                    }
                }
                CombatOutcome::DefenderVictory | CombatOutcome::Ongoing => {
                    self.logs.push(format!(
                        "🛡【战报】{} 防守成功！进攻方攻势受挫停止！",
                        prov_name
                    ));
                    // 攻守双方恢复待命
                    for div_id in combat.attackers.iter().chain(&combat.defenders) {
                        if let Some(div) = self.divisions.get_mut(div_id) {
                            *div = div.with_order(DivisionOrder::Idle);
                        }
                    }
                }
            }
        }

        // 清除完全战死的师团
        self.divisions.retain(|_, d| d.strength > 0.0);

        // 更新未结束的战斗
        self.combats.extend(combat_updates);

        self
    }

    /// 3. 推进师团行军与移动接触
    fn pipeline_advance_movements(mut self, dt_hours: f32) -> Self {
        let mut new_combats_to_start = Vec::new();
        let mut arrivals = Vec::new();

        for (id, div) in &mut self.divisions {
            if let DivisionOrder::Moving { ref mut path, ref mut progress_km, distance_km } = div.order {
                *progress_km += div.speed_kmh * dt_hours;

                if *progress_km >= distance_km && !path.is_empty() {
                    // 到达下一跳，超出的里程计入下一段
                    let next_prov = path.remove(0);
                    *progress_km -= distance_km;
                    arrivals.push((*id, next_prov, path.clone()));
                }
            }
        }

        // 处理到达事件
        for (div_id, next_prov, remaining_path) in arrivals {
            let div_country = self.divisions[&div_id].country;
            let prov_controller = self.map.provinces[&next_prov].controller;

            // 检查该省份是否是敌对控制，或者驻扎着敌对部队
            let has_hostile_units = self.divisions.values().any(|d| {
                d.location == next_prov && self.countries[&div_country].is_at_war_with(d.country)
            });

            let is_hostile_territory = self.countries[&div_country].is_at_war_with(prov_controller) || has_hostile_units;

            if is_hostile_territory {
                // 触发战斗！
                new_combats_to_start.push((div_id, next_prov, remaining_path));
                continue;
            }

            // 和平移动进入
            let defended_combat = self.combats.values_mut()
                .find(|c| c.province == next_prov && c.defender_country == div_country);
            let Some(div) = self.divisions.get_mut(&div_id) else { continue };
            div.location = next_prov;
            if !remaining_path.is_empty() {
                continue;
            }
            match defended_combat {
                // 抵达正在遭受进攻的友方地块：加入防守
                Some(combat) => {
                    if !combat.defenders.contains(&div_id) {
                        combat.defenders.push(div_id);
                    }
                    div.order = DivisionOrder::Fighting {
                        combat_province: next_prov,
                        is_attacker: false,
                        resume_path: Vec::new(),
                    };
                }
                None => div.order = DivisionOrder::Idle,
            }
        }

        // 启动新的战斗
        for (attacker_id, target_prov, resume_path) in new_combats_to_start {
            let attacking_order = DivisionOrder::Fighting {
                combat_province: target_prov,
                is_attacker: true,
                resume_path,
            };

            // 检查目标省份是否已有正在进行的战斗
            let existing_combat = self.combats.values_mut().find(|c| c.province == target_prov);

            if let Some(combat) = existing_combat {
                if !combat.attackers.contains(&attacker_id) {
                    combat.attackers.push(attacker_id);
                }
                if let Some(div) = self.divisions.get_mut(&attacker_id) {
                    div.order = attacking_order;
                }
            } else {
                // 新建战斗
                let attacker_country = self.divisions[&attacker_id].country;
                let defender_country = self.map.provinces[&target_prov].controller;

                // 收集防御部队
                let defenders: Vec<DivisionId> = self.divisions.values()
                    .filter(|d| d.location == target_prov && (d.country == defender_country || self.countries[&attacker_country].is_at_war_with(d.country)))
                    .map(|d| d.id)
                    .collect();

                let combat_id = self.next_combat_id;
                self.next_combat_id += 1;

                let combat = Combat::new(
                    combat_id,
                    target_prov,
                    attacker_country,
                    defender_country,
                    vec![attacker_id],
                    defenders.clone(),
                );
                self.combats.insert(combat_id, combat);

                if let Some(div) = self.divisions.get_mut(&attacker_id) {
                    div.order = attacking_order;
                }
                for &def_id in &defenders {
                    if let Some(div) = self.divisions.get_mut(&def_id) {
                        div.order = DivisionOrder::Fighting {
                            combat_province: target_prov,
                            is_attacker: false,
                            resume_path: Vec::new(),
                        };
                    }
                }

                self.logs.push(format!(
                    "⚔【战斗爆发】在 {} 爆发殊死决战！",
                    self.map.provinces[&target_prov].name
                ));
            }
        }

        self
    }

    /// 新师团的部署地：首都；首都失守则退往仍控制的本土地块，再退往任一控制地块
    fn spawn_location(&self, country: CountryId) -> Option<ProvinceId> {
        let capital = country.capital();
        if self.map.provinces[&capital].controller == country {
            return Some(capital);
        }
        let controlled = || self.map.provinces.values().filter(move |p| p.controller == country);
        controlled()
            .find(|p| p.original_owner == country)
            .or_else(|| controlled().next())
            .map(|p| p.id)
    }

    /// 4. 推进工业、生产线与组织度/兵力自然恢复
    fn pipeline_advance_economy(mut self, dt_hours: f32) -> Self {
        let mut new_spawned_divisions = Vec::new();

        // 推进各国经济
        for country in self.countries.values_mut() {
            let (updated, completed_units) = country.step_economy(dt_hours);
            *country = updated;

            for unit_type in completed_units {
                new_spawned_divisions.push((country.id, unit_type));
            }
        }

        // 部署新师团
        for (country_id, unit_type) in new_spawned_divisions {
            let Some(spawn_loc) = self.spawn_location(country_id) else { continue };

            let div_id = self.next_division_id;
            self.next_division_id += 1;

            let name = format!("{}-第{}新编师", country_id.short_name(), div_id % 100);
            let new_div = Division::new(div_id, name, country_id, spawn_loc, unit_type);
            self.divisions.insert(div_id, new_div);

            if country_id == self.player_country {
                self.logs.push(format!(
                    "🏭【军工生产】新编 {} 已在 {} 部署就绪！",
                    unit_type.name(),
                    self.map.provinces[&spawn_loc].name
                ));
            }
        }

        // 待命部队组织度与兵力缓慢回复
        for div in self.divisions.values_mut() {
            if let DivisionOrder::Idle = div.order {
                *div = div.replenish_and_recover(0.5 * dt_hours, 0.2 * dt_hours);
            }
        }

        self
    }

    /// 5. 检查国家投降机制（Capitulation）：胜利点控制度低于 20% 时投降，玩家亦不例外
    fn pipeline_check_capitulations(mut self) -> Self {
        let candidates: Vec<CountryId> = self.countries.values()
            .filter(|c| !c.has_capitulated)
            .map(|c| c.id)
            .collect();

        for country_id in candidates {
            let orig_vp = self.map.original_victory_points(country_id);
            let cur_vp = self.map.total_victory_points(country_id);

            if orig_vp > 0 && (cur_vp as f32 / orig_vp as f32) < 0.20 {
                self = self.capitulate(country_id);
            }
        }

        self
    }

    /// 投降：剩余领土由占领国瓜分，军队解散，所有交战状态结束
    fn capitulate(mut self, loser: CountryId) -> Self {
        self.logs.push(format!(
            "🏳【历史性时刻】{} 国防崩溃，正式向交战国签署无条件投降书！",
            loser.name()
        ));

        // 各占领国在其法理领土上占据的胜利点
        let mut occupier_vp: BTreeMap<CountryId, u32> = BTreeMap::new();
        for p in self.map.provinces.values().filter(|p| p.original_owner == loser && p.controller != loser) {
            *occupier_vp.entry(p.controller).or_default() += p.victory_points;
        }
        let primary_occupier = occupier_vp.iter()
            .max_by_key(|&(_, &vp)| vp)
            .map(|(&c, _)| c)
            .or_else(|| self.countries[&loser].enemies.first().copied());

        // 剩余领土：优先交给接壤的占领国，否则交给主要占领国
        let remaining: Vec<ProvinceId> = self.map.provinces.values()
            .filter(|p| p.controller == loser)
            .map(|p| p.id)
            .collect();
        for prov_id in remaining {
            let adjacent_occupier = self.map.provinces[&prov_id].neighbors.iter()
                .map(|n| self.map.provinces[n].controller)
                .find(|c| occupier_vp.contains_key(c));
            if let Some(new_controller) = adjacent_occupier.or(primary_occupier)
                && let Some(prov) = self.map.provinces.get_mut(&prov_id)
            {
                *prov = prov.with_controller(new_controller);
            }
        }

        // 结束与投降国相关的战斗，其余参战部队恢复待命
        let ended: Vec<CombatId> = self.combats.values()
            .filter(|c| c.attacker_country == loser || c.defender_country == loser)
            .map(|c| c.id)
            .collect();
        for combat_id in ended {
            if let Some(combat) = self.combats.remove(&combat_id) {
                for div_id in combat.attackers.iter().chain(&combat.defenders) {
                    if let Some(div) = self.divisions.get_mut(div_id) {
                        *div = div.with_order(DivisionOrder::Idle);
                    }
                }
            }
        }
        for combat in self.combats.values_mut() {
            combat.attackers.retain(|id| self.divisions.get(id).is_some_and(|d| d.country != loser));
            combat.defenders.retain(|id| self.divisions.get(id).is_some_and(|d| d.country != loser));
        }

        // 解散军队、停战
        self.divisions.retain(|_, d| d.country != loser);
        if self.selected_division.is_some_and(|id| !self.divisions.contains_key(&id)) {
            self.selected_division = None;
        }
        for country in self.countries.values_mut() {
            *country = if country.id == loser {
                Country {
                    enemies: Vec::new(),
                    production_queue: Vec::new(),
                    ..country.capitulated()
                }
            } else {
                country.make_peace_with(loser)
            };
        }

        self.world_tension = (self.world_tension + 20.0).min(100.0);
        self
    }

    /// 6. 判定胜负：玩家投降则失败，其余列强全部投降则胜利
    fn pipeline_check_outcome(mut self) -> Self {
        let outcome = if self.countries[&self.player_country].has_capitulated {
            Some(GameOutcome::Defeat)
        } else if self.countries.values().all(|c| c.id == self.player_country || c.has_capitulated) {
            Some(GameOutcome::Victory)
        } else {
            None
        };

        if let Some(outcome) = outcome {
            self.logs.push(match outcome {
                GameOutcome::Victory => "🏆【终局】所有列强均已投降，欧陆战争以我方全面胜利告终！".to_string(),
                GameOutcome::Defeat => "💀【终局】我国已投降，战争失败。".to_string(),
            });
            self.outcome = Some(outcome);
            self.is_paused = true;
        }
        self
    }

    /// 7. 纯函数生成极简战术 AI：每 6 小时调度一次生产与进攻
    fn pipeline_ai_decisions(mut self) -> Self {
        if !self.date.hour.is_multiple_of(6) {
            return self;
        }

        let ai_countries: Vec<CountryId> = self.countries.values()
            .filter(|c| c.id != self.player_country && !c.has_capitulated && !c.enemies.is_empty())
            .map(|c| c.id)
            .collect();

        let mut ai_orders = Vec::new();

        for country_id in ai_countries {
            // 交战中的 AI 把空闲军工厂投入步兵生产
            let mut country = self.countries[&country_id].clone();
            while let Some(updated) = country.queue_production(UnitType::Infantry) {
                country = updated;
            }
            self.countries.insert(country_id, country);

            let enemies = &self.countries[&country_id].enemies;

            // 找到空闲师团
            let idle_units = self.divisions.values()
                .filter(|d| d.country == country_id && matches!(d.order, DivisionOrder::Idle));

            for unit in idle_units {
                // 寻找接壤的敌方省份
                let hostile_neighbor = self.map.provinces[&unit.location].neighbors.iter()
                    .find(|&&n| enemies.contains(&self.map.provinces[&n].controller));

                if let Some(&target) = hostile_neighbor {
                    ai_orders.push((unit.id, target));
                }
            }
        }

        for (unit_id, target) in ai_orders {
            self = self.order_unit_to_move(unit_id, target);
        }

        self
    }
}
