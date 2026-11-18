//! 由模组与剧本构建的静态世界：开局后不变，整局共享，前端启动时读取一次。

use std::collections::{BTreeMap, BTreeSet};

use crate::map::{Province, WorldMap};
use crate::modpack::{CountryDef, FactionDef, ModPack, ScenarioDef, TerrainDef, UnitTypeDef};
use crate::types::{CountryId, GameDate, ProvinceId};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScenarioInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub start_date: GameDate,
    pub factions: Vec<FactionDef>,
    pub majors: Vec<CountryId>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct World {
    pub scenario: ScenarioInfo,
    pub terrains: BTreeMap<String, TerrainDef>,
    /// 保持模组中的定义顺序：首个兵种为 AI 默认生产的兵种
    pub unit_types: Vec<UnitTypeDef>,
    pub countries: BTreeMap<CountryId, CountryDef>,
    pub capitals: BTreeMap<CountryId, ProvinceId>,
    pub map: WorldMap,
}

impl World {
    /// 由已校验的模组与剧本构建世界；邻接关系自动补全为双向
    pub fn build(pack: &ModPack, sc: &ScenarioDef) -> Self {
        let owner_of: BTreeMap<ProvinceId, CountryId> = sc.owners.iter()
            .flat_map(|(&tag, provs)| provs.iter().map(move |&p| (p, tag)))
            .collect();

        let mut neighbors: BTreeMap<ProvinceId, BTreeSet<ProvinceId>> = BTreeMap::new();
        for p in &pack.provinces {
            for &n in p.neighbors.iter().filter(|&&n| n != p.id) {
                neighbors.entry(p.id).or_default().insert(n);
                neighbors.entry(n).or_default().insert(p.id);
            }
        }

        let provinces = pack.provinces.iter()
            .map(|p| {
                let province = Province {
                    id: p.id,
                    name: p.name.clone(),
                    name_en: p.name_en.clone(),
                    owner: owner_of[&p.id],
                    terrain: p.terrain.clone(),
                    victory_points: p.victory_points,
                    civ_factories: p.civ_factories,
                    mil_factories: p.mil_factories,
                    infrastructure: p.infrastructure,
                    neighbors: neighbors.get(&p.id).map(|n| n.iter().copied().collect()).unwrap_or_default(),
                    center: p.center,
                };
                (p.id, province)
            })
            .collect();
        let map = WorldMap { provinces };

        // 在场国家：拥有领土的国家
        let countries: BTreeMap<CountryId, CountryDef> = pack.countries.iter()
            .filter(|c| sc.owners.get(&c.tag).is_some_and(|p| !p.is_empty()))
            .map(|c| (c.tag, c.clone()))
            .collect();

        let explicit_capital = |tag: CountryId| sc.countries.iter().find(|c| c.tag == tag).and_then(|c| c.capital);
        let capitals = countries.keys()
            .map(|&tag| {
                let capital = explicit_capital(tag).unwrap_or_else(|| {
                    // 缺省首都：胜利点最高的领土（同分取编号最小者）
                    sc.owners[&tag].iter()
                        .copied()
                        .max_by_key(|p| (map.provinces[p].victory_points, std::cmp::Reverse(*p)))
                        .expect("在场国家至少拥有一个省份")
                });
                (tag, capital)
            })
            .collect();

        Self {
            scenario: ScenarioInfo {
                id: sc.id.clone(),
                name: sc.name.clone(),
                description: sc.description.clone(),
                start_date: sc.start_date,
                factions: sc.factions.clone(),
                majors: sc.majors.clone(),
            },
            terrains: pack.terrains.iter().map(|t| (t.id.clone(), t.clone())).collect(),
            unit_types: pack.unit_types.clone(),
            countries,
            capitals,
            map,
        }
    }

    pub fn country_name(&self, tag: CountryId) -> String {
        self.countries.get(&tag).map_or_else(|| tag.to_string(), |c| c.name.clone())
    }

    pub fn province_name(&self, id: ProvinceId) -> &str {
        &self.map.provinces[&id].name
    }

    pub fn unit_type(&self, id: &str) -> Option<&UnitTypeDef> {
        self.unit_types.iter().find(|u| u.id == id)
    }

    pub fn faction_of(&self, tag: CountryId) -> Option<&FactionDef> {
        self.scenario.factions.iter().find(|f| f.members.contains(&tag))
    }

    /// 两国同属一个阵营
    pub fn are_allies(&self, a: CountryId, b: CountryId) -> bool {
        a == b || self.faction_of(a).is_some_and(|f| f.members.contains(&b))
    }

    /// 从 `from` 行军进入相邻省份 `to` 的有效距离：大圆距离 × 目标地形行军倍率
    pub fn hop_km(&self, from: ProvinceId, to: ProvinceId) -> f32 {
        let terrain = &self.map.provinces[&to].terrain;
        let cost = self.terrains.get(terrain).map_or(1.0, |t| t.movement_cost);
        self.map.distance_km(from, to) * cost
    }

    pub fn find_path(&self, start: ProvinceId, goal: ProvinceId) -> Option<Vec<ProvinceId>> {
        self.map.find_path(start, goal, |a, b| self.hop_km(a, b))
    }

    /// 地形的（攻方修正，守方修正）
    pub fn combat_modifiers(&self, province: ProvinceId) -> (f32, f32) {
        let terrain = &self.map.provinces[&province].terrain;
        self.terrains.get(terrain).map_or((1.0, 1.0), |t| (t.attack_modifier, t.defense_modifier))
    }
}
