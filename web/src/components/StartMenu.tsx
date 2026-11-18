import { useEffect, useMemo, useState } from "react";
import { type LoadedMods, type ModIndex, type ModManifest, fetchManifest, fetchModIndex, listScenarios, loadMods } from "../mods";
import type { Setup } from "../game";

interface CountryOption {
  tag: string;
  name: string;
  color: string;
}

/** 从各层数据汇总国家名称与颜色（后加载的覆盖先前的） */
function listCountries(mods: LoadedMods): Map<string, CountryOption> {
  const out = new Map<string, CountryOption>();
  for (const layer of mods.layers) {
    for (const file of layer.files) {
      for (const c of (file as { countries?: CountryOption[] }).countries ?? []) {
        out.set(c.tag, { ...out.get(c.tag), ...c });
      }
    }
  }
  return out;
}

interface Props {
  initial: Setup | null;
  error: string | null;
  onStart: (setup: Setup, mods: LoadedMods) => void;
}

/** 开局界面：选择模组、剧本与操控的国家 */
export function StartMenu({ initial, error, onStart }: Props) {
  const [index, setIndex] = useState<ModIndex | null>(null);
  const [manifests, setManifests] = useState<ModManifest[]>([]);
  const [modId, setModId] = useState<string | null>(initial?.mod ?? null);
  const [mods, setMods] = useState<LoadedMods | null>(null);
  const [scenarioId, setScenarioId] = useState<string | null>(initial?.scenario ?? null);
  const [player, setPlayer] = useState<string | null>(initial?.player ?? null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    fetchModIndex()
      .then(async (idx) => {
        const loaded = await Promise.all(idx.mods.map(fetchManifest));
        if (!active) return;
        setIndex(idx);
        setManifests(loaded);
        setModId((m) => m ?? idx.default.mod);
      })
      .catch((e) => { if (active) setLoadError(String(e)); });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (!modId) return;
    let active = true;
    setMods(null);
    setLoadError(null);
    loadMods(modId)
      .then((loaded) => { if (active) setMods(loaded); })
      .catch((e) => { if (active) setLoadError(String(e)); });
    return () => { active = false; };
  }, [modId]);

  const scenarios = useMemo(() => (mods ? listScenarios(mods.layers) : []), [mods]);
  const scenario = scenarios.find((s) => s.id === scenarioId) ?? scenarios.find((s) => s.id === index?.default.scenario) ?? scenarios[0];
  const countries = useMemo(() => (mods ? listCountries(mods) : new Map<string, CountryOption>()), [mods]);
  const playable = (scenario?.countries ?? [])
    .map((tag) => countries.get(tag) ?? { tag, name: tag, color: "#777" })
    .sort((a, b) => a.name.localeCompare(b.name, "zh"));
  const chosen = player && scenario?.countries.includes(player) ? player : scenario?.player;

  // 只有“基础规则”之类不含剧本的模组不可单独开局
  const startable = manifests.filter((m) => m.depends.length > 0 || m.data.some((d) => d.includes("scenario")));

  return (
    <div className="start-menu">
      <div className="start-intro">
        <span className="eyebrow">STRATEGIC COMMAND · 1936—1945</span>
        <div className="archive-mark" aria-hidden="true">1936<span>— 1945 —</span></div>
        <h2>运筹帷幄<br />重绘世界格局</h2>
        <p>从一纸作战计划开始。统筹工业、部署师团，在动荡的年代推演世界的走向。</p>
        <span className="archive-caption">战区档案 / WORLD WAR II</span>
      </div>
      <div className="start-card">
        <div className="dossier-heading"><span>作战档案 / CAMPAIGN DOSSIER</span><span>参谋部</span></div>
        <h1>钢铁雄心</h1>
        <p className="muted">极简二战推演 · 选择战区、历史剧本与指挥国家。</p>

        {(error || loadError) && <pre className="load-error">{error ?? loadError}</pre>}

        <label>
          01 / 战区模组
          <select value={modId ?? ""} onChange={(e) => setModId(e.target.value)}>
            {startable.map((m) => (
              <option key={m.id} value={m.id}>
                {m.name}（{m.id} v{m.version}）
              </option>
            ))}
          </select>
        </label>
        {manifests.find((m) => m.id === modId)?.description && (
          <p className="muted">{manifests.find((m) => m.id === modId)!.description}</p>
        )}

        <label>
          02 / 历史剧本
          <select value={scenario?.id ?? ""} onChange={(e) => setScenarioId(e.target.value)} disabled={!mods}>
            {scenarios.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </label>
        {scenario?.description && <p className="muted">{scenario.description}</p>}

        <label>
          03 / 指挥国家
          <select value={chosen ?? ""} onChange={(e) => setPlayer(e.target.value)} disabled={!mods}>
            {playable.map((c) => (
              <option key={c.tag} value={c.tag}>
                {c.name}（{c.tag}）{c.tag === scenario?.player ? " · 推荐" : ""}
              </option>
            ))}
          </select>
        </label>

        <button
          className="start-button"
          disabled={!mods || !scenario}
          onClick={() => mods && scenario && onStart({ mod: modId!, scenario: scenario.id, player: chosen ?? null }, mods)}
        >
          {mods ? "签发命令 · 开始推演 →" : "正在调阅战区档案…"}
        </button>
      </div>
    </div>
  );
}
