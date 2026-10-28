use crate::model::types::{CountryId, ProvinceId, Terrain};

#[derive(Debug, Clone, PartialEq)]
pub struct Province {
    pub id: ProvinceId,
    pub name: &'static str,
    pub original_owner: CountryId,
    pub controller: CountryId,
    pub terrain: Terrain,
    pub victory_points: u32,
    pub civ_factories: u32,
    pub mil_factories: u32,
    pub infrastructure: u32,
    pub neighbors: Vec<ProvinceId>,
    pub map_pos: (u16, u16), // 用于 TUI 地图网格绘制 (x, y)
}

impl Province {
    pub fn is_controlled_by(&self, country: CountryId) -> bool {
        self.controller == country
    }

    pub fn is_adjacent_to(&self, other: ProvinceId) -> bool {
        self.neighbors.contains(&other)
    }

    /// 函数式更新控制权
    pub fn with_controller(&self, new_controller: CountryId) -> Self {
        Self {
            controller: new_controller,
            ..self.clone()
        }
    }
}
