import type { CountryId, Division, GameDate, GameState, ProvinceId, World } from "./types";

const pad = (n: number) => String(n).padStart(2, "0");

export function formatDate(d: GameDate): string {
  return `${d.year}年${pad(d.month)}月${pad(d.day)}日 ${pad(d.hour)}:00`;
}

export function countryColor(world: World, tag: CountryId): string {
  return world.countries[tag]?.color ?? "#777";
}

export function countryName(world: World, tag: CountryId): string {
  return world.countries[tag]?.name ?? tag;
}

export function factionOf(world: World, tag: CountryId) {
  return world.scenario.factions.find((f) => f.members.includes(tag));
}

export function areAllies(world: World, a: CountryId, b: CountryId): boolean {
  return a === b || (factionOf(world, a)?.members.includes(b) ?? false);
}

export const controllerOf = (state: GameState, id: ProvinceId): CountryId => state.controllers[id];

export function divisionsIn(state: GameState, province: ProvinceId): Division[] {
  return Object.values(state.divisions)
    .filter((d) => d.location === province)
    .sort((a, b) => {
      const ownA = a.country === state.player_country ? 0 : 1;
      const ownB = b.country === state.player_country ? 0 : 1;
      return ownA - ownB || a.country.localeCompare(b.country) || a.id - b.id;
    });
}

/** 某国当前控制的胜利点占其开局胜利点的百分比 */
export function vpShare(world: World, state: GameState, tag: CountryId): number {
  let orig = 0;
  let cur = 0;
  for (const p of Object.values(world.map.provinces)) {
    if (p.owner === tag) orig += p.victory_points;
    if (state.controllers[p.id] === tag) cur += p.victory_points;
  }
  return orig > 0 ? Math.round((cur / orig) * 100) : 0;
}

export function orderLabel(d: Division): string {
  switch (d.order.kind) {
    case "Idle":
      return "待命";
    case "Moving":
      return "行军中";
    case "Fighting":
      return d.order.is_attacker ? "进攻中" : "防守中";
  }
}

type LogTone = "victory" | "loss" | "battle" | "plain";

export function logTone(log: string): LogTone {
  if (log.includes("大捷") || log.includes("宣战")) return "victory";
  if (log.includes("溃退") || log.includes("包围") || log.includes("投降")) return "loss";
  if (log.includes("战斗") || log.includes("战报")) return "battle";
  return "plain";
}
