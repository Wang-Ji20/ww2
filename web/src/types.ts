// 与 crates/core 中 serde 序列化结构一一对应的类型定义

/** 国家标签，如 "GER"，由模组定义 */
export type CountryId = string;
/** 省份编号，由模组定义 */
export type ProvinceId = number;
/** 兵种编号，如 "Infantry"，由模组定义 */
export type UnitTypeId = string;
export type DivisionId = number;

export interface GameDate {
  year: number;
  month: number;
  day: number;
  hour: number;
}

/* ───────────── 静态世界（worldJson，开局读取一次） ───────────── */

export interface Province {
  id: ProvinceId;
  name: string;
  name_en: string;
  /** 剧本开局时的法理所有者 */
  owner: CountryId;
  terrain: string;
  victory_points: number;
  civ_factories: number;
  mil_factories: number;
  infrastructure: number;
  neighbors: ProvinceId[];
  /** [经度, 纬度] */
  center: [number, number];
}

export interface TerrainDef {
  id: string;
  name: string;
  attack_modifier: number;
  defense_modifier: number;
  movement_cost: number;
}

export interface UnitTypeDef {
  id: UnitTypeId;
  name: string;
  icon: string;
  max_organization: number;
  soft_attack: number;
  hard_attack: number;
  defense: number;
  breakthrough: number;
  speed_kmh: number;
  production_cost: number;
  manpower_cost: number;
}

export interface CountryDef {
  tag: CountryId;
  name: string;
  name_en: string;
  color: string;
}

export interface FactionDef {
  id: string;
  name: string;
  members: CountryId[];
}

export interface World {
  scenario: {
    id: string;
    name: string;
    description: string;
    start_date: GameDate;
    factions: FactionDef[];
    majors: CountryId[];
  };
  terrains: Record<string, TerrainDef>;
  /** 首个兵种为 AI 默认生产的兵种 */
  unit_types: UnitTypeDef[];
  countries: Record<CountryId, CountryDef>;
  capitals: Record<CountryId, ProvinceId>;
  // JSON 对象键总是字符串
  map: { provinces: Record<string, Province> };
}

/* ───────────── 动态状态（stateJson，每帧读取） ───────────── */

export type DivisionOrder =
  | { kind: "Idle" }
  | { kind: "Moving"; path: ProvinceId[]; progress_km: number; distance_km: number }
  | { kind: "Fighting"; combat_province: ProvinceId; is_attacker: boolean; resume_path: ProvinceId[] };

export interface Division {
  id: DivisionId;
  name: string;
  country: CountryId;
  location: ProvinceId;
  unit_type: UnitTypeId;
  strength: number;
  organization: number;
  max_organization: number;
  soft_attack: number;
  hard_attack: number;
  defense: number;
  breakthrough: number;
  speed_kmh: number;
  order: DivisionOrder;
}

export interface ProductionLine {
  unit_type: UnitTypeId;
  assigned_factories: number;
  progress: number;
}

export interface Country {
  id: CountryId;
  manpower: number;
  political_power: number;
  civ_factories: number;
  mil_factories: number;
  enemies: CountryId[];
  has_capitulated: boolean;
  production_queue: ProductionLine[];
}

export interface Combat {
  id: number;
  province: ProvinceId;
  attacker_country: CountryId;
  defender_country: CountryId;
  attackers: DivisionId[];
  defenders: DivisionId[];
  progress: number;
}

export type GameOutcome = "Victory" | "Defeat";

export interface GameState {
  controllers: Record<string, CountryId>;
  countries: Record<CountryId, Country>;
  divisions: Record<string, Division>;
  combats: Record<string, Combat>;
  outcome: GameOutcome | null;
  date: GameDate;
  speed: number;
  is_paused: boolean;
  world_tension: number;
  selected_province: ProvinceId;
  selected_division: DivisionId | null;
  player_country: CountryId;
  logs: string[];
}

export type UserAction =
  | { type: "TogglePause" }
  | { type: "SetSpeed"; args: number }
  | { type: "StepSingleHour" }
  | { type: "SelectProvince"; args: ProvinceId }
  | { type: "NavigateMap"; args: [number, number] }
  | { type: "SelectNextDivision" }
  | { type: "SelectDivision"; args: DivisionId }
  | { type: "OrderMoveSelected"; args: ProvinceId }
  | { type: "OrderAllUnitsInProvince"; args: [ProvinceId, ProvinceId] }
  | { type: "QueueProduction"; args: UnitTypeId }
  | { type: "CancelProduction"; args: number }
  | { type: "DeclareWar"; args: CountryId };

/** 前端本地的行军部署状态：选好部队后再点击目标地块 */
export type Targeting = null | { allUnits: boolean; source: ProvinceId };
