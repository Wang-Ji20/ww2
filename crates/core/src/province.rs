use crate::types::{CountryId, ProvinceId, Terrain};

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
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
    pub map_pos: (u16, u16), // 地图坐标 (x, y)，前端据此布局节点
}

impl Province {
    /// 函数式更新控制权
    pub fn with_controller(&self, new_controller: CountryId) -> Self {
        Self {
            controller: new_controller,
            ..self.clone()
        }
    }
}
