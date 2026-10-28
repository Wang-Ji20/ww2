use crate::model::types::{CountryId, ProvinceId, UnitType};

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionLine {
    pub unit_type: UnitType,
    pub assigned_factories: u32,
    pub progress: f32, // 达到 cost 即完成生产
    pub target_province: ProvinceId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Country {
    pub id: CountryId,
    pub manpower: f32,
    pub political_power: f32,
    pub stability: f32,
    pub war_support: f32,
    pub civ_factories: u32,
    pub mil_factories: u32,
    pub enemies: Vec<CountryId>,
    pub has_capitulated: bool,
    pub production_queue: Vec<ProductionLine>,
}

impl Country {
    pub fn new(id: CountryId, base_manpower: f32, civ: u32, mil: u32) -> Self {
        Self {
            id,
            manpower: base_manpower,
            political_power: 150.0,
            stability: 0.80,
            war_support: 0.85,
            civ_factories: civ,
            mil_factories: mil,
            enemies: Vec::new(),
            has_capitulated: false,
            production_queue: Vec::new(),
        }
    }

    pub fn is_at_war_with(&self, other: CountryId) -> bool {
        self.enemies.contains(&other)
    }

    /// 纯函数：添加交战国
    pub fn declare_war_on(&self, target: CountryId) -> Self {
        if self.enemies.contains(&target) {
            self.clone()
        } else {
            let mut enemies = self.enemies.clone();
            enemies.push(target);
            Self { enemies, ..self.clone() }
        }
    }

    /// 纯函数：生产与资源推进
    pub fn step_economy(&self, dt_hours: f32) -> (Self, Vec<UnitType>) {
        if self.has_capitulated {
            return (self.clone(), Vec::new());
        }

        let pp_growth = (1.5 / 24.0) * dt_hours;
        let mp_growth = (120.0 / 24.0) * dt_hours;

        let mut completed_units = Vec::new();
        let updated_queue = self.production_queue.iter().map(|line| {
            let (_, _, _, _, _, _, cost, _) = line.unit_type.base_stats();
            let hourly_prod = (line.assigned_factories as f32 * 2.5) * (dt_hours / 24.0);
            let new_progress = line.progress + hourly_prod;

            if new_progress >= cost {
                completed_units.push(line.unit_type);
                ProductionLine {
                    progress: (new_progress - cost),
                    ..line.clone()
                }
            } else {
                ProductionLine {
                    progress: new_progress,
                    ..line.clone()
                }
            }
        }).collect();

        let updated_country = Self {
            political_power: (self.political_power + pp_growth).min(2000.0),
            manpower: self.manpower + mp_growth,
            production_queue: updated_queue,
            ..self.clone()
        };

        (updated_country, completed_units)
    }

    pub fn with_capitulation(&self, capitulated: bool) -> Self {
        Self {
            has_capitulated: capitulated,
            ..self.clone()
        }
    }
}
