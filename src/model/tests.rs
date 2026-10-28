#[cfg(test)]
mod tests {
    use crate::model::combat::{Combat, CombatOutcome};
    use crate::model::division::{Division, DivisionId, DivisionOrder};
    use crate::model::game_state::{GameState, UserAction};
    use crate::model::map::WorldMap;
    use crate::model::types::{CountryId, ProvinceId, Terrain, UnitType};
    use std::collections::HashMap;

    #[test]
    fn test_pathfinding_bfs_connectivity() {
        let map = WorldMap::new_europe_1939();
        // 柏林到华沙寻路
        let path = map.find_path(ProvinceId::Berlin, ProvinceId::Warsaw, |_| true);
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(*path.last().unwrap(), ProvinceId::Warsaw);
        assert!(path.contains(&ProvinceId::Danzig));
    }

    #[test]
    fn test_pure_combat_resolution() {
        let _map = WorldMap::new_europe_1939();
        let mut divisions = HashMap::new();

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
}
