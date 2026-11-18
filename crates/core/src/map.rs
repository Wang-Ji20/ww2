use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

use crate::types::{CountryId, ProvinceId, TerrainId};

/// 地球平均半径（公里）
const EARTH_RADIUS_KM: f32 = 6371.0;

/// 静态省份数据：开局后不再改变（控制权记录在 GameState 中）
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Province {
    pub id: ProvinceId,
    pub name: String,
    pub name_en: String,
    /// 剧本开局时的法理所有者
    pub owner: CountryId,
    pub terrain: TerrainId,
    pub victory_points: u32,
    pub civ_factories: u32,
    pub mil_factories: u32,
    pub infrastructure: u32,
    pub neighbors: Vec<ProvinceId>,
    /// [经度, 纬度]
    pub center: (f32, f32),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorldMap {
    pub provinces: BTreeMap<ProvinceId, Province>,
}

/// A* 优先队列节点：按估计总代价从小到大弹出
#[derive(PartialEq)]
struct Frontier {
    estimate: f32,
    province: ProvinceId,
}

impl Eq for Frontier {}

impl Ord for Frontier {
    fn cmp(&self, other: &Self) -> Ordering {
        other.estimate.total_cmp(&self.estimate).then(other.province.cmp(&self.province))
    }
}

impl PartialOrd for Frontier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl WorldMap {
    /// 两省中心之间的大圆距离（公里）
    pub fn distance_km(&self, a: ProvinceId, b: ProvinceId) -> f32 {
        haversine_km(self.provinces[&a].center, self.provinces[&b].center)
    }

    /// 纯函数：A* 寻路，代价为行军距离（含地形倍率 `hop_cost`）。
    /// 返回不含起点的路径；起点即终点时返回 `[start]`。
    pub fn find_path(
        &self,
        start: ProvinceId,
        goal: ProvinceId,
        hop_cost: impl Fn(ProvinceId, ProvinceId) -> f32,
    ) -> Option<Vec<ProvinceId>> {
        if start == goal {
            return Some(vec![start]);
        }
        let goal_center = self.provinces.get(&goal)?.center;
        let heuristic = |p: ProvinceId| haversine_km(self.provinces[&p].center, goal_center);

        let mut best: BTreeMap<ProvinceId, f32> = BTreeMap::from([(start, 0.0)]);
        let mut came_from: BTreeMap<ProvinceId, ProvinceId> = BTreeMap::new();
        let mut open = BinaryHeap::from([Frontier { estimate: heuristic(start), province: start }]);

        while let Some(Frontier { estimate, province: current }) = open.pop() {
            if current == goal {
                let mut path = vec![goal];
                let mut curr = goal;
                while let Some(&prev) = came_from.get(&curr) {
                    if prev == start {
                        break;
                    }
                    path.push(prev);
                    curr = prev;
                }
                path.reverse();
                return Some(path);
            }
            let cost_here = best[&current];
            // 过期的队列项
            if estimate > cost_here + heuristic(current) + 1e-3 {
                continue;
            }
            for &next in &self.provinces[&current].neighbors {
                let cost = cost_here + hop_cost(current, next);
                if best.get(&next).is_none_or(|&known| cost < known) {
                    best.insert(next, cost);
                    came_from.insert(next, current);
                    open.push(Frontier { estimate: cost + heuristic(next), province: next });
                }
            }
        }
        None
    }

    /// 纯函数：计算某国家初始拥有的胜利点总数
    pub fn original_victory_points(&self, country: CountryId) -> u32 {
        self.provinces.values()
            .filter(|p| p.owner == country)
            .map(|p| p.victory_points)
            .sum()
    }
}

pub fn haversine_km((lon1, lat1): (f32, f32), (lon2, lat2): (f32, f32)) -> f32 {
    let (lat1, lat2) = (lat1.to_radians(), lat2.to_radians());
    let dlat = lat2 - lat1;
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * a.sqrt().min(1.0).asin()
}
