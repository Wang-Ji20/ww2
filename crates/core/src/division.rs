use crate::modpack::UnitTypeDef;
use crate::types::{CountryId, ProvinceId, UnitTypeId};

pub type DivisionId = u32;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind")]
pub enum DivisionOrder {
    Idle,
    Moving {
        path: Vec<ProvinceId>,
        progress_km: f32,
        distance_km: f32,
    },
    Fighting {
        combat_province: ProvinceId,
        is_attacker: bool,
        /// 攻方胜利后继续行军的剩余路径
        resume_path: Vec<ProvinceId>,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Division {
    pub id: DivisionId,
    pub name: String,
    pub country: CountryId,
    pub location: ProvinceId,
    pub unit_type: UnitTypeId,
    pub strength: f32,          // 0.0 ~ 100.0 (HP 百分比)
    pub organization: f32,      // 当前组织度
    pub max_organization: f32,  // 最大组织度
    pub soft_attack: f32,
    pub hard_attack: f32,
    pub defense: f32,
    pub breakthrough: f32,
    pub speed_kmh: f32,
    pub order: DivisionOrder,
}

impl Division {
    /// 纯函数构造师团
    pub fn new(id: DivisionId, name: String, country: CountryId, location: ProvinceId, unit_type: &UnitTypeDef) -> Self {
        let stats = unit_type.stats;
        Self {
            id,
            name,
            country,
            location,
            unit_type: unit_type.id.clone(),
            strength: 100.0,
            organization: stats.max_organization,
            max_organization: stats.max_organization,
            soft_attack: stats.soft_attack,
            hard_attack: stats.hard_attack,
            defense: stats.defense,
            breakthrough: stats.breakthrough,
            speed_kmh: stats.speed_kmh,
            order: DivisionOrder::Idle,
        }
    }

    /// 计算实战有效攻防属性（受当前 HP 兵力衰减，函数式映射）
    pub fn effective_combat_stats(&self) -> (f32, f32, f32, f32) {
        let strength_ratio = (self.strength / 100.0).clamp(0.05, 1.0);
        let org_ratio = (self.organization / self.max_organization).clamp(0.1, 1.0);
        let factor = strength_ratio * (0.5 + 0.5 * org_ratio);

        (
            self.soft_attack * factor,
            self.hard_attack * factor,
            self.defense * factor,
            self.breakthrough * factor,
        )
    }

    /// 纯函数承受战斗伤害，返回新的师团对象
    pub fn take_damage(&self, org_damage: f32, hp_damage: f32) -> Self {
        let new_org = (self.organization - org_damage).max(0.0);
        let new_hp = (self.strength - hp_damage).max(0.0);
        Self {
            organization: new_org,
            strength: new_hp,
            ..self.clone()
        }
    }

    /// 纯函数恢复组织度与补员
    pub fn replenish_and_recover(&self, org_regen: f32, hp_reinforce: f32) -> Self {
        let new_org = (self.organization + org_regen).min(self.max_organization);
        let new_hp = (self.strength + hp_reinforce).min(100.0);
        Self {
            organization: new_org,
            strength: new_hp,
            ..self.clone()
        }
    }

    pub fn with_order(&self, order: DivisionOrder) -> Self {
        Self {
            order,
            ..self.clone()
        }
    }

    pub fn with_location(&self, location: ProvinceId) -> Self {
        Self {
            location,
            ..self.clone()
        }
    }

    pub fn is_broken(&self) -> bool {
        self.organization <= 1.0 || self.strength <= 1.0
    }
}
