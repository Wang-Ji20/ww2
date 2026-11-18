//! 模组包（Mod Pack）：地图、国家、兵种、地形与剧本全部来自数据文件。
//!
//! 一个模组由若干 JSON 数据文件组成，每个文件可包含任意几个分区：
//! `terrains`、`unit_types`、`countries`、`provinces`、`scenarios`。
//! 多个模组按加载顺序叠加：后加载的条目按主键（`id` / `tag`）与先前条目做深度合并，
//! 只需写出要改动的字段；`"_delete": true` 删除该条目。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::types::{CountryId, GameDate, ProvinceId, TerrainId, UnitStats, UnitTypeId};

/// 各分区及其条目主键
const SECTIONS: [(&str, &str); 5] = [
    ("terrains", "id"),
    ("unit_types", "id"),
    ("countries", "tag"),
    ("provinces", "id"),
    ("scenarios", "id"),
];

/// 一个已读取的模组：清单 id 与其全部数据文件的内容
#[derive(Debug, Clone, Deserialize)]
pub struct ModLayer {
    pub id: String,
    pub files: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainDef {
    pub id: TerrainId,
    pub name: String,
    pub attack_modifier: f32,
    pub defense_modifier: f32,
    /// 行军距离倍率：山地、丛林行军更慢
    #[serde(default = "one")]
    pub movement_cost: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitTypeDef {
    pub id: UnitTypeId,
    pub name: String,
    pub icon: String,
    #[serde(flatten)]
    pub stats: UnitStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryDef {
    pub tag: CountryId,
    pub name: String,
    #[serde(default)]
    pub name_en: String,
    /// 地图填色，CSS 颜色
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvinceDef {
    pub id: ProvinceId,
    pub name: String,
    #[serde(default)]
    pub name_en: String,
    pub terrain: TerrainId,
    #[serde(default)]
    pub victory_points: u32,
    #[serde(default)]
    pub civ_factories: u32,
    #[serde(default)]
    pub mil_factories: u32,
    #[serde(default = "default_infrastructure")]
    pub infrastructure: u32,
    pub neighbors: Vec<ProvinceId>,
    /// 省份中心经纬度 [经度, 纬度]
    pub center: (f32, f32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionDef {
    pub unit_type: UnitTypeId,
    pub factories: u32,
    #[serde(default)]
    pub progress: f32,
}

/// 剧本中的国家开局数据；未列出的国家使用默认值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioCountry {
    pub tag: CountryId,
    /// 缺省为其领土中胜利点最高的省份
    #[serde(default)]
    pub capital: Option<ProvinceId>,
    #[serde(default = "default_manpower")]
    pub manpower: f32,
    #[serde(default = "default_civ")]
    pub civ_factories: u32,
    #[serde(default = "default_mil")]
    pub mil_factories: u32,
    #[serde(default)]
    pub production: Vec<ProductionDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivisionDef {
    pub name: String,
    pub country: CountryId,
    pub location: ProvinceId,
    pub unit_type: UnitTypeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionDef {
    pub id: String,
    pub name: String,
    pub members: Vec<CountryId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub start_date: GameDate,
    /// 默认玩家国家
    pub player: CountryId,
    /// 各国开局领土
    pub owners: BTreeMap<CountryId, Vec<ProvinceId>>,
    /// 开局时被他国军事占领的省份（控制者 ≠ 所有者）
    #[serde(default)]
    pub controllers: BTreeMap<CountryId, Vec<ProvinceId>>,
    #[serde(default)]
    pub countries: Vec<ScenarioCountry>,
    #[serde(default)]
    pub divisions: Vec<DivisionDef>,
    /// 开局即处于交战状态的国家对
    #[serde(default)]
    pub wars: Vec<(CountryId, CountryId)>,
    #[serde(default)]
    pub factions: Vec<FactionDef>,
    /// 主要列强：玩家阵营以外的列强全部投降即获胜
    #[serde(default)]
    pub majors: Vec<CountryId>,
    #[serde(default)]
    pub world_tension: f32,
    #[serde(default)]
    pub intro_logs: Vec<String>,
}

/// 合并后的完整模组内容
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModPack {
    #[serde(default)]
    pub terrains: Vec<TerrainDef>,
    #[serde(default)]
    pub unit_types: Vec<UnitTypeDef>,
    #[serde(default)]
    pub countries: Vec<CountryDef>,
    #[serde(default)]
    pub provinces: Vec<ProvinceDef>,
    #[serde(default)]
    pub scenarios: Vec<ScenarioDef>,
}

fn one() -> f32 {
    1.0
}
fn default_infrastructure() -> u32 {
    5
}
fn default_manpower() -> f32 {
    100_000.0
}
fn default_civ() -> u32 {
    2
}
fn default_mil() -> u32 {
    1
}

impl ModPack {
    /// 按加载顺序合并多个模组；出错时返回带模组与文件位置的说明
    pub fn from_layers(layers: &[ModLayer]) -> Result<Self, Vec<String>> {
        let mut merged: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
        let mut errors = Vec::new();

        for layer in layers {
            for (file_idx, file) in layer.files.iter().enumerate() {
                let Some(obj) = file.as_object() else {
                    errors.push(format!("[{}] 第 {} 个数据文件不是 JSON 对象", layer.id, file_idx + 1));
                    continue;
                };
                for key in obj.keys().filter(|k| !SECTIONS.iter().any(|(s, _)| s == k)) {
                    errors.push(format!("[{}] 未知分区 {key:?}", layer.id));
                }
                for &(section, pk) in &SECTIONS {
                    let Some(entries) = obj.get(section) else { continue };
                    let Some(entries) = entries.as_array() else {
                        errors.push(format!("[{}] 分区 {section} 应为数组", layer.id));
                        continue;
                    };
                    let target = merged.entry(section).or_default();
                    for entry in entries {
                        if let Err(e) = merge_entry(target, entry, pk) {
                            errors.push(format!("[{}] {section}: {e}", layer.id));
                        }
                    }
                }
            }
        }

        let as_object: Map<String, Value> =
            merged.into_iter().map(|(k, v)| (k.to_string(), Value::Array(v))).collect();
        match serde_json::from_value::<ModPack>(Value::Object(as_object)) {
            Ok(pack) if errors.is_empty() => Ok(pack),
            Ok(_) => Err(errors),
            Err(e) => {
                errors.push(format!("数据格式错误：{e}"));
                Err(errors)
            }
        }
    }

    /// 解析前端传入的模组数组 JSON
    pub fn from_layers_json(json: &str) -> Result<Self, Vec<String>> {
        let layers: Vec<ModLayer> =
            serde_json::from_str(json).map_err(|e| vec![format!("模组列表格式错误：{e}")])?;
        Self::from_layers(&layers)
    }

    pub fn scenario(&self, id: &str) -> Option<&ScenarioDef> {
        self.scenarios.iter().find(|s| s.id == id)
    }

    /// 检查引用完整性；`scenario` 为 None 时只检查静态数据
    pub fn validate(&self, scenario: Option<&ScenarioDef>) -> Vec<String> {
        let mut errors = Vec::new();
        let terrains: BTreeSet<&str> = self.terrains.iter().map(|t| t.id.as_str()).collect();
        let units: BTreeSet<&str> = self.unit_types.iter().map(|u| u.id.as_str()).collect();
        let countries: BTreeSet<CountryId> = self.countries.iter().map(|c| c.tag).collect();
        let provinces: BTreeSet<ProvinceId> = self.provinces.iter().map(|p| p.id).collect();

        if self.unit_types.is_empty() {
            errors.push("至少需要定义一个兵种".to_string());
        }
        for p in &self.provinces {
            if !terrains.contains(p.terrain.as_str()) {
                errors.push(format!("省份 {} ({}) 的地形 {:?} 未定义", p.id, p.name, p.terrain));
            }
            for n in p.neighbors.iter().filter(|n| !provinces.contains(n)) {
                errors.push(format!("省份 {} ({}) 的邻接省份 {n} 不存在", p.id, p.name));
            }
        }

        let Some(sc) = scenario else { return errors };
        let ctx = |what: String| format!("剧本 {}: {what}", sc.id);
        let check_country = |errors: &mut Vec<String>, tag: CountryId, role: &str| {
            if !countries.contains(&tag) {
                errors.push(ctx(format!("{role}引用了未定义的国家 {tag}")));
            }
        };

        check_country(&mut errors, sc.player, "玩家国家");
        let mut owner_of: BTreeMap<ProvinceId, CountryId> = BTreeMap::new();
        for (&tag, provs) in &sc.owners {
            check_country(&mut errors, tag, "领土表");
            for &p in provs {
                if !provinces.contains(&p) {
                    errors.push(ctx(format!("{tag} 的领土 {p} 不存在")));
                } else if let Some(prev) = owner_of.insert(p, tag) {
                    errors.push(ctx(format!("省份 {p} 同时属于 {prev} 与 {tag}")));
                }
            }
        }
        for p in self.provinces.iter().filter(|p| !owner_of.contains_key(&p.id)) {
            errors.push(ctx(format!("省份 {} ({}) 没有所属国家", p.id, p.name)));
        }
        for (&tag, provs) in &sc.controllers {
            check_country(&mut errors, tag, "占领表");
            if !sc.owners.contains_key(&tag) {
                errors.push(ctx(format!("占领者 {tag} 自身没有领土")));
            }
            for p in provs.iter().filter(|p| !provinces.contains(p)) {
                errors.push(ctx(format!("{tag} 占领的省份 {p} 不存在")));
            }
        }
        if !sc.owners.contains_key(&sc.player) {
            errors.push(ctx(format!("玩家国家 {} 没有任何领土", sc.player)));
        }
        for c in &sc.countries {
            check_country(&mut errors, c.tag, "国家开局数据");
            if let Some(cap) = c.capital
                && owner_of.get(&cap) != Some(&c.tag)
            {
                errors.push(ctx(format!("{} 的首都 {cap} 不在其领土内", c.tag)));
            }
            for line in c.production.iter().filter(|l| !units.contains(l.unit_type.as_str())) {
                errors.push(ctx(format!("{} 的生产线兵种 {:?} 未定义", c.tag, line.unit_type)));
            }
        }
        for d in &sc.divisions {
            check_country(&mut errors, d.country, "师团");
            if !provinces.contains(&d.location) {
                errors.push(ctx(format!("师团 {} 的驻地 {} 不存在", d.name, d.location)));
            }
            if !units.contains(d.unit_type.as_str()) {
                errors.push(ctx(format!("师团 {} 的兵种 {:?} 未定义", d.name, d.unit_type)));
            }
        }
        for &(a, b) in &sc.wars {
            check_country(&mut errors, a, "战争");
            check_country(&mut errors, b, "战争");
        }
        for f in &sc.factions {
            for &m in &f.members {
                check_country(&mut errors, m, &format!("阵营 {} ", f.name));
            }
        }
        for &m in &sc.majors {
            check_country(&mut errors, m, "列强名单");
        }
        errors
    }
}

/// 将一个条目按主键合并进分区：新主键追加，已有主键深度合并，`_delete` 删除
fn merge_entry(target: &mut Vec<Value>, entry: &Value, pk: &str) -> Result<(), String> {
    let key = entry.get(pk).ok_or_else(|| format!("条目缺少主键 {pk:?}: {entry}"))?;
    let existing = target.iter().position(|e| e.get(pk) == Some(key));
    let delete = entry.get("_delete").and_then(Value::as_bool).unwrap_or(false);

    match (existing, delete) {
        (Some(i), true) => {
            target.remove(i);
        }
        (None, true) => return Err(format!("要删除的条目 {key} 不存在")),
        (Some(i), false) => deep_merge(&mut target[i], entry),
        (None, false) => target.push(entry.clone()),
    }
    Ok(())
}

/// 对象逐字段递归合并；数组与标量整体替换
fn deep_merge(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(base), Value::Object(patch)) => {
            for (k, v) in patch {
                match base.get_mut(k) {
                    Some(slot) => deep_merge(slot, v),
                    None => {
                        base.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (base, patch) => *base = patch.clone(),
    }
}
