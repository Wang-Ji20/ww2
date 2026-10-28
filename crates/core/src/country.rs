use crate::types::{CountryId, UnitType};

/// 每条新开生产线默认分配的军工厂数
pub const FACTORIES_PER_LINE: u32 = 5;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ProductionLine {
    pub unit_type: UnitType,
    pub assigned_factories: u32,
    pub progress: f32, // 达到 cost 即完成生产
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Country {
    pub id: CountryId,
    pub manpower: f32,
    pub political_power: f32,
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

    /// 纯函数：与目标国停战（投降后解除交战状态）
    pub fn make_peace_with(&self, target: CountryId) -> Self {
        Self {
            enemies: self.enemies.iter().copied().filter(|&c| c != target).collect(),
            ..self.clone()
        }
    }

    /// 尚未分配给生产线的军工厂
    pub fn free_factories(&self) -> u32 {
        let assigned: u32 = self.production_queue.iter().map(|l| l.assigned_factories).sum();
        self.mil_factories.saturating_sub(assigned)
    }

    /// 纯函数：新开一条生产线；没有空闲军工厂时返回 None
    pub fn queue_production(&self, unit_type: UnitType) -> Option<Self> {
        let factories = FACTORIES_PER_LINE.min(self.free_factories());
        if factories == 0 {
            return None;
        }
        let mut production_queue = self.production_queue.clone();
        production_queue.push(ProductionLine {
            unit_type,
            assigned_factories: factories,
            progress: 0.0,
        });
        Some(Self { production_queue, ..self.clone() })
    }

    /// 纯函数：撤销一条生产线，释放其军工厂
    pub fn cancel_production(&self, index: usize) -> Self {
        Self {
            production_queue: self.production_queue.iter()
                .enumerate()
                .filter(|&(i, _)| i != index)
                .map(|(_, l)| l.clone())
                .collect(),
            ..self.clone()
        }
    }

    /// 纯函数：生产与资源推进。师团完工时扣除人力；人力不足则产线停在完工状态等待补充。
    pub fn step_economy(&self, dt_hours: f32) -> (Self, Vec<UnitType>) {
        if self.has_capitulated {
            return (self.clone(), Vec::new());
        }

        let pp_growth = (1.5 / 24.0) * dt_hours;
        let mp_growth = (120.0 / 24.0) * dt_hours;

        let mut manpower = self.manpower + mp_growth;
        let mut completed_units = Vec::new();
        let updated_queue = self.production_queue.iter().map(|line| {
            let stats = line.unit_type.base_stats();
            let hourly_prod = (line.assigned_factories as f32 * 2.5) * (dt_hours / 24.0);
            let new_progress = line.progress + hourly_prod;

            let progress = if new_progress < stats.production_cost {
                new_progress
            } else if manpower >= stats.manpower_cost {
                manpower -= stats.manpower_cost;
                completed_units.push(line.unit_type);
                new_progress - stats.production_cost
            } else {
                stats.production_cost
            };

            ProductionLine { progress, ..line.clone() }
        }).collect();

        let updated_country = Self {
            political_power: (self.political_power + pp_growth).min(2000.0),
            manpower,
            production_queue: updated_queue,
            ..self.clone()
        };

        (updated_country, completed_units)
    }

    pub fn capitulated(&self) -> Self {
        Self {
            has_capitulated: true,
            ..self.clone()
        }
    }
}
