use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::combat::{Combat, CombatOutcome};
use crate::country::Country;
use crate::division::{Division, DivisionId, DivisionOrder};
use crate::game_state::{GameOutcome, GameState, UserAction};
use crate::modpack::{ModLayer, ModPack};
use crate::types::{CountryId, GameDate, ProvinceId};

// europe-mini 模组中的省份编号
const BERLIN: ProvinceId = 1;
const COLOGNE: ProvinceId = 2;
const MUNICH: ProvinceId = 3;
const DANZIG: ProvinceId = 5;
const WARSAW: ProvinceId = 6;
const KRAKOW: ProvinceId = 7;

/// 柏林的第1装甲师（开局 id 101）
const PANZER_1: DivisionId = 101;

macro_rules! mod_file {
    ($path:literal) => {
        serde_json::from_str::<Value>(include_str!(concat!("../../../web/public/mods/", $path))).unwrap()
    };
}

fn mini_layers() -> Vec<ModLayer> {
    vec![
        ModLayer { id: "base".into(), files: vec![mod_file!("base/rules.json")] },
        ModLayer {
            id: "europe-mini".into(),
            files: vec![
                mod_file!("europe-mini/countries.json"),
                mod_file!("europe-mini/provinces.json"),
                mod_file!("europe-mini/scenarios/1939.json"),
            ],
        },
    ]
}

fn mini_pack() -> ModPack {
    ModPack::from_layers(&mini_layers()).unwrap()
}

fn mini() -> GameState {
    GameState::new(&mini_pack(), "1939", None).unwrap()
}

fn tag(s: &str) -> CountryId {
    CountryId::new(s).unwrap()
}

fn step_hours(state: GameState, hours: u32) -> GameState {
    (0..hours).fold(state, |s, _| s.step_pipeline(1.0))
}

fn json(state: &GameState) -> String {
    serde_json::to_string(state).unwrap()
}

#[test]
fn test_pathfinding_bfs_connectivity() {
    let state = mini();
    // 柏林到华沙寻路
    let path = state.world.find_path(BERLIN, WARSAW);
    assert!(path.is_some());
    let path = path.unwrap();
    assert_eq!(*path.last().unwrap(), WARSAW);
    assert!(path.contains(&DANZIG));
}

#[test]
fn test_pure_combat_resolution() {
    let mut divisions = BTreeMap::new();

    // 强力装甲师 vs 虚弱步兵师
    let pz_id: DivisionId = 1;
    let pack = mini_pack();
    let unit = |id: &str| pack.unit_types.iter().find(|u| u.id == id).unwrap();
    let pz = Division::new(pz_id, "神勇装甲师".to_string(), tag("GER"), DANZIG, unit("Panzer"));
    divisions.insert(pz_id, pz);

    let inf_id: DivisionId = 2;
    let mut inf = Division::new(inf_id, "虚弱守军".to_string(), tag("POL"), DANZIG, unit("Infantry"));
    inf.organization = 5.0; // 极低组织度
    divisions.insert(inf_id, inf);

    let combat = Combat::new(
        1,
        DANZIG,
        tag("GER"),
        tag("POL"),
        vec![pz_id],
        vec![inf_id],
    );

    let report = combat.step(&divisions, (1.0, 1.0), 10.0);
    assert_eq!(report.outcome, CombatOutcome::AttackerVictory);
    assert!(!report.damages.is_empty());
}

#[test]
fn test_pure_pipeline_state_advancement() {
    let initial = mini();
    let start_hour = initial.date.hour;

    // 步进 5 小时
    let advanced = initial.step_pipeline(5.0);
    assert_eq!(advanced.date.hour, (start_hour + 5) % 24);
}

#[test]
fn test_user_action_targeting_and_move() {
    let initial = mini();
    // 选中柏林
    let state = initial.handle_action(UserAction::SelectProvince(BERLIN));
    // 全军进攻但泽
    let state = state.handle_action(UserAction::OrderAllUnitsInProvince(BERLIN, DANZIG));

    // 检查柏林的部队是否下达了 Moving 指令
    let moving_count = state.divisions.values()
        .filter(|d| matches!(d.order, DivisionOrder::Moving { .. }))
        .count();
    assert!(moving_count > 0);
}

#[test]
fn test_country_capitulation_condition() {
    let mut state = mini();
    // 将波兰所有领土转给德国
    for p in state.world.map.provinces.values() {
        if p.owner == tag("POL") {
            state.controllers.insert(p.id, tag("GER"));
        }
    }
    // 执行检查投降流水线
    let stepped = state.step_pipeline(1.0);
    assert!(stepped.countries[&tag("POL")].has_capitulated);
}

#[test]
fn test_select_division_only_accepts_player_units() {
    let initial = mini();
    let own = initial.divisions.values()
        .find(|d| d.country == initial.player_country && d.location == MUNICH)
        .unwrap().id;
    let enemy = initial.divisions.values()
        .find(|d| d.country == tag("POL"))
        .unwrap().id;

    let state = initial.handle_action(UserAction::SelectDivision(own));
    assert_eq!(state.selected_division, Some(own));
    assert_eq!(state.selected_province, MUNICH);

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
    let mut state = mini();
    let player = state.player_country;
    let lines_before = state.countries[&player].production_queue.len();
    let free = state.countries[&player].free_factories();
    assert_eq!(free, 20, "德国 45 座军工厂中开局已分配 25 座");

    for _ in 0..10 {
        state = state.handle_action(UserAction::QueueProduction("Infantry".into()));
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
    let units = mini_pack().unit_types;
    let country = Country::new(tag("POL"), 0.0, 10, 10)
        .queue_production("Infantry")
        .unwrap();
    let cost = units.iter().find(|u| u.id == "Infantry").unwrap().stats;

    // 人力为零：产线停在完工状态，不会生产
    let mut stalled = country.clone();
    stalled.production_queue[0].progress = cost.production_cost - 0.01;
    let (stalled, done) = stalled.step_economy(&units, 1.0);
    assert!(done.is_empty());
    assert_eq!(stalled.production_queue[0].progress, cost.production_cost);

    // 人力到位后完工并扣除人力
    let funded = Country { manpower: 50_000.0, ..stalled };
    let (after, done) = funded.step_economy(&units, 1.0);
    assert_eq!(done, vec!["Infantry".to_string()]);
    assert!(after.manpower < 50_000.0 - cost.manpower_cost + 10.0);
}

#[test]
fn test_attacker_can_withdraw_but_defender_cannot() {
    let mut state = mini()
        .handle_action(UserAction::OrderAllUnitsInProvince(BERLIN, DANZIG));
    while !matches!(state.divisions[&PANZER_1].order, DivisionOrder::Fighting { .. }) {
        assert!(state.date.day < 3, "装甲师应在两天内抵达但泽开战");
        state = state.step_pipeline(1.0);
    }
    assert!(matches!(state.divisions[&PANZER_1].order, DivisionOrder::Fighting { is_attacker: true, .. }));

    // 攻方撤出战斗，改赴慕尼黑
    let state = state
        .handle_action(UserAction::SelectDivision(PANZER_1))
        .handle_action(UserAction::OrderMoveSelected(MUNICH));
    assert!(matches!(state.divisions[&PANZER_1].order, DivisionOrder::Moving { .. }));
    assert!(state.combats.values().all(|c| !c.attackers.contains(&PANZER_1)));

    // 守方被牵制，命令被拒绝
    let mut state = state;
    let defending = DivisionOrder::Fighting {
        combat_province: BERLIN,
        is_attacker: false,
        resume_path: Vec::new(),
    };
    state.divisions.get_mut(&PANZER_1).unwrap().order = defending.clone();
    let state = state.handle_action(UserAction::OrderMoveSelected(COLOGNE));
    assert_eq!(state.divisions[&PANZER_1].order, defending);
    assert!(state.logs.last().unwrap().contains("无法脱离战斗"));
}

#[test]
fn test_attackers_continue_along_their_path_after_victory() {
    let mut state = mini();
    // 但泽无人防守：攻方必胜
    state.divisions.retain(|_, d| d.location != DANZIG);
    let mut state = state
        .handle_action(UserAction::SelectDivision(PANZER_1))
        .handle_action(UserAction::OrderMoveSelected(WARSAW));

    for _ in 0..80 {
        state = state.step_pipeline(1.0);
        if state.divisions[&PANZER_1].location == DANZIG {
            break;
        }
    }
    assert_eq!(state.controller(DANZIG), tag("GER"));
    assert!(matches!(
        &state.divisions[&PANZER_1].order,
        DivisionOrder::Moving { path, distance_km, .. }
            if path == &vec![WARSAW] && (*distance_km - state.world.hop_km(DANZIG, WARSAW)).abs() < 1e-3
    ));
}

#[test]
fn test_new_divisions_avoid_an_occupied_capital() {
    let mut state = mini();
    state.controllers.insert(BERLIN, tag("POL"));
    let cost = state.world.unit_type("Infantry").unwrap().stats.production_cost;
    let germany = state.countries.get_mut(&tag("GER")).unwrap();
    assert_eq!(germany.production_queue[1].unit_type, "Infantry");
    germany.production_queue[1].progress = cost - 0.01;
    let before: Vec<DivisionId> = state.divisions.keys().copied().collect();

    let state = state.step_pipeline(1.0);
    let spawned: Vec<&Division> = state.divisions.values().filter(|d| !before.contains(&d.id)).collect();
    assert_eq!(spawned.len(), 1);
    assert_eq!(spawned[0].country, tag("GER"));
    assert_ne!(spawned[0].location, BERLIN);
    assert_eq!(state.controller(spawned[0].location), tag("GER"));
}

#[test]
fn test_capitulation_partitions_land_disbands_army_and_ends_war() {
    // 克拉科夫只占波兰胜利点的一小部分
    let mut pack = mini_pack();
    pack.provinces.iter_mut().find(|p| p.id == KRAKOW).unwrap().victory_points = 5;
    let mut state = GameState::new(&pack, "1939", None).unwrap();
    state.controllers.insert(WARSAW, tag("GER"));
    state.controllers.insert(DANZIG, tag("SOV"));

    let state = state.step_pipeline(1.0);
    let poland = &state.countries[&tag("POL")];
    assert!(poland.has_capitulated);
    assert!(poland.enemies.is_empty());
    // 克拉科夫交给接壤的占领国
    assert_eq!(state.controller(KRAKOW), tag("GER"));
    assert!(state.divisions.values().all(|d| d.country != tag("POL")));
    assert!(!state.countries[&tag("GER")].is_at_war_with(tag("POL")));
    assert!(state.combats.values().all(|c| c.attacker_country != tag("POL") && c.defender_country != tag("POL")));
    assert_eq!(state.outcome, None);
}

#[test]
fn test_player_can_lose_and_the_game_stops() {
    let mut state = mini().handle_action(UserAction::TogglePause);
    for p in state.world.map.provinces.values() {
        if p.owner == tag("GER") {
            state.controllers.insert(p.id, tag("POL"));
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
    let mut state = mini();
    for country in state.countries.values_mut() {
        if country.id != tag("GER") {
            *country = country.capitulated();
        }
    }
    let state = state.step_pipeline(1.0);
    assert_eq!(state.outcome, Some(GameOutcome::Victory));
}

#[test]
fn test_ai_at_war_puts_free_factories_to_work() {
    let state = mini();
    assert!(state.countries[&tag("POL")].production_queue.is_empty());
    // 开局 06:00，推进到 12:00 触发 AI 调度
    let state = step_hours(state, 6);
    let poland = &state.countries[&tag("POL")];
    assert!(!poland.production_queue.is_empty());
    assert_eq!(poland.free_factories(), 0);
    // 未参战的法国不扩军
    assert!(state.countries[&tag("FRA")].production_queue.is_empty());
}

#[test]
fn test_simulation_is_deterministic_and_step_size_independent() {
    let opening = || {
        mini()
            .handle_action(UserAction::OrderAllUnitsInProvince(BERLIN, DANZIG))
            .handle_action(UserAction::OrderAllUnitsInProvince(MUNICH, KRAKOW))
            .handle_action(UserAction::DeclareWar(tag("FRA")))
    };

    let hourly = step_hours(opening(), 48);
    assert_eq!(json(&hourly), json(&step_hours(opening(), 48)));
    assert_eq!(json(&hourly), json(&opening().step_pipeline(48.0)));
}

/* ───────────── 模组系统 ───────────── */

#[test]
fn test_country_tags_are_validated() {
    assert_eq!(tag("GER").as_str(), "GER");
    assert_eq!(tag("D").as_str(), "D");
    assert!(CountryId::new("").is_none());
    assert!(CountryId::new("GERM").is_none());
    assert!(CountryId::new("ger").is_none());
    assert_eq!(serde_json::to_string(&tag("SOV")).unwrap(), "\"SOV\"");
}

#[test]
fn test_later_layers_patch_replace_and_delete_entries() {
    let mut layers = mini_layers();
    layers.push(ModLayer {
        id: "patch".into(),
        files: vec![json!({
            // 只改动一个字段，其余字段保留
            "provinces": [{ "id": KRAKOW, "terrain": "Mountain" }],
            "unit_types": [
                { "id": "Motorized", "_delete": true },
                { "id": "Cavalry", "name": "骑兵师", "icon": "[Cav]", "max_organization": 50.0,
                  "soft_attack": 20.0, "hard_attack": 2.0, "defense": 20.0, "breakthrough": 10.0,
                  "speed_kmh": 7.0, "production_cost": 40.0, "manpower_cost": 8000.0 }
            ],
            "countries": [{ "tag": "POL", "color": "#ffffff" }]
        })],
    });
    let pack = ModPack::from_layers(&layers).unwrap();
    let krakow = pack.provinces.iter().find(|p| p.id == KRAKOW).unwrap();
    assert_eq!(krakow.terrain, "Mountain");
    assert_eq!(krakow.name, "克拉科夫");
    let units: Vec<&str> = pack.unit_types.iter().map(|u| u.id.as_str()).collect();
    assert_eq!(units, ["Infantry", "Panzer", "Cavalry"]);
    let poland = pack.countries.iter().find(|c| c.tag == tag("POL")).unwrap();
    assert_eq!((poland.name.as_str(), poland.color.as_str()), ("波兰共和国", "#ffffff"));

    // 删除摩托化师后，引用它的开局师团无法通过校验
    let errors = GameState::new(&pack, "1939", None).err().unwrap();
    assert!(errors.iter().any(|e| e.contains("Motorized")), "{errors:?}");
}

#[test]
fn test_validation_reports_broken_references() {
    let mut layers = mini_layers();
    layers.push(ModLayer {
        id: "broken".into(),
        files: vec![json!({
            "provinces": [{ "id": BERLIN, "terrain": "Lava", "neighbors": [999] }],
            "scenarios": [{ "id": "1939", "player": "XYZ" }]
        })],
    });
    let pack = ModPack::from_layers(&layers).unwrap();
    let errors = GameState::new(&pack, "1939", None).err().unwrap();
    let has = |needle: &str| errors.iter().any(|e| e.contains(needle));
    assert!(has("Lava") && has("999") && has("XYZ"), "{errors:?}");

    assert!(GameState::new(&mini_pack(), "nope", None).is_err());
    let unknown_section = ModLayer { id: "x".into(), files: vec![json!({ "provinsces": [] })] };
    assert!(ModPack::from_layers(&[unknown_section]).is_err());
}

#[test]
fn test_any_country_with_territory_is_playable() {
    let state = GameState::new(&mini_pack(), "1939", Some(tag("FRA"))).unwrap();
    assert_eq!(state.player_country, tag("FRA"));
    assert_eq!(state.selected_province, 8, "选中法国首都巴黎");
    assert!(GameState::new(&mini_pack(), "1939", Some(tag("ITA"))).is_err());
}

#[test]
fn test_neighbors_are_symmetric_even_if_listed_once() {
    let mut pack = mini_pack();
    // 只在一侧声明伦敦—加来的新连线
    pack.provinces.iter_mut().find(|p| p.id == 11).unwrap().neighbors.push(10);
    let state = GameState::new(&pack, "1939", None).unwrap();
    assert!(state.world.map.provinces[&10].neighbors.contains(&11));
}

#[test]
fn test_declaring_war_pulls_in_both_factions() {
    let mut pack = mini_pack();
    let sc = &mut pack.scenarios[0];
    sc.factions = serde_json::from_value(json!([
        { "id": "allies", "name": "同盟国", "members": ["FRA", "ENG"] }
    ])).unwrap();
    let state = GameState::new(&pack, "1939", None).unwrap()
        .handle_action(UserAction::DeclareWar(tag("FRA")));
    let germany = &state.countries[&tag("GER")];
    assert!(germany.is_at_war_with(tag("FRA")) && germany.is_at_war_with(tag("ENG")));
    assert!(!germany.is_at_war_with(tag("SOV")));
    assert!(state.countries[&tag("ENG")].is_at_war_with(tag("GER")));

    // 同盟国成员之间不能互相宣战
    let state = GameState::new(&pack, "1939", Some(tag("FRA"))).unwrap()
        .handle_action(UserAction::DeclareWar(tag("ENG")));
    assert!(state.countries[&tag("FRA")].enemies.is_empty());
}

#[test]
fn test_victory_only_requires_rival_majors() {
    let mut pack = mini_pack();
    let sc = &mut pack.scenarios[0];
    sc.majors = vec![tag("GER"), tag("FRA")];
    let mut state = GameState::new(&pack, "1939", None).unwrap();
    assert_eq!(state.rivals(), vec![tag("FRA")]);
    let france = state.countries.get_mut(&tag("FRA")).unwrap();
    *france = france.capitulated();
    assert_eq!(state.step_pipeline(1.0).outcome, Some(GameOutcome::Victory));
}

#[test]
fn test_idle_ai_units_march_to_the_front() {
    // 莫斯科的苏军远离前线；对德宣战后应开赴与德国（经波兰）接壤的省份
    let state = mini()
        .handle_action(UserAction::DeclareWar(tag("SOV")));
    let moscow_units: Vec<DivisionId> = state.divisions.values()
        .filter(|d| d.location == 16)
        .map(|d| d.id)
        .collect();
    assert!(!moscow_units.is_empty());
    let state = step_hours(state, 6);
    for id in moscow_units {
        assert!(
            matches!(state.divisions[&id].order, DivisionOrder::Moving { .. }),
            "{id} 应已出发前往前线"
        );
    }
}

#[test]
fn test_navigation_follows_compass_directions() {
    let state = mini().handle_action(UserAction::NavigateMap(1, 0));
    // 柏林以东的邻接省份是但泽
    assert_eq!(state.selected_province, DANZIG);
    let state = state.handle_action(UserAction::NavigateMap(-1, 0));
    assert_eq!(state.selected_province, BERLIN);
}

#[test]
fn test_pathfinding_prefers_shorter_real_distance() {
    let state = mini();
    let path = state.world.find_path(BERLIN, KRAKOW).unwrap();
    assert_eq!(path.last(), Some(&KRAKOW));
    let length: f32 = std::iter::once(BERLIN).chain(path.iter().copied())
        .zip(path.iter().copied())
        .map(|(a, b)| state.world.hop_km(a, b))
        .sum();
    assert!(length < 1500.0, "{path:?} {length}");
}

#[test]
fn test_scenarios_can_start_with_occupied_provinces() {
    let mut pack = mini_pack();
    pack.scenarios[0].controllers = BTreeMap::from([(tag("GER"), vec![DANZIG])]);
    let state = GameState::new(&pack, "1939", None).unwrap();
    assert_eq!(state.controller(DANZIG), tag("GER"));
    assert_eq!(state.world.map.provinces[&DANZIG].owner, tag("POL"));

    pack.scenarios[0].controllers = BTreeMap::from([(tag("ITA"), vec![999])]);
    let errors = GameState::new(&pack, "1939", None).err().unwrap();
    assert!(errors.iter().any(|e| e.contains("ITA")) && errors.iter().any(|e| e.contains("999")), "{errors:?}");
}

/* ───────────── world-1936 模组 ───────────── */

fn world_pack() -> ModPack {
    ModPack::from_layers(&[
        ModLayer { id: "base".into(), files: vec![mod_file!("base/rules.json")] },
        ModLayer {
            id: "world-1936".into(),
            files: vec![
                mod_file!("world-1936/countries.json"),
                mod_file!("world-1936/provinces.json"),
                mod_file!("world-1936/scenarios/1936.json"),
            ],
        },
    ])
    .unwrap()
}

#[test]
fn test_world_1936_pack_is_valid_and_playable() {
    let pack = world_pack();
    assert!(pack.provinces.len() >= 900, "约 1000 个省份，实际 {}", pack.provinces.len());
    assert_eq!(pack.validate(pack.scenario("1936")), Vec::<String>::new());

    let state = GameState::new(&pack, "1936", None).unwrap();
    // 每个国家都有首都，且开局控制自己的首都
    for (tag, &capital) in &state.world.capitals {
        assert_eq!(state.world.map.provinces[&capital].owner, *tag);
    }
    // 柏林能走到维也纳、布拉格、巴黎、华沙与莫斯科
    let berlin = state.world.capitals[&tag("GER")];
    for target in ["AUT", "CZE", "FRA", "POL", "SOV"] {
        let goal = state.world.capitals[&tag(target)];
        assert!(state.world.find_path(berlin, goal).is_some(), "柏林无法到达 {target}");
    }
    // 意军开局已占领埃塞俄比亚北部
    let occupied = |s: &GameState| {
        s.world.map.provinces.values()
            .filter(|p| p.owner == tag("ETH") && s.controller(p.id) == tag("ITA"))
            .count()
    };
    let at_start = occupied(&state);
    assert!(at_start > 0, "意军开局应占领提格雷");

    // 由中立的苏联观战两周：意大利 AI 继续进攻埃塞俄比亚
    let state = GameState::new(&pack, "1936", Some(tag("SOV"))).unwrap();
    let state = step_hours(state, 24 * 14);
    assert!(occupied(&state) > at_start, "两周后意军应攻占更多埃塞俄比亚省份");
}
