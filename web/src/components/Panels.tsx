import { useState } from "react";
import { COUNTRY_COLOR, COUNTRY_SHORT, WAR_CANDIDATES, divisionsIn, formatDate, logTone, orderLabel } from "../display";
import type { Catalog, CountryId, Division, GameState, Targeting, UnitType, UserAction } from "../types";

type Act = (action: UserAction) => void;

function CountryTag({ id }: { id: CountryId }) {
  return (
    <span className="country-tag" style={{ background: COUNTRY_COLOR[id] }}>
      {COUNTRY_SHORT[id]}
    </span>
  );
}

function Meter({ value, max, tone }: { value: number; max: number; tone: string }) {
  const pct = Math.max(0, Math.min(100, (value / max) * 100));
  return (
    <span className={`meter ${tone}`}>
      <span style={{ width: `${pct}%` }} />
    </span>
  );
}

/* ───────────── 顶部状态栏 ───────────── */

export function TopBar({ state, act }: { state: GameState; act: Act }) {
  const player = state.countries[state.player_country];
  const divisions = Object.values(state.divisions).filter((d) => d.country === state.player_country).length;

  return (
    <header className="top-bar">
      <h1>钢铁雄心 · 极简欧战推演</h1>

      <div className="clock">
        <span className="date">{formatDate(state.date)}</span>
        <button
          className={`play ${state.is_paused ? "paused" : "running"}`}
          disabled={state.outcome !== null}
          onClick={() => act({ type: "TogglePause" })}
          title="暂停 / 继续 (空格)"
        >
          {state.is_paused ? "▶ 继续" : "❚❚ 暂停"}
        </button>
        <div className="speed" role="group" aria-label="推演速度">
          {[1, 2, 3, 4, 5].map((s) => (
            <button
              key={s}
              className={state.speed === s ? "active" : ""}
              onClick={() => act({ type: "SetSpeed", args: s })}
              title={`速度 ${s} (按 ${s})`}
            >
              {s}
            </button>
          ))}
        </div>
        <button onClick={() => act({ type: "StepSingleHour" })} title="单步推进 1 小时 (.)">
          +1h
        </button>
      </div>

      <dl className="stats">
        <div>
          <dt>世界紧张度</dt>
          <dd className="tension">
            <Meter value={state.world_tension} max={100} tone="danger" /> {state.world_tension.toFixed(0)}%
          </dd>
        </div>
        <div>
          <dt>政治点数</dt>
          <dd>{player.political_power.toFixed(0)}</dd>
        </div>
        <div>
          <dt>人力</dt>
          <dd>{(player.manpower / 1_000_000).toFixed(2)}M</dd>
        </div>
        <div>
          <dt>民用工厂</dt>
          <dd>{player.civ_factories}</dd>
        </div>
        <div>
          <dt>军用工厂</dt>
          <dd>{player.mil_factories}</dd>
        </div>
        <div>
          <dt>陆军</dt>
          <dd>{divisions} 个师</dd>
        </div>
      </dl>
    </header>
  );
}

/* ───────────── 地块情报 ───────────── */

export function ProvincePanel({ state, catalog, act }: { state: GameState; catalog: Catalog; act: Act }) {
  const prov = state.map.provinces[state.selected_province];
  const terrain = catalog.terrains.find((t) => t.id === prov.terrain)!;
  const fullName = catalog.provinces.find((p) => p.id === prov.id)?.name ?? prov.name;
  const occupied = prov.controller !== prov.original_owner;

  return (
    <section className="panel">
      <h2>{fullName}</h2>
      <dl className="facts">
        <dt>控制方</dt>
        <dd>
          <CountryTag id={prov.controller} />
          {occupied && (
            <span className="warn">
              {" "}
              占领中（法理 <CountryTag id={prov.original_owner} />）
            </span>
          )}
        </dd>
        <dt>胜利点</dt>
        <dd>★ {prov.victory_points}</dd>
        <dt>地形</dt>
        <dd>
          {terrain.name}
          <span className="muted">
            {" "}
            攻 {Math.round(terrain.attack_modifier * 100)}% · 守 {Math.round(terrain.defense_modifier * 100)}%
          </span>
        </dd>
        <dt>工业</dt>
        <dd>
          民工 {prov.civ_factories} · 军工 {prov.mil_factories}
        </dd>
        <dt>基建</dt>
        <dd>
          <Meter value={prov.infrastructure} max={10} tone="info" /> {prov.infrastructure}/10
        </dd>
        <dt>邻接</dt>
        <dd className="chips">
          {prov.neighbors.map((n) => (
            <button key={n} className="chip" onClick={() => act({ type: "SelectProvince", args: n })}>
              {state.map.provinces[n].name}
            </button>
          ))}
        </dd>
      </dl>
    </section>
  );
}

/* ───────────── 驻军与师团卡片 ───────────── */

interface DivisionPanelProps {
  state: GameState;
  catalog: Catalog;
  act: Act;
  targeting: Targeting;
  startTargeting: (allUnits: boolean) => void;
  cancelTargeting: () => void;
}

export function DivisionPanel({ state, catalog, act, targeting, startTargeting, cancelTargeting }: DivisionPanelProps) {
  const units = divisionsIn(state, state.selected_province);
  const own = units.filter((d) => d.country === state.player_country);
  const selected = state.selected_division !== null ? state.divisions[state.selected_division] : undefined;
  const active = selected && selected.location === state.selected_province ? selected : own[0];

  return (
    <section className="panel">
      <h2>
        驻军 <span className="muted">{units.length} 个师</span>
      </h2>

      {targeting ? (
        <div className="targeting-banner">
          <span>
            🎯 {targeting.allUnits ? "全军突击" : `调动 ${active?.name ?? "师团"}`}：点击地图上的目标地块
          </span>
          <button onClick={cancelTargeting}>取消 (Esc)</button>
        </div>
      ) : (
        <div className="actions">
          <button disabled={!active} onClick={() => startTargeting(false)} title="命令选中师团移动/进攻 (M)">
            移动选中师团 <kbd>M</kbd>
          </button>
          <button disabled={own.length === 0} onClick={() => startTargeting(true)} title="本地块所有我方师团发动总攻 (A)">
            全军突击 <kbd>A</kbd>
          </button>
        </div>
      )}

      {units.length === 0 ? (
        <p className="muted">该地块当前无任何武装力量驻守。</p>
      ) : (
        <ul className="division-list">
          {units.map((d) => {
            const mine = d.country === state.player_country;
            return (
              <li key={d.id}>
                <button
                  className={`division-row${active?.id === d.id ? " active" : ""}`}
                  disabled={!mine}
                  onClick={() => act({ type: "SelectDivision", args: d.id })}
                >
                  <CountryTag id={d.country} />
                  <span className="unit-icon">{catalog.unit_types.find((u) => u.id === d.unit_type)?.icon}</span>
                  <span className="division-name">{d.name}</span>
                  <span className="bars">
                    <Meter value={d.organization} max={d.max_organization} tone="org" />
                    <Meter value={d.strength} max={100} tone="hp" />
                  </span>
                  <span className={`status ${d.order.kind.toLowerCase()}`}>{orderLabel(d)}</span>
                </button>
              </li>
            );
          })}
        </ul>
      )}

      {active && <DivisionCard division={active} catalog={catalog} />}
    </section>
  );
}

function DivisionCard({ division: d, catalog }: { division: Division; catalog: Catalog }) {
  const type = catalog.unit_types.find((u) => u.id === d.unit_type)!;
  return (
    <div className="division-card">
      <h3>
        {d.name} <span className="muted">{type.name}</span>
      </h3>
      <div className="gauge">
        <span>组织度</span>
        <Meter value={d.organization} max={d.max_organization} tone="org" />
        <span className="num">
          {d.organization.toFixed(0)}/{d.max_organization.toFixed(0)}
        </span>
      </div>
      <div className="gauge">
        <span>兵力</span>
        <Meter value={d.strength} max={100} tone="hp" />
        <span className="num">{d.strength.toFixed(0)}%</span>
      </div>
      <dl className="stat-grid">
        <dt>软攻</dt>
        <dd>{d.soft_attack}</dd>
        <dt>硬攻</dt>
        <dd>{d.hard_attack}</dd>
        <dt>防御</dt>
        <dd>{d.defense}</dd>
        <dt>突破</dt>
        <dd>{d.breakthrough}</dd>
        <dt>速度</dt>
        <dd>{d.speed_kmh} km/h</dd>
      </dl>
    </div>
  );
}

/* ───────────── 军工生产 ───────────── */

const PRODUCIBLE: UnitType[] = ["Infantry", "Motorized", "Panzer"];

export function ProductionPanel({ state, catalog, act }: { state: GameState; catalog: Catalog; act: Act }) {
  const player = state.countries[state.player_country];
  const typeOf = (id: UnitType) => catalog.unit_types.find((u) => u.id === id)!;
  const assigned = player.production_queue.reduce((sum, line) => sum + line.assigned_factories, 0);
  const free = Math.max(0, player.mil_factories - assigned);

  return (
    <section className="panel">
      <h2>
        军工生产线{" "}
        <span className="muted">
          空闲军工厂 {free}/{player.mil_factories}
        </span>
      </h2>
      <div className="actions">
        {PRODUCIBLE.map((id) => {
          const t = typeOf(id);
          return (
            <button
              key={id}
              disabled={free === 0}
              onClick={() => act({ type: "QueueProduction", args: id })}
              title={free === 0 ? "没有空闲军工厂，请先撤销一条生产线" : `成本 ${t.production_cost} · 人力 ${t.manpower_cost / 1000}k`}
            >
              + {t.name}
              {id === "Infantry" && <kbd>P</kbd>}
              {id === "Panzer" && <kbd>⇧P</kbd>}
            </button>
          );
        })}
      </div>
      {player.production_queue.length === 0 ? (
        <p className="muted">生产线空闲中。</p>
      ) : (
        <ul className="production-list">
          {player.production_queue.map((line, i) => {
            const t = typeOf(line.unit_type);
            const waitingForManpower = line.progress >= t.production_cost;
            return (
              <li key={i}>
                <span className="unit-icon">{t.icon}</span>
                <span>{t.name}</span>
                <span className="muted">{line.assigned_factories} 工厂</span>
                <Meter value={line.progress} max={t.production_cost} tone="prod" />
                {waitingForManpower ? (
                  <span className="warn">缺人力</span>
                ) : (
                  <span className="num">{((line.progress / t.production_cost) * 100).toFixed(0)}%</span>
                )}
                <button
                  className="icon-button"
                  onClick={() => act({ type: "CancelProduction", args: i })}
                  title="撤销生产线，释放军工厂"
                  aria-label={`撤销 ${t.name} 生产线`}
                >
                  ×
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

/* ───────────── 外交 ───────────── */

export function DiplomacyPanel({ state, act }: { state: GameState; act: Act }) {
  const [confirming, setConfirming] = useState(false);
  const player = state.countries[state.player_country];
  const next =
    state.outcome === null
      ? WAR_CANDIDATES.find((c) => !player.enemies.includes(c) && !state.countries[c].has_capitulated)
      : undefined;
  const vpShare = (id: CountryId) => {
    const provs = Object.values(state.map.provinces);
    const orig = provs.filter((p) => p.original_owner === id).reduce((s, p) => s + p.victory_points, 0);
    const cur = provs.filter((p) => p.controller === id).reduce((s, p) => s + p.victory_points, 0);
    return orig > 0 ? Math.round((cur / orig) * 100) : 0;
  };

  return (
    <section className="panel">
      <h2>外交态势</h2>
      <ul className="diplomacy-list">
        {Object.values(state.countries)
          .filter((c) => c.id !== state.player_country)
          .map((c) => {
            const status = c.has_capitulated ? "已投降" : player.enemies.includes(c.id) ? "交战中" : "中立";
            return (
              <li key={c.id}>
                <CountryTag id={c.id} />
                <span className={`relation ${c.has_capitulated ? "capitulated" : player.enemies.includes(c.id) ? "war" : "peace"}`}>
                  {status}
                </span>
                <span className="muted">胜利点 {vpShare(c.id)}%</span>
              </li>
            );
          })}
      </ul>
      {next &&
        (confirming ? (
          <div className="confirm">
            <span>确认对 {COUNTRY_SHORT[next]} 宣战？世界紧张度将上升。</span>
            <button
              className="danger"
              onClick={() => {
                act({ type: "DeclareWarNextEnemy" });
                setConfirming(false);
              }}
            >
              宣战
            </button>
            <button onClick={() => setConfirming(false)}>取消</button>
          </div>
        ) : (
          <button className="danger-outline" onClick={() => setConfirming(true)}>
            对 {COUNTRY_SHORT[next]} 宣战…
          </button>
        ))}
    </section>
  );
}

/* ───────────── 战报 ───────────── */

export function LogPanel({ logs }: { logs: string[] }) {
  return (
    <section className="panel log-panel">
      <h2>战报简讯</h2>
      <ol className="log-list">
        {logs
          .slice(-40)
          .reverse()
          .map((log, i) => (
            <li key={logs.length - i} className={`log ${logTone(log)}`}>
              {log}
            </li>
          ))}
      </ol>
    </section>
  );
}

/* ───────────── 终局 ───────────── */

export function OutcomeOverlay({ state, onRestart }: { state: GameState; onRestart: () => void }) {
  if (state.outcome === null) return null;
  const victory = state.outcome === "Victory";
  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-labelledby="outcome-title">
      <div className={`overlay-card ${victory ? "victory" : "defeat"}`}>
        <h2 id="outcome-title">{victory ? "胜利" : "战败"}</h2>
        <p>{victory ? "所有列强均已投降，欧陆战争以我方全面胜利告终。" : "我国已签署投降书，战争失败。"}</p>
        <p className="muted">{formatDate(state.date)}</p>
        <button onClick={onRestart}>重新开局</button>
      </div>
    </div>
  );
}
