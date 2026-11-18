// 模组加载：读取 public/mods 下的模组清单，按依赖顺序拉取数据文件交给核心合并。

const MODS_ROOT = `${import.meta.env.BASE_URL}mods`;

export interface ModManifest {
  id: string;
  name: string;
  version: string;
  description: string;
  depends: string[];
  /** 数据文件，相对模组目录 */
  data: string[];
  /** 省份轮廓（TopoJSON），相对模组目录；后加载的模组覆盖先前的 */
  map?: string | null;
}

export interface ModIndex {
  mods: string[];
  default: { mod: string; scenario: string };
}

/** 按加载顺序排列的模组：传给 wasm 的 `[{ id, files }]` */
export interface ModLayer {
  id: string;
  files: unknown[];
}

export interface LoadedMods {
  manifests: ModManifest[];
  layers: ModLayer[];
  /** 省份轮廓地址；所有模组都未提供时为 null，地图退化为节点图 */
  mapUrl: string | null;
}

export interface ScenarioSummary {
  id: string;
  name: string;
  description: string;
  player: string;
  countries: string[];
}

// Share concurrent reads (including StrictMode mounts), but allow later reloads.
const pendingJson = new Map<string, Promise<unknown>>();
function fetchJson<T>(url: string): Promise<T> {
  let pending = pendingJson.get(url);
  if (!pending) {
    pending = fetch(url).then(async (res) => {
      if (!res.ok) throw new Error(`读取 ${url} 失败：HTTP ${res.status}`);
      return res.json();
    }).finally(() => pendingJson.delete(url));
    pendingJson.set(url, pending);
  }
  return pending as Promise<T>;
}

export const fetchModIndex = () => fetchJson<ModIndex>(`${MODS_ROOT}/index.json`);

export const fetchManifest = (id: string) => fetchJson<ModManifest>(`${MODS_ROOT}/${id}/mod.json`);

/** 深度优先展开依赖，得到依赖在前的加载顺序；检测循环依赖 */
export async function resolveLoadOrder(id: string): Promise<ModManifest[]> {
  const order: ModManifest[] = [];
  const visiting = new Set<string>();
  const done = new Set<string>();

  const visit = async (modId: string) => {
    if (done.has(modId)) return;
    if (visiting.has(modId)) throw new Error(`模组循环依赖：${[...visiting, modId].join(" → ")}`);
    visiting.add(modId);
    const manifest = await fetchManifest(modId);
    for (const dep of manifest.depends ?? []) await visit(dep);
    visiting.delete(modId);
    done.add(modId);
    order.push(manifest);
  };

  await visit(id);
  return order;
}

export async function loadMods(id: string): Promise<LoadedMods> {
  const manifests = await resolveLoadOrder(id);
  const layers = await Promise.all(
    manifests.map(async (m) => ({
      id: m.id,
      files: await Promise.all(m.data.map((file) => fetchJson<unknown>(`${MODS_ROOT}/${m.id}/${file}`))),
    })),
  );
  const withMap = manifests.filter((m) => m.map).at(-1);
  return { manifests, layers, mapUrl: withMap ? `${MODS_ROOT}/${withMap.id}/${withMap.map}` : null };
}

type Entry = Record<string, unknown>;

/** 从各层数据中汇总剧本列表（后加载的同 id 剧本覆盖先前的字段），供开局界面选择 */
export function listScenarios(layers: ModLayer[]): ScenarioSummary[] {
  const byId = new Map<string, Entry>();
  for (const layer of layers) {
    for (const file of layer.files) {
      const scenarios = (file as { scenarios?: Entry[] }).scenarios ?? [];
      for (const sc of scenarios) {
        const id = String(sc.id);
        if (sc._delete) byId.delete(id);
        else byId.set(id, { ...byId.get(id), ...sc });
      }
    }
  }
  return [...byId.values()].map((sc) => ({
    id: String(sc.id),
    name: String(sc.name ?? sc.id),
    description: String(sc.description ?? ""),
    player: String(sc.player),
    countries: Object.keys((sc.owners as Entry) ?? {}),
  }));
}
