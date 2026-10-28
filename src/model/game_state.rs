use std::collections::HashMap;
use crate::model::combat::{Combat, CombatId, CombatOutcome};
use crate::model::country::{Country, ProductionLine};
use crate::model::division::{Division, DivisionId, DivisionOrder};
use crate::model::map::WorldMap;
use crate::model::types::{CountryId, GameDate, ProvinceId, UnitType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionMode {
    Normal,
    Targeting { source_province: ProvinceId, all_units: bool },
}

pub enum UserAction {
    TogglePause,
    SetSpeed(u8),
    StepSingleHour,
    SelectProvince(ProvinceId),
    NavigateMap(i32, i32), // 相对移动光标
    SelectNextDivision,
    EnterTargetingMode { all_units: bool },
    CancelTargetingMode,
    ConfirmTargeting,
    OrderMoveSelected(ProvinceId),
    OrderAllUnitsInProvince(ProvinceId, ProvinceId), // 某省所有师团进攻目标
    QueueProduction(UnitType),
    DeclareWarNextEnemy,
}

#[derive(Clone)]
pub struct GameState {
    pub map: WorldMap,
    pub countries: HashMap<CountryId, Country>,
    pub divisions: HashMap<DivisionId, Division>,
    pub combats: HashMap<CombatId, Combat>,
    pub date: GameDate,
    pub speed: u8,
    pub is_paused: bool,
    pub world_tension: f32,
    pub selected_province: ProvinceId,
    pub selected_division: Option<DivisionId>,
    pub player_country: CountryId,
    pub mode: InteractionMode,
    pub logs: Vec<String>,
    pub next_division_id: u32,
    pub next_combat_id: u32,
}

impl GameState {
    /// 构造二战 1939 开局场景
    pub fn new_1939_scenario() -> Self {
        let map = WorldMap::new_europe_1939();
        let mut countries = HashMap::new();

        // 德意志国（轴心国领袖）
        let mut germany = Country::new(CountryId::Germany, 1_500_000.0, 30, 45);
        germany = germany.declare_war_on(CountryId::Poland);
        germany.production_queue.push(ProductionLine {
            unit_type: UnitType::Panzer,
            assigned_factories: 15,
            progress: 30.0,
            target_province: ProvinceId::Berlin,
        });
        germany.production_queue.push(ProductionLine {
            unit_type: UnitType::Infantry,
            assigned_factories: 10,
            progress: 20.0,
            target_province: ProvinceId::Berlin,
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
        let mut divisions = HashMap::new();
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

        let mut logs = Vec::new();
        logs.push("【世界快讯】1939年9月1日：德意志国对波兰宣战！第二次世界大战爆发！".to_string());
        logs.push("【最高统帅部】请调配装甲部队，沿但泽走廊与克拉科夫突破波兰防线！".to_string());

        Self {
            map,
            countries,
            divisions,
            combats: HashMap::new(),
            date: GameDate::start_date(),
            speed: 3,
            is_paused: true,
            world_tension: 25.0,
            selected_province: ProvinceId::Berlin,
            selected_division: None,
            player_country: CountryId::Germany,
            mode: InteractionMode::Normal,
            logs,
            next_division_id: div_id + 1,
            next_combat_id: 1,
        }
    }

    /// 函数式处理单个用户动作
    pub fn handle_action(self, action: UserAction) -> Self {
        match action {
            UserAction::TogglePause => {
                let paused = !self.is_paused;
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
                    self.handle_action(UserAction::SelectProvince(target_id))
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
                let mut countries = self.countries;
                if let Some(player) = countries.get_mut(&self.player_country) {
                    let cost_factories = 5.min(player.mil_factories.max(1));
                    player.production_queue.push(ProductionLine {
                        unit_type,
                        assigned_factories: cost_factories,
                        progress: 0.0,
                        target_province: ProvinceId::Berlin,
                    });
                }
                Self {
                    countries,
                    ..self
                }
            }
            UserAction::EnterTargetingMode { all_units } => {
                let source_prov = self.selected_province;
                let has_units = self.divisions.values().any(|d| d.location == source_prov && d.country == self.player_country);
                if !has_units {
                    let mut logs = self.logs;
                    logs.push("【提示】当前地块没有我方作战部队，无法下达行军命令。".to_string());
                    return Self { logs, ..self };
                }
                let mut logs = self.logs;
                logs.push(format!(
                    "🎯【行军部署】请使用方向键选择目标地块，按 [Enter] 下达{}指令，[Esc] 取消",
                    if all_units { "【全军突击】" } else { "【师团调动】" }
                ));
                Self {
                    mode: InteractionMode::Targeting { source_province: source_prov, all_units },
                    logs,
                    ..self
                }
            }
            UserAction::CancelTargetingMode => {
                let mut logs = self.logs;
                logs.push("已取消行军部署。".to_string());
                Self {
                    mode: InteractionMode::Normal,
                    logs,
                    ..self
                }
            }
            UserAction::ConfirmTargeting => {
                match self.mode {
                    InteractionMode::Targeting { source_province, all_units } => {
                        let target_prov = self.selected_province;
                        let state = Self {
                            mode: InteractionMode::Normal,
                            ..self
                        };
                        if all_units {
                            state.handle_action(UserAction::OrderAllUnitsInProvince(source_province, target_prov))
                        } else {
                            state.handle_action(UserAction::OrderMoveSelected(target_prov))
                        }
                    }
                    InteractionMode::Normal => self,
                }
            }
            UserAction::DeclareWarNextEnemy => {
                // 查找尚未宣战的国家（法国、英国、苏联）
                let player_enemies = &self.countries[&self.player_country].enemies;
                let candidates = [CountryId::France, CountryId::UnitedKingdom, CountryId::SovietUnion];
                if let Some(&target) = candidates.iter().find(|&&c| !player_enemies.contains(&c)) {
                    let mut countries = self.countries;
                    if let Some(player) = countries.get_mut(&self.player_country) {
                        *player = player.declare_war_on(target);
                    }
                    if let Some(tgt) = countries.get_mut(&target) {
                        *tgt = tgt.declare_war_on(self.player_country);
                    }
                    let mut logs = self.logs;
                    logs.push(format!("⚡【世界大战】德意志国对 {} 宣战！战火全面升级！", target.name()));
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
        let current_loc = match self.divisions.get(&div_id) {
            Some(d) => d.location,
            None => return self,
        };

        if current_loc == target_prov {
            return self;
        }

        // 寻路：必须可达
        if let Some(path) = self.map.find_path(current_loc, target_prov, |_| true) {
            if let Some(div) = self.divisions.get_mut(&div_id) {
                div.order = DivisionOrder::Moving {
                    path,
                    progress_km: 0.0,
                    distance_km: 100.0,
                };
            }
            self.logs.push(format!(
                "【命令】{} 开始向 {} 进军！",
                self.divisions[&div_id].name,
                self.map.provinces[&target_prov].name
            ));
        }

        self
    }

    /// 函数式流水线步进整个世界（Functional State Evolution Pipeline）
    pub fn step_pipeline(self, dt_hours: f32) -> Self {
        self
            .pipeline_advance_time(dt_hours)
            .pipeline_advance_combats(dt_hours)
            .pipeline_advance_movements(dt_hours)
            .pipeline_advance_economy(dt_hours)
            .pipeline_check_capitulations()
            .pipeline_ai_decisions()
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
        let mut combat_updates = HashMap::new();
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
                    updated_combat.hours_elapsed += dt_hours;
                    combat_updates.insert(c_id, updated_combat);
                }
                CombatOutcome::AttackerVictory => {
                    finished_combats.push((c_id, CombatOutcome::AttackerVictory, combat.clone()));
                }
                CombatOutcome::DefenderVictory => {
                    finished_combats.push((c_id, CombatOutcome::DefenderVictory, combat.clone()));
                }
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

                    // 3. 攻方进驻目标地块
                    for &att_id in &combat.attackers {
                        if let Some(div) = self.divisions.get_mut(&att_id) {
                            *div = div.with_location(combat.province)
                                .with_order(DivisionOrder::Idle);
                        }
                    }
                }
                CombatOutcome::DefenderVictory => {
                    self.logs.push(format!(
                        "🛡【战报】{} 防守成功！进攻方攻势受挫停止！",
                        prov_name
                    ));
                    // 攻方停止进攻
                    for &att_id in &combat.attackers {
                        if let Some(div) = self.divisions.get_mut(&att_id) {
                            *div = div.with_order(DivisionOrder::Idle);
                        }
                    }
                    // 守方恢复待命
                    for &def_id in &combat.defenders {
                        if let Some(div) = self.divisions.get_mut(&def_id) {
                            *div = div.with_order(DivisionOrder::Idle);
                        }
                    }
                }
                _ => {}
            }
        }

        // 清除完全战死的师团
        self.divisions.retain(|_, d| d.strength > 0.0);

        // 更新未结束的战斗
        for (c_id, combat) in combat_updates {
            self.combats.insert(c_id, combat);
        }

        self
    }

    /// 3. 推进师团行军与移动接触
    fn pipeline_advance_movements(mut self, dt_hours: f32) -> Self {
        let mut new_combats_to_start = Vec::new();
        let mut arrivals = Vec::new();

        for (id, div) in &mut self.divisions {
            if let DivisionOrder::Moving { ref mut path, ref mut progress_km, distance_km } = div.order {
                let hourly_speed = div.speed_kmh;
                *progress_km += hourly_speed * dt_hours;

                if *progress_km >= distance_km {
                    // 到达下一跳
                    if let Some(next_prov) = path.first().copied() {
                        path.remove(0);
                        *progress_km = 0.0;
                        arrivals.push((*id, next_prov, path.is_empty()));
                    }
                }
            }
        }

        // 处理到达事件
        for (div_id, next_prov, is_path_empty) in arrivals {
            let div_country = self.divisions[&div_id].country;
            let prov_controller = self.map.provinces[&next_prov].controller;

            // 检查该省份是否是敌对控制，或者驻扎着敌对部队
            let has_hostile_units = self.divisions.values().any(|d| {
                d.location == next_prov && self.countries[&div_country].is_at_war_with(d.country)
            });

            let is_hostile_territory = self.countries[&div_country].is_at_war_with(prov_controller) || has_hostile_units;

            if is_hostile_territory {
                // 触发战斗！
                new_combats_to_start.push((div_id, next_prov));
            } else {
                // 和平移动进入
                if let Some(div) = self.divisions.get_mut(&div_id) {
                    div.location = next_prov;
                    // 占领无守军的中立/敌方地块
                    if self.countries[&div_country].is_at_war_with(prov_controller) {
                        self.map.provinces.get_mut(&next_prov).unwrap().controller = div_country;
                    }
                    if is_path_empty {
                        div.order = DivisionOrder::Idle;
                    }
                }
            }
        }

        // 启动新的战斗
        for (attacker_id, target_prov) in new_combats_to_start {
            // 检查目标省份是否已有正在进行的战斗
            let existing_combat = self.combats.values_mut().find(|c| c.province == target_prov);

            if let Some(combat) = existing_combat {
                if !combat.attackers.contains(&attacker_id) {
                    combat.attackers.push(attacker_id);
                }
                if let Some(div) = self.divisions.get_mut(&attacker_id) {
                    div.order = DivisionOrder::Fighting {
                        combat_province: target_prov,
                        is_attacker: true,
                    };
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
                    div.order = DivisionOrder::Fighting {
                        combat_province: target_prov,
                        is_attacker: true,
                    };
                }
                for &def_id in &defenders {
                    if let Some(div) = self.divisions.get_mut(&def_id) {
                        div.order = DivisionOrder::Fighting {
                            combat_province: target_prov,
                            is_attacker: false,
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
            let spawn_loc = match country_id {
                CountryId::Germany => ProvinceId::Berlin,
                CountryId::Poland => ProvinceId::Warsaw,
                CountryId::France => ProvinceId::Paris,
                CountryId::UnitedKingdom => ProvinceId::London,
                CountryId::SovietUnion => ProvinceId::Moscow,
            };

            let div_id = self.next_division_id;
            self.next_division_id += 1;

            let name = format!("{}-第{}新编师", country_id.short_name(), div_id % 100);
            let new_div = Division::new(div_id, name, country_id, spawn_loc, unit_type);
            self.divisions.insert(div_id, new_div);

            if country_id == self.player_country {
                self.logs.push(format!("🏭【军工生产】新编 {} 已在首都部署就绪！", unit_type.name()));
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

    /// 5. 检查国家投降机制（Capitulation）
    fn pipeline_check_capitulations(mut self) -> Self {
        let countries_keys: Vec<CountryId> = self.countries.keys().copied().collect();

        for country_id in countries_keys {
            if country_id == self.player_country {
                continue;
            }
            let is_already_capitulated = self.countries[&country_id].has_capitulated;
            if is_already_capitulated {
                continue;
            }

            let orig_vp = self.map.original_victory_points(country_id);
            let cur_vp = self.map.total_victory_points(country_id);

            // 占领控制度低于 20% 时宣告投降
            if orig_vp > 0 && (cur_vp as f32 / orig_vp as f32) < 0.20 {
                let country_name = country_id.name();
                self.logs.push(format!(
                    "🏳【历史性时刻】{} 国防崩溃，正式向交战国签署无条件投降书！",
                    country_name
                ));

                if let Some(c) = self.countries.get_mut(&country_id) {
                    *c = c.with_capitulation(true);
                }

                self.world_tension = (self.world_tension + 20.0).min(100.0);
            }
        }

        self
    }

    /// 6. 纯函数生成极简战术 AI
    fn pipeline_ai_decisions(mut self) -> Self {
        // 每逢整点有概率触发 AI 调度
        if self.date.hour % 6 != 0 {
            return self;
        }

        let non_player_countries: Vec<CountryId> = self.countries.keys()
            .copied()
            .filter(|&c| c != self.player_country && !self.countries[&c].has_capitulated)
            .collect();

        let mut ai_orders = Vec::new();

        for country_id in non_player_countries {
            let enemies = &self.countries[&country_id].enemies;
            if enemies.is_empty() {
                continue;
            }

            // 找到空闲师团
            let idle_units: Vec<DivisionId> = self.divisions.values()
                .filter(|d| d.country == country_id && matches!(d.order, DivisionOrder::Idle))
                .map(|d| d.id)
                .collect();

            for unit_id in idle_units {
                let current_loc = self.divisions[&unit_id].location;
                // 寻找接壤的敌方省份
                let hostile_neighbor = self.map.provinces[&current_loc].neighbors.iter()
                    .find(|&&n| enemies.contains(&self.map.provinces[&n].controller));

                if let Some(&target) = hostile_neighbor {
                    ai_orders.push((unit_id, target));
                }
            }
        }

        for (unit_id, target) in ai_orders {
            self = self.order_unit_to_move(unit_id, target);
        }

        self
    }
}
