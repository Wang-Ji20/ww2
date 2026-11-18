use std::collections::BTreeMap;
use crate::division::{Division, DivisionId};
use crate::types::{CountryId, ProvinceId};

pub type CombatId = u32;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Combat {
    pub id: CombatId,
    pub province: ProvinceId,
    pub attacker_country: CountryId,
    pub defender_country: CountryId,
    pub attackers: Vec<DivisionId>,
    pub defenders: Vec<DivisionId>,
    /// 战局进度：0.0 (攻方溃退) 到 100.0 (守方崩溃沦陷)，初始 50.0
    pub progress: f32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum CombatOutcome {
    Ongoing,
    AttackerVictory,
    DefenderVictory,
}

#[derive(Debug, Clone)]
pub struct CombatDamageRecord {
    pub division_id: DivisionId,
    pub org_damage: f32,
    pub hp_damage: f32,
}

#[derive(Debug, Clone)]
pub struct CombatStepReport {
    pub outcome: CombatOutcome,
    pub damages: Vec<CombatDamageRecord>,
    pub new_progress: f32,
}

impl Combat {
    pub fn new(
        id: CombatId,
        province: ProvinceId,
        attacker_country: CountryId,
        defender_country: CountryId,
        attackers: Vec<DivisionId>,
        defenders: Vec<DivisionId>,
    ) -> Self {
        Self {
            id,
            province,
            attacker_country,
            defender_country,
            attackers,
            defenders,
            progress: 50.0,
        }
    }

    /// 纯函数结算战斗的一个时间片；修正值为地形的（攻方修正，守方修正）
    pub fn step(
        &self,
        divisions: &BTreeMap<DivisionId, Division>,
        (att_mod, def_mod): (f32, f32),
        dt_hours: f32,
    ) -> CombatStepReport {

        // 1. 函数式聚合攻方总战力与突破
        let (att_soft, att_hard, _att_def, att_brk) = self.attackers.iter()
            .filter_map(|id| divisions.get(id))
            .filter(|d| !d.is_broken())
            .map(|d| d.effective_combat_stats())
            .fold((0.0f32, 0.0f32, 0.0f32, 0.0f32), |acc, (s, h, d, b)| {
                (acc.0 + s, acc.1 + h, acc.2 + d, acc.3 + b)
            });

        // 2. 函数式聚合守方总战力与防御
        let (def_soft, def_hard, def_def, _def_brk) = self.defenders.iter()
            .filter_map(|id| divisions.get(id))
            .filter(|d| !d.is_broken())
            .map(|d| d.effective_combat_stats())
            .fold((0.0f32, 0.0f32, 0.0f32, 0.0f32), |acc, (s, h, d, b)| {
                (acc.0 + s, acc.1 + h, acc.2 + d, acc.3 + b)
            });

        let total_att_attack = (att_soft + att_hard * 0.5) * att_mod;
        let total_def_attack = (def_soft + def_hard * 0.5) * def_mod;

        // 3. 计算对守方的伤害：突破 vs 防御 (HOI4 机制)
        let effective_att_hit = if total_att_attack > def_def {
            def_def * 0.1 + (total_att_attack - def_def) * 0.4
        } else {
            total_att_attack * 0.1
        };

        // 4. 计算对攻方的反击伤害：守方攻击 vs 攻方突破
        let effective_def_hit = if total_def_attack > att_brk {
            att_brk * 0.1 + (total_def_attack - att_brk) * 0.4
        } else {
            total_def_attack * 0.1
        };

        let hourly_att_dmg = (effective_att_hit * 0.08 * dt_hours).max(0.1);
        let hourly_def_dmg = (effective_def_hit * 0.08 * dt_hours).max(0.1);

        // 5. 分摊伤害到具体部队
        let active_def_count = self.defenders.iter()
            .filter_map(|id| divisions.get(id))
            .filter(|d| !d.is_broken())
            .count().max(1) as f32;

        let active_att_count = self.attackers.iter()
            .filter_map(|id| divisions.get(id))
            .filter(|d| !d.is_broken())
            .count().max(1) as f32;

        let mut damages = Vec::new();

        // 守方受损
        for &id in &self.defenders {
            if let Some(div) = divisions.get(&id)
                && !div.is_broken()
            {
                let org_dmg = hourly_att_dmg / active_def_count;
                let hp_dmg = org_dmg * 0.15;
                damages.push(CombatDamageRecord {
                    division_id: id,
                    org_damage: org_dmg,
                    hp_damage: hp_dmg,
                });
            }
        }

        // 攻方受损
        for &id in &self.attackers {
            if let Some(div) = divisions.get(&id)
                && !div.is_broken()
            {
                let org_dmg = hourly_def_dmg / active_att_count;
                let hp_dmg = org_dmg * 0.15;
                damages.push(CombatDamageRecord {
                    division_id: id,
                    org_damage: org_dmg,
                    hp_damage: hp_dmg,
                });
            }
        }

        // 6. 更新战局进度 (50为中间，>50偏攻方，<50偏守方)
        let delta_progress = (total_att_attack - total_def_attack) * 0.03 * dt_hours;
        let new_progress = (self.progress + delta_progress).clamp(0.0, 100.0);

        // 7. 判断战斗胜负条件
        let all_defenders_broken = self.defenders.iter()
            .filter_map(|id| divisions.get(id))
            .all(|d| {
                let damage = damages.iter().find(|r| r.division_id == d.id);
                let org = d.organization - damage.map_or(0.0, |r| r.org_damage);
                org <= 1.0
            });

        let all_attackers_broken = self.attackers.iter()
            .filter_map(|id| divisions.get(id))
            .all(|d| {
                let damage = damages.iter().find(|r| r.division_id == d.id);
                let org = d.organization - damage.map_or(0.0, |r| r.org_damage);
                org <= 1.0
            });

        let outcome = if all_defenders_broken || new_progress >= 99.0 {
            CombatOutcome::AttackerVictory
        } else if all_attackers_broken || new_progress <= 1.0 {
            CombatOutcome::DefenderVictory
        } else {
            CombatOutcome::Ongoing
        };

        CombatStepReport {
            outcome,
            damages,
            new_progress,
        }
    }
}
