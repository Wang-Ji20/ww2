use std::collections::BTreeMap;

use crate::combat::{Combat, CombatOutcome};
use crate::country::Country;
use crate::division::{Division, DivisionId, DivisionOrder};
use crate::game_state::{GameOutcome, GameState, UserAction};
use crate::map::WorldMap;
use crate::types::{CountryId, GameDate, ProvinceId, Terrain, UnitType};

/// 柏林的第1装甲师（开局 id 101）
const PANZER_1: DivisionId = 101;

fn step_hours(state: GameState, hours: u32) -> GameState {
    (0..hours).fold(state, |s, _| s.step_pipeline(1.0))
}

fn json(state: &GameState) -> String {
    serde_json::to_string(state).unwrap()
}

#[test]
fn test_pathfinding_bfs_connectivity() {
    let map = WorldMap::new_europe_1939();
    // 柏林到华沙寻路
    let path = map.find_path(ProvinceId::Berlin, ProvinceId::Warsaw);
    assert!(path.is_some());
    let path = path.unwrap();
    assert_eq!(*path.last().unwrap(), ProvinceId::Warsaw);
    assert!(path.contains(&ProvinceId::Danzig));
}

#[test]
fn test_pure_combat_resolution() {
    let mut divisions = BTreeMap::new();

    // 强力装甲师 vs 虚弱步兵师
    let pz_id: DivisionId = 1;
    let pz = Division::new(pz_id, "神勇装甲师".to_string(), CountryId::Germany, ProvinceId::Danzig, UnitType::Panzer);
    divisions.insert(pz_id, pz);

    let inf_id: DivisionId = 2;
    let mut inf = Division::new(inf_id, "虚弱守军".to_string(), CountryId::Poland, ProvinceId::Danzig, UnitType::Infantry);
    inf.organization = 5.0; // 极低组织度
    divisions.insert(inf_id, inf);

    let combat = Combat::new(
        1,
        ProvinceId::Danzig,
        CountryId::Germany,
        CountryId::Poland,
        vec![pz_id],
        vec![inf_id],
    );

    let report = combat.step(&divisions, Terrain::Plains, 10.0);
    assert_eq!(report.outcome, CombatOutcome::AttackerVictory);
    assert!(!report.damages.is_empty());
}

#[test]
fn test_pure_pipeline_state_advancement() {
    let initial = GameState::new_1939_scenario();
    let start_hour = initial.date.hour;

    // 步进 5 小时
    let advanced = initial.step_pipeline(5.0);
    assert_eq!(advanced.date.hour, (start_hour + 5) % 24);
}

#[test]
fn test_user_action_targeting_and_move() {
    let initial = GameState::new_1939_scenario();
    // 选中柏林
    let state = initial.handle_action(UserAction::SelectProvince(ProvinceId::Berlin));
    // 全军进攻但泽
    let state = state.handle_action(UserAction::OrderAllUnitsInProvince(ProvinceId::Berlin, ProvinceId::Danzig));

    // 检查柏林的部队是否下达了 Moving 指令
    let moving_count = state.divisions.values()
        .filter(|d| matches!(d.order, DivisionOrder::Moving { .. }))
        .count();
    assert!(moving_count > 0);
}

#[test]
fn test_country_capitulation_condition() {
    let mut state = GameState::new_1939_scenario();
    // 将波兰所有领土转给德国
    for prov in state.map.provinces.values_mut() {
        if prov.original_owner == CountryId::Poland {
            prov.controller = CountryId::Germany;
        }
    }
    // 执行检查投降流水线
    let stepped = state.step_pipeline(1.0);
    assert!(stepped.countries[&CountryId::Poland].has_capitulated);
}

#[test]
fn test_select_division_only_accepts_player_units() {
    let initial = GameState::new_1939_scenario();
    let own = initial.divisions.values()
        .find(|d| d.country == initial.player_country && d.location == ProvinceId::Munich)
        .unwrap().id;
    let enemy = initial.divisions.values()
        .find(|d| d.country == CountryId::Poland)
        .unwrap().id;

    let state = initial.handle_action(UserAction::SelectDivision(own));
    assert_eq!(state.selected_division, Some(own));
    assert_eq!(state.selected_province, ProvinceId::Munich);

    let state = state.handle_action(UserAction::SelectDivision(enemy));
    assert_eq!(state.selected_division, Some(own));
}

#[test]
fn test_leap_years() {
    let feb_28 = |year| GameDate { year, month: 2, day: 28, hour: 23 };
    assert_eq!(feb_28(1940).advance_hours(1), GameDate { year: 1940, month: 2, day: 29, hour: 0 });
    assert_eq!(feb_28(1941).advance_hours(1), GameDate { year: 1941, month: 3, day: 1, hour: 0 });
    assert_eq!(feb_28(1900).advance_hours(1), GameDate { year: 1900, month: 3, day: 1, hour: 0 });
}

#[test]
fn test_production_is_limited_by_free_factories() {
    let mut state = GameState::new_1939_scenario();
    let player = state.player_country;
    let lines_before = state.countries[&player].production_queue.len();
    let free = state.countries[&player].free_factories();
    assert_eq!(free, 20, "德国 45 座军工厂中开局已分配 25 座");

    for _ in 0..10 {
        state = state.handle_action(UserAction::QueueProduction(UnitType::Infantry));
    }
    let germany = &state.countries[&player];
    assert_eq!(germany.production_queue.len(), lines_before + 4);
    assert_eq!(germany.free_factories(), 0);
    assert!(state.logs.last().unwrap().contains("军工厂均已分配"));

    // 撤销一条生产线即可释放军工厂
    let state = state.handle_action(UserAction::CancelProduction(0));
    assert_eq!(state.countries[&player].free_factories(), 15);
}

#[test]
fn test_production_consumes_manpower_and_stalls_without_it() {
    let country = Country::new(CountryId::Poland, 0.0, 10, 10)
        .queue_production(UnitType::Infantry)
        .unwrap();
    let cost = UnitType::Infantry.base_stats();

    // 人力为零：产线停在完工状态，不会生产
    let mut stalled = country.clone();
    stalled.production_queue[0].progress = cost.production_cost - 0.01;
    let (stalled, units) = stalled.step_economy(1.0);
    assert!(units.is_empty());
    assert_eq!(stalled.production_queue[0].progress, cost.production_cost);

    // 人力到位后完工并扣除人力
    let funded = Country { manpower: 50_000.0, ..stalled };
    let (after, units) = funded.step_economy(1.0);
    assert_eq!(units, vec![UnitType::Infantry]);
    assert!(after.manpower < 50_000.0 - cost.manpower_cost + 10.0);
}

#[test]
fn test_attacker_can_withdraw_but_defender_cannot() {
    let mut state = GameState::new_1939_scenario()
        .handle_action(UserAction::OrderAllUnitsInProvince(ProvinceId::Berlin, ProvinceId::Danzig));
    while !matches!(state.divisions[&PANZER_1].order, DivisionOrder::Fighting { .. }) {
        assert!(state.date.day < 3, "装甲师应在一天内抵达但泽开战");
        state = state.step_pipeline(1.0);
    }
    assert!(matches!(state.divisions[&PANZER_1].order, DivisionOrder::Fighting { is_attacker: true, .. }));

    // 攻方撤出战斗，改赴慕尼黑
    let state = state
        .handle_action(UserAction::SelectDivision(PANZER_1))
        .handle_action(UserAction::OrderMoveSelected(ProvinceId::Munich));
    assert!(matches!(state.divisions[&PANZER_1].order, DivisionOrder::Moving { .. }));
    assert!(state.combats.values().all(|c| !c.attackers.contains(&PANZER_1)));

    // 守方被牵制，命令被拒绝
    let mut state = state;
    let defending = DivisionOrder::Fighting {
        combat_province: ProvinceId::Berlin,
        is_attacker: false,
        resume_path: Vec::new(),
    };
    state.divisions.get_mut(&PANZER_1).unwrap().order = defending.clone();
    let state = state.handle_action(UserAction::OrderMoveSelected(ProvinceId::Cologne));
    assert_eq!(state.divisions[&PANZER_1].order, defending);
    assert!(state.logs.last().unwrap().contains("无法脱离战斗"));
}

#[test]
fn test_attackers_continue_along_their_path_after_victory() {
    let mut state = GameState::new_1939_scenario();
    // 但泽无人防守：攻方必胜
    state.divisions.retain(|_, d| d.location != ProvinceId::Danzig);
    let mut state = state
        .handle_action(UserAction::SelectDivision(PANZER_1))
        .handle_action(UserAction::OrderMoveSelected(ProvinceId::Warsaw));

    for _ in 0..30 {
        state = state.step_pipeline(1.0);
        if state.divisions[&PANZER_1].location == ProvinceId::Danzig {
            break;
        }
    }
    assert_eq!(state.map.provinces[&ProvinceId::Danzig].controller, CountryId::Germany);
    assert!(matches!(
        &state.divisions[&PANZER_1].order,
        DivisionOrder::Moving { path, .. } if path == &vec![ProvinceId::Warsaw]
    ));
}

#[test]
fn test_new_divisions_avoid_an_occupied_capital() {
    let mut state = GameState::new_1939_scenario();
    state.map.provinces.get_mut(&ProvinceId::Berlin).unwrap().controller = CountryId::Poland;
    let germany = state.countries.get_mut(&CountryId::Germany).unwrap();
    let cost = germany.production_queue[1].unit_type.base_stats().production_cost;
    germany.production_queue[1].progress = cost - 0.01;
    let before: Vec<DivisionId> = state.divisions.keys().copied().collect();

    let state = state.step_pipeline(1.0);
    let spawned: Vec<&Division> = state.divisions.values().filter(|d| !before.contains(&d.id)).collect();
    assert_eq!(spawned.len(), 1);
    assert_eq!(spawned[0].country, CountryId::Germany);
    assert_ne!(spawned[0].location, ProvinceId::Berlin);
    assert_eq!(state.map.provinces[&spawned[0].location].controller, CountryId::Germany);
}

#[test]
fn test_capitulation_partitions_land_disbands_army_and_ends_war() {
    let mut state = GameState::new_1939_scenario();
    let set_controller = |s: &mut GameState, p: ProvinceId, c: CountryId| {
        s.map.provinces.get_mut(&p).unwrap().controller = c;
    };
    set_controller(&mut state, ProvinceId::Warsaw, CountryId::Germany);
    set_controller(&mut state, ProvinceId::Danzig, CountryId::SovietUnion);
    state.map.provinces.get_mut(&ProvinceId::Krakow).unwrap().victory_points = 5;

    let state = state.step_pipeline(1.0);
    let poland = &state.countries[&CountryId::Poland];
    assert!(poland.has_capitulated);
    assert!(poland.enemies.is_empty());
    // 克拉科夫交给接壤的占领国
    assert_eq!(state.map.provinces[&ProvinceId::Krakow].controller, CountryId::Germany);
    assert!(state.divisions.values().all(|d| d.country != CountryId::Poland));
    assert!(!state.countries[&CountryId::Germany].is_at_war_with(CountryId::Poland));
    assert!(state.combats.values().all(|c| c.attacker_country != CountryId::Poland && c.defender_country != CountryId::Poland));
    assert_eq!(state.outcome, None);
}

#[test]
fn test_player_can_lose_and_the_game_stops() {
    let mut state = GameState::new_1939_scenario().handle_action(UserAction::TogglePause);
    for prov in state.map.provinces.values_mut() {
        if prov.original_owner == CountryId::Germany {
            prov.controller = CountryId::Poland;
        }
    }
    let state = state.step_pipeline(1.0);
    assert_eq!(state.outcome, Some(GameOutcome::Defeat));
    assert!(state.is_paused);

    let date = state.date;
    let state = state.handle_action(UserAction::TogglePause).step_pipeline(5.0);
    assert!(state.is_paused);
    assert_eq!(state.date, date, "对局结束后时间不再推进");
}

#[test]
fn test_player_wins_when_every_other_power_capitulates() {
    let mut state = GameState::new_1939_scenario();
    for country in state.countries.values_mut() {
        if country.id != CountryId::Germany {
            *country = country.capitulated();
        }
    }
    let state = state.step_pipeline(1.0);
    assert_eq!(state.outcome, Some(GameOutcome::Victory));
}

#[test]
fn test_ai_at_war_puts_free_factories_to_work() {
    let state = GameState::new_1939_scenario();
    assert!(state.countries[&CountryId::Poland].production_queue.is_empty());
    // 开局 06:00，推进到 12:00 触发 AI 调度
    let state = step_hours(state, 6);
    let poland = &state.countries[&CountryId::Poland];
    assert!(!poland.production_queue.is_empty());
    assert_eq!(poland.free_factories(), 0);
    // 未参战的法国不扩军
    assert!(state.countries[&CountryId::France].production_queue.is_empty());
}

#[test]
fn test_simulation_is_deterministic_and_step_size_independent() {
    let opening = || {
        GameState::new_1939_scenario()
            .handle_action(UserAction::OrderAllUnitsInProvince(ProvinceId::Berlin, ProvinceId::Danzig))
            .handle_action(UserAction::OrderAllUnitsInProvince(ProvinceId::Munich, ProvinceId::Krakow))
            .handle_action(UserAction::DeclareWarNextEnemy)
    };

    let hourly = step_hours(opening(), 48);
    assert_eq!(json(&hourly), json(&step_hours(opening(), 48)));
    assert_eq!(json(&hourly), json(&opening().step_pipeline(48.0)));
}
