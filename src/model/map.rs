use std::collections::{HashMap, VecDeque};
use crate::model::province::Province;
use crate::model::types::{CountryId, ProvinceId, Terrain};

#[derive(Clone)]
pub struct WorldMap {
    pub provinces: HashMap<ProvinceId, Province>,
}

impl WorldMap {
    /// 纯函数构造二战 1939 欧战开局地图
    pub fn new_europe_1939() -> Self {
        let mut map = HashMap::new();

        // 辅助宏或闭包构造省份
        let create_prov = |id: ProvinceId,
                           name: &'static str,
                           owner: CountryId,
                           terrain: Terrain,
                           vp: u32,
                           civ: u32,
                           mil: u32,
                           infra: u32,
                           neighbors: Vec<ProvinceId>,
                           pos: (u16, u16)|
         -> Province {
            Province {
                id,
                name,
                original_owner: owner,
                controller: owner,
                terrain,
                victory_points: vp,
                civ_factories: civ,
                mil_factories: mil,
                infrastructure: infra,
                neighbors,
                map_pos: pos,
            }
        };

        // 德意志国
        map.insert(
            ProvinceId::Berlin,
            create_prov(
                ProvinceId::Berlin,
                "柏林",
                CountryId::Germany,
                Terrain::Plains,
                50, 10, 15, 8,
                vec![ProvinceId::Cologne, ProvinceId::Munich, ProvinceId::Danzig],
                (24, 7),
            ),
        );
        map.insert(
            ProvinceId::Cologne,
            create_prov(
                ProvinceId::Cologne,
                "科隆",
                CountryId::Germany,
                Terrain::Urban,
                20, 8, 10, 7,
                vec![ProvinceId::Berlin, ProvinceId::Munich, ProvinceId::Calais, ProvinceId::Maginot],
                (17, 8),
            ),
        );
        map.insert(
            ProvinceId::Munich,
            create_prov(
                ProvinceId::Munich,
                "慕尼黑",
                CountryId::Germany,
                Terrain::Mountain,
                20, 6, 8, 7,
                vec![ProvinceId::Berlin, ProvinceId::Cologne, ProvinceId::Krakow],
                (22, 12),
            ),
        );
        map.insert(
            ProvinceId::EastPrussia,
            create_prov(
                ProvinceId::EastPrussia,
                "东普鲁士",
                CountryId::Germany,
                Terrain::Forest,
                15, 3, 4, 6,
                vec![ProvinceId::Danzig, ProvinceId::Warsaw, ProvinceId::Minsk, ProvinceId::Leningrad],
                (36, 5),
            ),
        );

        // 波兰
        map.insert(
            ProvinceId::Danzig,
            create_prov(
                ProvinceId::Danzig,
                "但泽走廊",
                CountryId::Poland,
                Terrain::Plains,
                15, 2, 2, 5,
                vec![ProvinceId::Berlin, ProvinceId::EastPrussia, ProvinceId::Warsaw],
                (30, 6),
            ),
        );
        map.insert(
            ProvinceId::Warsaw,
            create_prov(
                ProvinceId::Warsaw,
                "华沙",
                CountryId::Poland,
                Terrain::Plains,
                40, 5, 4, 6,
                vec![ProvinceId::Danzig, ProvinceId::EastPrussia, ProvinceId::Krakow, ProvinceId::Minsk, ProvinceId::Kiev],
                (34, 9),
            ),
        );
        map.insert(
            ProvinceId::Krakow,
            create_prov(
                ProvinceId::Krakow,
                "克拉科夫",
                CountryId::Poland,
                Terrain::Forest,
                15, 3, 2, 5,
                vec![ProvinceId::Munich, ProvinceId::Warsaw, ProvinceId::Kiev],
                (31, 12),
            ),
        );

        // 法国
        map.insert(
            ProvinceId::Paris,
            create_prov(
                ProvinceId::Paris,
                "巴黎",
                CountryId::France,
                Terrain::Urban,
                50, 10, 8, 8,
                vec![ProvinceId::Calais, ProvinceId::Maginot],
                (10, 10),
            ),
        );
        map.insert(
            ProvinceId::Maginot,
            create_prov(
                ProvinceId::Maginot,
                "马奇诺",
                CountryId::France,
                Terrain::Fortified,
                20, 2, 4, 9,
                vec![ProvinceId::Paris, ProvinceId::Cologne],
                (15, 11),
            ),
        );
        map.insert(
            ProvinceId::Calais,
            create_prov(
                ProvinceId::Calais,
                "加来",
                CountryId::France,
                Terrain::Plains,
                15, 3, 3, 7,
                vec![ProvinceId::Paris, ProvinceId::Cologne, ProvinceId::Dover],
                (11, 7),
            ),
        );

        // 英国
        map.insert(
            ProvinceId::London,
            create_prov(
                ProvinceId::London,
                "伦敦",
                CountryId::UnitedKingdom,
                Terrain::Urban,
                50, 12, 12, 9,
                vec![ProvinceId::Dover],
                (5, 5),
            ),
        );
        map.insert(
            ProvinceId::Dover,
            create_prov(
                ProvinceId::Dover,
                "多佛尔",
                CountryId::UnitedKingdom,
                Terrain::Fortified,
                15, 3, 3, 8,
                vec![ProvinceId::London, ProvinceId::Calais],
                (8, 7),
            ),
        );

        // 苏联
        map.insert(
            ProvinceId::Minsk,
            create_prov(
                ProvinceId::Minsk,
                "明斯克",
                CountryId::SovietUnion,
                Terrain::Forest,
                20, 4, 5, 5,
                vec![ProvinceId::EastPrussia, ProvinceId::Warsaw, ProvinceId::Leningrad, ProvinceId::Moscow, ProvinceId::Kiev],
                (42, 8),
            ),
        );
        map.insert(
            ProvinceId::Kiev,
            create_prov(
                ProvinceId::Kiev,
                "基辅",
                CountryId::SovietUnion,
                Terrain::Plains,
                25, 6, 6, 5,
                vec![ProvinceId::Krakow, ProvinceId::Warsaw, ProvinceId::Minsk, ProvinceId::Moscow, ProvinceId::Stalingrad],
                (41, 12),
            ),
        );
        map.insert(
            ProvinceId::Leningrad,
            create_prov(
                ProvinceId::Leningrad,
                "列宁格勒",
                CountryId::SovietUnion,
                Terrain::Urban,
                40, 8, 10, 7,
                vec![ProvinceId::EastPrussia, ProvinceId::Minsk, ProvinceId::Moscow],
                (46, 4),
            ),
        );
        map.insert(
            ProvinceId::Moscow,
            create_prov(
                ProvinceId::Moscow,
                "莫斯科",
                CountryId::SovietUnion,
                Terrain::Urban,
                60, 15, 18, 8,
                vec![ProvinceId::Leningrad, ProvinceId::Minsk, ProvinceId::Kiev, ProvinceId::Stalingrad],
                (52, 7),
            ),
        );
        map.insert(
            ProvinceId::Stalingrad,
            create_prov(
                ProvinceId::Stalingrad,
                "斯大林格勒",
                CountryId::SovietUnion,
                Terrain::Urban,
                50, 10, 12, 6,
                vec![ProvinceId::Kiev, ProvinceId::Moscow],
                (53, 12),
            ),
        );

        Self { provinces: map }
    }

    /// 纯函数：广度优先寻路算法 (BFS Pathfinding)
    pub fn find_path(
        &self,
        start: ProvinceId,
        goal: ProvinceId,
        passable_predicate: impl Fn(ProvinceId) -> bool,
    ) -> Option<Vec<ProvinceId>> {
        if start == goal {
            return Some(vec![start]);
        }

        let mut queue = VecDeque::new();
        let mut came_from: HashMap<ProvinceId, ProvinceId> = HashMap::new();
        queue.push_back(start);

        while let Some(current) = queue.pop_front() {
            if current == goal {
                // 回溯路径
                let mut path = Vec::new();
                let mut curr = goal;
                while curr != start {
                    path.push(curr);
                    curr = came_from[&curr];
                }
                path.reverse();
                return Some(path);
            }

            if let Some(prov) = self.provinces.get(&current) {
                for &neighbor in &prov.neighbors {
                    if !came_from.contains_key(&neighbor) && neighbor != start {
                        if neighbor == goal || passable_predicate(neighbor) {
                            came_from.insert(neighbor, current);
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }

        None
    }

    /// 纯函数：计算某国家当前控制的胜利点总数
    pub fn total_victory_points(&self, country: CountryId) -> u32 {
        self.provinces.values()
            .filter(|p| p.controller == country)
            .map(|p| p.victory_points)
            .sum()
    }

    /// 纯函数：计算某国家初始拥有的胜利点总数
    pub fn original_victory_points(&self, country: CountryId) -> u32 {
        self.provinces.values()
            .filter(|p| p.original_owner == country)
            .map(|p| p.victory_points)
            .sum()
    }
}
