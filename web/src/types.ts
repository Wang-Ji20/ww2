// 与 crates/core 中 serde 序列化结构一一对应的类型定义

export type CountryId = "Germany" | "Poland" | "France" | "UnitedKingdom" | "SovietUnion";

export type ProvinceId =
  | "Berlin" | "Cologne" | "Munich" | "EastPrussia"
  | "Danzig" | "Warsaw" | "Krakow"
  | "Paris" | "Maginot" | "Calais"
  | "London" | "Dover"
  | "Minsk" | "Kiev" | "Leningrad" | "Moscow" | "Stalingrad";

export type Terrain = "Plains" | "Forest" | "Mountain" | "Urban" | "Fortified";
export type UnitType = "Infantry" | "Panzer" | "Motorized";
export type DivisionId = number;

export interface GameDate {
  year: number;
  month: number;
  day: number;
  hour: number;
}

export interface Province {
  id: ProvinceId;
  name: string;
  original_owner: CountryId;
  controller: CountryId;
  terrain: Terrain;
  victory_points: number;
  civ_factories: number;
  mil_factories: number;
  infrastructure: number;
  neighbors: ProvinceId[];
  map_pos: [number, number];
}

export type DivisionOrder =
  | { kind: "Idle" }
  | { kind: "Moving"; path: ProvinceId[]; progress_km: number; distance_km: number }
  | { kind: "Fighting"; combat_province: ProvinceId; is_attacker: boolean; resume_path: ProvinceId[] };

export interface Division {
  id: DivisionId;
  name: string;
  country: CountryId;
  location: ProvinceId;
  unit_type: UnitType;
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
  unit_type: UnitType;
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
  map: { provinces: Record<ProvinceId, Province> };
  countries: Record<CountryId, Country>;
  // JSON 对象键总是字符串
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
  | { type: "QueueProduction"; args: UnitType }
  | { type: "CancelProduction"; args: number }
  | { type: "DeclareWarNextEnemy" };

export interface Catalog {
  provinces: { id: ProvinceId; name: string }[];
  terrains: { id: Terrain; name: string; attack_modifier: number; defense_modifier: number }[];
  unit_types: {
    id: UnitType;
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
  }[];
}

/** 前端本地的行军部署状态：选好部队后再点击目标地块 */
export type Targeting = null | { allUnits: boolean; source: ProvinceId };
