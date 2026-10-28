import type { CountryId, Division, GameDate, GameState, ProvinceId } from "./types";

export const COUNTRY_COLOR: Record<CountryId, string> = {
  Germany: "#8d939c",
  Poland: "#c75b7a",
  France: "#4f78cc",
  UnitedKingdom: "#c9733e",
  SovietUnion: "#b8323a",
};

export const COUNTRY_SHORT: Record<CountryId, string> = {
  Germany: "GER",
  Poland: "POL",
  France: "FRA",
  UnitedKingdom: "ENG",
  SovietUnion: "SOV",
};

/** 宣战顺序与核心 DeclareWarNextEnemy 保持一致 */
export const WAR_CANDIDATES: CountryId[] = ["France", "UnitedKingdom", "SovietUnion"];

const pad = (n: number) => String(n).padStart(2, "0");

export function formatDate(d: GameDate): string {
  return `${d.year}年${pad(d.month)}月${pad(d.day)}日 ${pad(d.hour)}:00`;
}

export function divisionsIn(state: GameState, province: ProvinceId): Division[] {
  return Object.values(state.divisions)
    .filter((d) => d.location === province)
    .sort((a, b) => {
      const ownA = a.country === state.player_country ? 0 : 1;
      const ownB = b.country === state.player_country ? 0 : 1;
      return ownA - ownB || a.country.localeCompare(b.country) || a.id - b.id;
    });
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
