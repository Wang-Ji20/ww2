use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use crate::combat::{Combat, CombatId, CombatOutcome};
use crate::country::{Country, ProductionLine};
use crate::division::{Division, DivisionId, DivisionOrder};
use crate::modpack::ModPack;
use crate::types::{CountryId, GameDate, ProvinceId, UnitTypeId};
use crate::world::World;

/// 战报最多保留的条数，防止日志无限增长
pub const MAX_LOGS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum GameOutcome {
    /// 玩家阵营以外的列强全部投降
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
    NavigateMap(i32, i32), // 相对移动光标：x 向东，y 向南
    SelectNextDivision,
    SelectDivision(DivisionId),
    OrderMoveSelected(ProvinceId),
    OrderAllUnitsInProvince(ProvinceId, ProvinceId), // 某省所有师团进攻目标
    QueueProduction(UnitTypeId),
    CancelProduction(usize), // 生产队列下标
    DeclareWar(CountryId),
}

/// 动态对局状态。静态世界（地图、规则、剧本）以 `Arc` 共享，不参与序列化。
#[derive(Clone, serde::Serialize)]
pub struct GameState {
    #[serde(skip)]
    pub world: Arc<World>,
    /// 各省当前控制者
    pub controllers: BTreeMap<ProvinceId, CountryId>,
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
    /// 由模组构建剧本开局；`player` 缺省为剧本指定的国家。数据有误时返回全部错误。
    pub fn new(pack: &ModPack, scenario_id: &str, player: Option<CountryId>) -> Result<Self, Vec<String>> {
        let sc = pack.scenario(scenario_id)
            .ok_or_else(|| vec![format!("剧本 {scenario_id:?} 不存在")])?;
        let errors = pack.validate(Some(sc));
        if !errors.is_empty() {
            return Err(errors);
        }
        let world = Arc::new(World::build(pack, sc));
        let player_country = player.unwrap_or(sc.player);
        if !world.countries.contains_key(&player_country) {
            return Err(vec![format!("国家 {player_country} 在剧本 {scenario_id} 中没有领土，无法操控")]);
        }

        let mut countries: BTreeMap<CountryId, Country> = world.countries.keys()
            .map(|&tag| {
                let country = match sc.countries.iter().find(|c| c.tag == tag) {
                    Some(c) => Country {
                        production_queue: c.production.iter()
                            .map(|l| ProductionLine {
                                unit_type: l.unit_type.clone(),
                                assigned_factories: l.factories,
                                progress: l.progress,
                            })
                            .collect(),
                        ..Country::new(tag, c.manpower, c.civ_factories, c.mil_factories)
                    },
                    None => Country::new(tag, 100_000.0, 2, 1),
                };
                (tag, country)
            })
            .collect();
        for &(a, b) in &sc.wars {
            if let Some(c) = countries.get_mut(&a) {
                *c = c.declare_war_on(b);
            }
            if let Some(c) = countries.get_mut(&b) {
                *c = c.declare_war_on(a);
            }
        }

        let first_division_id = 101;
        let divisions: BTreeMap<DivisionId, Division> = sc.divisions.iter()
            .filter(|d| world.countries.contains_key(&d.country))
            .zip(first_division_id..)
            .map(|(d, id)| {
                let unit = world.unit_type(&d.unit_type).expect("已校验兵种");
                (id, Division::new(id, d.name.clone(), d.country, d.location, unit))
            })
            .collect();
        let next_division_id = first_division_id + divisions.len() as u32;

        let mut controllers: BTreeMap<ProvinceId, CountryId> =
            world.map.provinces.values().map(|p| (p.id, p.owner)).collect();
        for (&occupier, provs) in &sc.controllers {
            for &p in provs {
                controllers.insert(p, occupier);
            }
        }

        Ok(Self {
            controllers,
            countries,
            divisions,
            combats: BTreeMap::new(),
            outcome: None,
            date: sc.start_date,
            speed: 3,
            is_paused: true,
            world_tension: sc.world_tension,
            selected_province: world.capitals[&player_country],
            selected_division: None,
            player_country,
            logs: sc.intro_logs.clone(),
            next_division_id,
            next_combat_id: 1,
            world,
        })
    }

    pub fn controller(&self, province: ProvinceId) -> CountryId {
        self.controllers[&province]
    }

    /// 纯函数：计算某国家当前控制的胜利点总数
    pub fn total_victory_points(&self, country: CountryId) -> u32 {
        self.controllers.iter()
            .filter(|&(_, &c)| c == country)
            .map(|(p, _)| self.world.map.provinces[p].victory_points)
            .sum()
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
                let provinces = &self.world.map.provinces;
                let current = &provinces[&self.selected_province];
                let (lon0, lat0) = current.center;
                // 与目标方向夹角在 60° 以内的省份，按“距离 / 方向吻合度”评分；优先选邻接省份
                let score = |id: &ProvinceId| -> Option<f32> {
                    let (lon, lat) = provinces[id].center;
                    let (ex, ny) = ((lon - lon0) * lat0.to_radians().cos(), lat - lat0);
                    let len = ex.hypot(ny);
                    let (wx, wy) = (dx as f32, -dy as f32);
                    let cos = (ex * wx + ny * wy) / (len * wx.hypot(wy)).max(1e-6);
                    (len > 0.0 && cos > 0.5).then(|| len / cos)
                };
                let best = |ids: &mut dyn Iterator<Item = ProvinceId>| {
                    ids.filter_map(|id| score(&id).map(|sc| (sc, id)))
                        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
                        .map(|(_, id)| id)
                };
                let closest_id = best(&mut current.neighbors.iter().copied())
                    .or_else(|| best(&mut provinces.keys().copied()));

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
                if self.world.unit_type(&unit_type).is_none() {
                    return self;
                }
                let queued = self.countries[&self.player_country].queue_production(&unit_type);
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
            UserAction::DeclareWar(target) => {
                let player = self.player_country;
                self.declare_war(player, target)
            }
        }
    }

    /// 宣战：双方阵营成员自动参战。同阵营、已投降或已交战时不生效。
    pub fn declare_war(mut self, attacker: CountryId, target: CountryId) -> Self {
        let valid = |c: CountryId| self.countries.get(&c).is_some_and(|c| !c.has_capitulated);
        if !valid(attacker) || !valid(target) || self.world.are_allies(attacker, target)
            || self.countries[&attacker].is_at_war_with(target)
        {
            return self;
        }

        let side = |c: CountryId| -> Vec<CountryId> {
            let members = self.world.faction_of(c).map_or_else(|| vec![c], |f| f.members.clone());
            members.into_iter().filter(|&m| valid(m)).collect()
        };
        let (attackers, defenders) = (side(attacker), side(target));
        for &a in &attackers {
            for &d in &defenders {
                if let Some(c) = self.countries.get_mut(&a) {
                    *c = c.declare_war_on(d);
                }
                if let Some(c) = self.countries.get_mut(&d) {
                    *c = c.declare_war_on(a);
                }
            }
        }

        self.logs.push(format!(
            "⚡【世界大战】{} 对 {} 宣战！战火全面升级！",
            self.world.country_name(attacker),
            self.world.country_name(target)
        ));
        let joined: Vec<String> = defenders.iter()
            .filter(|&&d| d != target)
            .map(|&d| self.world.country_name(d))
            .collect();
        if !joined.is_empty() {
            self.logs.push(format!("【同盟】{} 履行盟约，一同参战！", joined.join("、")));
        }
        self.world_tension = (self.world_tension + 20.0).min(100.0);
        self
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
        let Some(path) = self.world.find_path(current_loc, target_prov) else {
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

        let distance_km = self.world.hop_km(current_loc, path[0]);
        if let Some(div) = self.divisions.get_mut(&div_id) {
            div.order = DivisionOrder::Moving {
                path,
                progress_km: 0.0,
                distance_km,
            };
        }
        // AI 调动频繁，只为玩家记录命令
        if self.divisions[&div_id].country == self.player_country {
            self.logs.push(format!(
                "【命令】{} 开始向 {} 进军！",
                name,
                self.world.province_name(target_prov)
            ));
        }

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
            let modifiers = self.world.combat_modifiers(combat.province);
            let report = combat.step(&self.divisions, modifiers, dt_hours);

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

            let world = Arc::clone(&self.world);
            let prov_name = world.province_name(combat.province);

            match outcome {
                CombatOutcome::AttackerVictory => {
                    self.logs.push(format!(
                        "★【战报大捷】攻方取得 {} 战役胜利！守军全线崩溃溃退！",
                        prov_name
                    ));

                    // 1. 变更控制权
                    self.controllers.insert(combat.province, combat.attacker_country);

                    // 2. 守军退却或包围歼灭 (Retreat / Encirclement)
                    let friendly_retreat_prov = world.map.provinces[&combat.province]
                        .neighbors
                        .iter()
                        .find(|&&n| self.controllers[&n] == combat.defender_country)
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
                                        distance_km: world.hop_km(combat.province, resume_path[0]),
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

        let world = Arc::clone(&self.world);
        for (id, div) in &mut self.divisions {
            if let DivisionOrder::Moving { ref mut path, ref mut progress_km, ref mut distance_km } = div.order {
                *progress_km += div.speed_kmh * dt_hours;

                if *progress_km >= *distance_km && !path.is_empty() {
                    // 到达下一跳，超出的里程计入下一段
                    let next_prov = path.remove(0);
                    *progress_km -= *distance_km;
                    if let Some(&after) = path.first() {
                        *distance_km = world.hop_km(next_prov, after);
                    }
                    arrivals.push((*id, next_prov, path.clone()));
                }
            }
        }

        // 处理到达事件
        for (div_id, next_prov, remaining_path) in arrivals {
            let div_country = self.divisions[&div_id].country;
            let prov_controller = self.controllers[&next_prov];

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
                let defender_country = self.controllers[&target_prov];

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

                // 只播报与玩家相关的战斗，避免全球战事刷屏
                if attacker_country == self.player_country || defender_country == self.player_country {
                    self.logs.push(format!(
                        "⚔【战斗爆发】在 {} 爆发殊死决战！",
                        self.world.province_name(target_prov)
                    ));
                }
            }
        }

        self
    }

    /// 新师团的部署地：首都；首都失守则退往仍控制的本土地块，再退往任一控制地块
    fn spawn_location(&self, country: CountryId) -> Option<ProvinceId> {
        let capital = self.world.capitals[&country];
        if self.controllers[&capital] == country {
            return Some(capital);
        }
        let controlled = || self.controllers.iter().filter(move |&(_, &c)| c == country).map(|(&p, _)| p);
        controlled()
            .find(|p| self.world.map.provinces[p].owner == country)
            .or_else(|| controlled().next())
    }

    /// 4. 推进工业、生产线与组织度/兵力自然恢复
    fn pipeline_advance_economy(mut self, dt_hours: f32) -> Self {
        let mut new_spawned_divisions = Vec::new();

        // 推进各国经济
        for country in self.countries.values_mut() {
            let (updated, completed_units) = country.step_economy(&self.world.unit_types, dt_hours);
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

            let Some(unit) = self.world.unit_type(&unit_type) else { continue };
            let name = format!("{}-第{}新编师", country_id, div_id % 100);
            let new_div = Division::new(div_id, name, country_id, spawn_loc, unit);
            self.divisions.insert(div_id, new_div);

            if country_id == self.player_country {
                self.logs.push(format!(
                    "🏭【军工生产】新编 {} 已在 {} 部署就绪！",
                    unit.name,
                    self.world.province_name(spawn_loc)
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
            let orig_vp = self.world.map.original_victory_points(country_id);
            let cur_vp = self.total_victory_points(country_id);

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
            self.world.country_name(loser)
        ));

        // 各占领国在其法理领土上占据的胜利点
        let mut occupier_vp: BTreeMap<CountryId, u32> = BTreeMap::new();
        for p in self.world.map.provinces.values().filter(|p| p.owner == loser) {
            let controller = self.controllers[&p.id];
            if controller != loser {
                *occupier_vp.entry(controller).or_default() += p.victory_points;
            }
        }
        let primary_occupier = occupier_vp.iter()
            .max_by_key(|&(_, &vp)| vp)
            .map(|(&c, _)| c)
            .or_else(|| self.countries[&loser].enemies.first().copied());

        // 剩余领土：优先交给接壤的占领国，否则交给主要占领国
        let remaining: Vec<ProvinceId> = self.controllers.iter()
            .filter(|&(_, &c)| c == loser)
            .map(|(&p, _)| p)
            .collect();
        for prov_id in remaining {
            let adjacent_occupier = self.world.map.provinces[&prov_id].neighbors.iter()
                .map(|n| self.controllers[n])
                .find(|c| occupier_vp.contains_key(c));
            if let Some(new_controller) = adjacent_occupier.or(primary_occupier) {
                self.controllers.insert(prov_id, new_controller);
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

    /// 玩家需要击败的对手：玩家阵营以外的列强；剧本未列出列强时为所有非盟国
    pub fn rivals(&self) -> Vec<CountryId> {
        let majors = &self.world.scenario.majors;
        let pool: Vec<CountryId> = if majors.is_empty() {
            self.countries.keys().copied().collect()
        } else {
            majors.iter().copied().filter(|m| self.countries.contains_key(m)).collect()
        };
        pool.into_iter().filter(|&c| !self.world.are_allies(c, self.player_country)).collect()
    }

    /// 6. 判定胜负：玩家投降则失败，玩家阵营以外的列强全部投降则胜利
    fn pipeline_check_outcome(mut self) -> Self {
        let rivals = self.rivals();
        let outcome = if self.countries[&self.player_country].has_capitulated {
            Some(GameOutcome::Defeat)
        } else if !rivals.is_empty() && rivals.iter().all(|c| self.countries[c].has_capitulated) {
            Some(GameOutcome::Victory)
        } else {
            None
        };

        if let Some(outcome) = outcome {
            self.logs.push(match outcome {
                GameOutcome::Victory => "🏆【终局】所有敌对列强均已投降，战争以我方全面胜利告终！".to_string(),
                GameOutcome::Defeat => "💀【终局】我国已投降，战争失败。".to_string(),
            });
            self.outcome = Some(outcome);
            self.is_paused = true;
        }
        self
    }

    /// 7. 纯函数战术 AI，每 6 小时调度一次：生产默认兵种，前线部队进攻接壤的敌方省份，后方空闲部队开赴最近的前线省份
    fn pipeline_ai_decisions(mut self) -> Self {
        if !self.date.hour.is_multiple_of(6) {
            return self;
        }

        let ai_countries: Vec<CountryId> = self.countries.values()
            .filter(|c| c.id != self.player_country && !c.has_capitulated && !c.enemies.is_empty())
            .map(|c| c.id)
            .collect();
        let Some(default_unit) = self.world.unit_types.first().map(|u| u.id.clone()) else {
            return self;
        };

        let mut ai_orders = Vec::new();

        for country_id in ai_countries {
            // 交战中的 AI 把空闲军工厂投入默认兵种生产
            let mut country = self.countries[&country_id].clone();
            while let Some(updated) = country.queue_production(&default_unit) {
                country = updated;
            }
            self.countries.insert(country_id, country);

            let enemies = &self.countries[&country_id].enemies;
            let provinces = &self.world.map.provinces;
            let hostile = |p: &ProvinceId| enemies.contains(&self.controllers[p]);
            let friendly = |p: &ProvinceId| self.world.are_allies(self.controllers[p], country_id);

            let idle_units: Vec<&Division> = self.divisions.values()
                .filter(|d| d.country == country_id && matches!(d.order, DivisionOrder::Idle))
                .collect();
            if idle_units.is_empty() {
                continue;
            }

            // 前线：与敌方接壤的友方省份；从前线出发在友方领土上做多源 BFS，得到每个省份通往前线的下一跳
            let front: Vec<ProvinceId> = self.controllers.keys()
                .filter(|p| friendly(p) && provinces[p].neighbors.iter().any(hostile))
                .copied()
                .collect();
            let mut toward_front: BTreeMap<ProvinceId, ProvinceId> = front.iter().map(|&p| (p, p)).collect();
            let mut queue: VecDeque<ProvinceId> = front.iter().copied().collect();
            while let Some(p) = queue.pop_front() {
                for &n in &provinces[&p].neighbors {
                    if friendly(&n) && !toward_front.contains_key(&n) {
                        toward_front.insert(n, p);
                        queue.push_back(n);
                    }
                }
            }

            // 同一目标的进攻分摊到不同省份：每个空闲部队选择已分配进攻部队最少的敌方邻省
            let mut assigned: BTreeMap<ProvinceId, usize> = BTreeMap::new();
            let front_set: BTreeSet<ProvinceId> = front.into_iter().collect();
            for unit in idle_units {
                let target = provinces[&unit.location].neighbors.iter()
                    .filter(|n| hostile(n))
                    .min_by_key(|&&n| (assigned.get(&n).copied().unwrap_or(0), n))
                    .copied();
                if let Some(target) = target {
                    *assigned.entry(target).or_default() += 1;
                    ai_orders.push((unit.id, target));
                } else if !front_set.contains(&unit.location) {
                    // 沿 BFS 树走到最近的前线省份
                    let mut dest = unit.location;
                    while let Some(&next) = toward_front.get(&dest) {
                        if next == dest {
                            break;
                        }
                        dest = next;
                    }
                    if dest != unit.location {
                        ai_orders.push((unit.id, dest));
                    }
                }
            }
        }

        for (unit_id, target) in ai_orders {
            self = self.order_unit_to_move(unit_id, target);
        }

        self
    }
}
