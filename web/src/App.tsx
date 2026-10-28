import { useCallback, useEffect, useState } from "react";
import type { Game } from "./wasm/ww2.js";
import { useGame } from "./game";
import { WarMap } from "./components/WarMap";
import { DiplomacyPanel, DivisionPanel, LogPanel, OutcomeOverlay, ProductionPanel, ProvincePanel, TopBar } from "./components/Panels";
import type { Catalog, ProvinceId, Targeting } from "./types";

export function App({ game, catalog }: { game: Game; catalog: Catalog }) {
  const { state, act, restart } = useGame(game);
  // 行军部署只存在于前端：选中部队后直接点击目标，选中师团不会被途经地块改写
  const [targeting, setTargeting] = useState<Targeting>(null);

  const ownUnitsHere = Object.values(state.divisions).some(
    (d) => d.location === state.selected_province && d.country === state.player_country,
  );

  const startTargeting = useCallback(
    (allUnits: boolean) => {
      if (allUnits ? ownUnitsHere : state.selected_division !== null) {
        setTargeting({ allUnits, source: state.selected_province });
      }
    },
    [ownUnitsHere, state.selected_division, state.selected_province],
  );

  const order = useCallback(
    (target: ProvinceId, allUnits: boolean, source: ProvinceId) => {
      act(
        allUnits
          ? { type: "OrderAllUnitsInProvince", args: [source, target] }
          : { type: "OrderMoveSelected", args: target },
      );
    },
    [act],
  );

  const onMapClick = (id: ProvinceId) => {
    if (targeting) {
      order(id, targeting.allUnits, targeting.source);
      setTargeting(null);
    } else {
      act({ type: "SelectProvince", args: id });
    }
  };

  const onQuickOrder = (id: ProvinceId, allUnits: boolean) => {
    setTargeting(null);
    order(id, allUnits, state.selected_province);
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (e.target instanceof HTMLInputElement) return;

      const arrows: Record<string, [number, number]> = {
        ArrowUp: [0, -1],
        ArrowDown: [0, 1],
        ArrowLeft: [-1, 0],
        ArrowRight: [1, 0],
      };

      if (e.key in arrows) {
        if (!targeting) act({ type: "NavigateMap", args: arrows[e.key] });
      } else if (e.key === " ") {
        act({ type: "TogglePause" });
      } else if ("12345".includes(e.key) && e.key.length === 1) {
        act({ type: "SetSpeed", args: Number(e.key) });
      } else if (e.key === ".") {
        act({ type: "StepSingleHour" });
      } else if (e.key === "Tab") {
        act({ type: "SelectNextDivision" });
      } else if (e.key === "m" || e.key === "M") {
        startTargeting(false);
      } else if (e.key === "a" || e.key === "A") {
        startTargeting(true);
      } else if (e.key === "Escape") {
        setTargeting(null);
      } else if (e.key === "p") {
        act({ type: "QueueProduction", args: "Infantry" });
      } else if (e.key === "P") {
        act({ type: "QueueProduction", args: "Panzer" });
      } else {
        return;
      }
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [act, startTargeting, targeting]);

  return (
    <div className="app">
      <TopBar state={state} act={act} />
      <main className="map-area">
        <WarMap state={state} catalog={catalog} targeting={targeting} onClick={onMapClick} onQuickOrder={onQuickOrder} />
        <p className="map-legend">
          点击选择地块 · 右键命令选中师团前往 · Shift+右键全军突击 · 空格暂停 · 1–5 调速 · Tab 切换师团
        </p>
      </main>
      <aside className="sidebar">
        <ProvincePanel state={state} catalog={catalog} act={act} />
        <DivisionPanel
          state={state}
          catalog={catalog}
          act={act}
          targeting={targeting}
          startTargeting={startTargeting}
          cancelTargeting={() => setTargeting(null)}
        />
      </aside>
      <div className="bottom">
        <ProductionPanel state={state} catalog={catalog} act={act} />
        <DiplomacyPanel state={state} act={act} />
        <LogPanel logs={state.logs} />
      </div>
      <OutcomeOverlay
        state={state}
        onRestart={() => {
          setTargeting(null);
          restart();
        }}
      />
    </div>
  );
}
