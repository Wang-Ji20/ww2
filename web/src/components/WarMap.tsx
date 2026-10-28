import { useMemo, useState } from "react";
import { COUNTRY_COLOR, COUNTRY_SHORT } from "../display";
import type { Catalog, Combat, CountryId, GameState, Province, ProvinceId, Targeting } from "../types";

const SCALE_X = 26;
const SCALE_Y = 58;
const PAD = 70;
const NODE_R = 24;
const RING_R = 32;

interface Props {
  state: GameState;
  catalog: Catalog;
  targeting: Targeting;
  onClick: (id: ProvinceId) => void;
  onQuickOrder: (id: ProvinceId, allUnits: boolean) => void;
}

interface March {
  key: string;
  from: ProvinceId;
  to: ProvinceId;
  country: CountryId;
  progress: number;
  count: number;
}

export function WarMap({ state, catalog, targeting, onClick, onQuickOrder }: Props) {
  const [hover, setHover] = useState<ProvinceId | null>(null);
  const provinces = Object.values(state.map.provinces);

  const layout = useMemo(() => {
    const xs = provinces.map((p) => p.map_pos[0]);
    const ys = provinces.map((p) => p.map_pos[1]);
    const minX = Math.min(...xs);
    const minY = Math.min(...ys);
    return {
      width: (Math.max(...xs) - minX) * SCALE_X + PAD * 2,
      height: (Math.max(...ys) - minY) * SCALE_Y + PAD * 2,
      pos: (p: Province) => ({
        x: (p.map_pos[0] - minX) * SCALE_X + PAD,
        y: (p.map_pos[1] - minY) * SCALE_Y + PAD,
      }),
    };
    // 地图拓扑与坐标在整局中不变
  }, []);

  const at = (id: ProvinceId) => layout.pos(state.map.provinces[id]);

  const edges = useMemo(
    () =>
      provinces.flatMap((p) =>
        p.neighbors.filter((n) => p.id < n).map((n) => [p.id, n] as const),
      ),
    [],
  );

  const unitCounts = new Map<ProvinceId, Map<CountryId, number>>();
  const marches = new Map<string, March>();
  for (const d of Object.values(state.divisions)) {
    const counts = unitCounts.get(d.location) ?? new Map<CountryId, number>();
    counts.set(d.country, (counts.get(d.country) ?? 0) + 1);
    unitCounts.set(d.location, counts);

    if (d.order.kind === "Moving" && d.order.path.length > 0) {
      const to = d.order.path[0];
      const key = `${d.location}-${to}-${d.country}`;
      const progress = Math.min(d.order.progress_km / d.order.distance_km, 1);
      const m = marches.get(key);
      if (m) {
        m.progress = Math.max(m.progress, progress);
        m.count += 1;
      } else {
        marches.set(key, { key, from: d.location, to, country: d.country, progress, count: 1 });
      }
    }
  }

  const combatAt = new Map<ProvinceId, Combat>(
    Object.values(state.combats).map((c) => [c.province, c]),
  );

  const preview = targeting && hover && hover !== targeting.source ? { from: targeting.source, to: hover } : null;

  return (
    <svg
      className={`war-map${targeting ? " targeting" : ""}`}
      viewBox={`0 0 ${layout.width} ${layout.height}`}
      role="img"
      aria-label="欧洲战区地图"
    >
      <defs>
        <marker id="arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="5" markerHeight="5" orient="auto">
          <path d="M0,0 L10,5 L0,10 z" fill="currentColor" />
        </marker>
        <pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse">
          <path d="M40 0 L0 0 0 40" fill="none" className="map-grid" />
        </pattern>
      </defs>
      <rect width={layout.width} height={layout.height} fill="url(#grid)" />

      <g className="map-edges">
        {edges.map(([a, b]) => {
          const pa = at(a);
          const pb = at(b);
          const front = state.map.provinces[a].controller !== state.map.provinces[b].controller;
          return (
            <line
              key={`${a}-${b}`}
              x1={pa.x}
              y1={pa.y}
              x2={pb.x}
              y2={pb.y}
              className={front ? "edge front" : "edge"}
            />
          );
        })}
      </g>

      {preview && (
        <line
          className="order-preview"
          {...linePoints(at(preview.from), at(preview.to))}
          markerEnd="url(#arrow)"
        />
      )}

      <g className="map-marches">
        {[...marches.values()].map((m) => {
          const a = at(m.from);
          const b = at(m.to);
          const pts = linePoints(a, b);
          const dot = {
            x: pts.x1 + (pts.x2 - pts.x1) * m.progress,
            y: pts.y1 + (pts.y2 - pts.y1) * m.progress,
          };
          return (
            <g key={m.key} style={{ color: COUNTRY_COLOR[m.country] }}>
              <line className="march" {...pts} markerEnd="url(#arrow)" />
              <circle cx={dot.x} cy={dot.y} r={4} fill="currentColor" />
              {m.count > 1 && (
                <text x={dot.x} y={dot.y - 8} className="march-count">
                  ×{m.count}
                </text>
              )}
            </g>
          );
        })}
      </g>

      <g className="map-nodes">
        {provinces.map((p) => {
          const { x, y } = layout.pos(p);
          const counts = unitCounts.get(p.id);
          const total = counts ? [...counts.values()].reduce((s, n) => s + n, 0) : 0;
          const combat = combatAt.get(p.id);
          const occupied = p.controller !== p.original_owner;
          const selected = state.selected_province === p.id;
          const terrain = catalog.terrains.find((t) => t.id === p.terrain)?.name ?? p.terrain;
          const tooltip = [
            catalog.provinces.find((c) => c.id === p.id)?.name ?? p.name,
            `控制: ${COUNTRY_SHORT[p.controller]}${occupied ? `（法理 ${COUNTRY_SHORT[p.original_owner]}）` : ""}`,
            counts
              ? `驻军: ${[...counts].map(([c, n]) => `${COUNTRY_SHORT[c]}×${n}`).join(" ")}`
              : "无驻军",
            targeting ? "点击：下达行军命令" : "点击选择 · 右键：命令选中师团前往 · Shift+右键：全军突击",
          ].join("\n");

          return (
            <g
              key={p.id}
              className={`node${selected ? " selected" : ""}${hover === p.id ? " hover" : ""}`}
              transform={`translate(${x} ${y})`}
              onClick={() => onClick(p.id)}
              onContextMenu={(e) => {
                e.preventDefault();
                onQuickOrder(p.id, e.shiftKey);
              }}
              onMouseEnter={() => setHover(p.id)}
              onMouseLeave={() => setHover((h) => (h === p.id ? null : h))}
            >
              <title>{tooltip}</title>
              {combat && <CombatRing combat={combat} />}
              {selected && <circle r={NODE_R + 5} className="select-ring" />}
              <circle
                r={NODE_R}
                fill={COUNTRY_COLOR[p.controller]}
                stroke={occupied ? COUNTRY_COLOR[p.original_owner] : "var(--node-stroke)"}
                strokeWidth={occupied ? 3 : 1.5}
                strokeDasharray={occupied ? "5 3" : undefined}
                className="node-body"
              />
              <text className="node-count" dy="0.35em">
                {total > 0 ? total : ""}
              </text>
              {combat && (
                <text className="node-combat" x={NODE_R - 2} y={-NODE_R + 4}>
                  ⚔
                </text>
              )}
              <text className="node-name" y={NODE_R + 16}>
                {p.name}
              </text>
              <text className="node-meta" y={NODE_R + 30}>
                {terrain} · ★{p.victory_points}
              </text>
            </g>
          );
        })}
      </g>
    </svg>
  );
}

/** 攻方进度环：从顶部顺时针填充 progress% */
function CombatRing({ combat }: { combat: Combat }) {
  const c = 2 * Math.PI * RING_R;
  const filled = (combat.progress / 100) * c;
  return (
    <g className="combat-ring" transform="rotate(-90)">
      <circle r={RING_R} className="combat-track" stroke={COUNTRY_COLOR[combat.defender_country]} />
      <circle
        r={RING_R}
        className="combat-fill"
        stroke={COUNTRY_COLOR[combat.attacker_country]}
        strokeDasharray={`${filled} ${c}`}
      />
    </g>
  );
}

/** 两节点间的连线，两端各让出节点半径 */
function linePoints(a: { x: number; y: number }, b: { x: number; y: number }) {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len = Math.hypot(dx, dy) || 1;
  const inset = NODE_R + 6;
  return {
    x1: a.x + (dx / len) * inset,
    y1: a.y + (dy / len) * inset,
    x2: b.x - (dx / len) * inset,
    y2: b.y - (dy / len) * inset,
  };
}
