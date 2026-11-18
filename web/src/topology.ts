// 省份几何：解码模组的 TopoJSON，投影为平面坐标并预构建 Path2D。
// 没有轮廓文件的模组（如 europe-mini）退化为以省份中心为圆心的节点。

import type { Province, ProvinceId } from "./types";

/** 每经度对应的平面单位 */
const UNITS_PER_DEG = 10;
/** 无轮廓模组中节点的半径（平面单位） */
const NODE_RADIUS = 9;
/** 地球一周对应的平面宽度 */
export const WORLD_WIDTH = 360 * UNITS_PER_DEG;
/** 覆盖经度超过此比例的地图视为全球地图，东西向环绕 */
const GLOBAL_SPAN = 0.95;

export type Point = [number, number];

/** 米勒圆柱投影：高纬度拉伸比墨卡托温和，适合全球战略地图 */
export function project([lon, lat]: [number, number]): Point {
  const phi = (Math.max(-85, Math.min(85, lat)) * Math.PI) / 180;
  const y = 1.25 * Math.log(Math.tan(Math.PI / 4 + 0.4 * phi));
  return [lon * UNITS_PER_DEG, (-y * 180 * UNITS_PER_DEG) / Math.PI];
}

export interface ProvinceShape {
  path: Path2D;
  /** [minX, minY, maxX, maxY] */
  bbox: [number, number, number, number];
  center: Point;
}

/** 一段边界：两侧省份（海岸线只有一侧）；路径按细节层级取自 GeometryLevel.arcs */
export interface BorderArc {
  left: ProvinceId;
  right: ProvinceId | null;
  bbox: [number, number, number, number];
}

/** 某一简化精度下的全部路径 */
export interface GeometryLevel {
  /** 省份轮廓（evenodd 填充） */
  shapes: Map<ProvinceId, Path2D>;
  /** 与 MapGeometry.arcs 一一对应 */
  arcs: Path2D[];
}

/** 简化所需的原始数据与已构建的层级缓存 */
interface Lod {
  /** 与 MapGeometry.arcs 一一对应的投影坐标 */
  points: Point[][];
  /** 各省的环，每个环为弧段索引序列（负数为反向） */
  rings: Map<ProvinceId, number[][]>;
  levels: Map<number, GeometryLevel>;
}

export interface MapGeometry {
  /** 是否有真实轮廓；否则为节点图 */
  hasShapes: boolean;
  shapes: Map<ProvinceId, ProvinceShape>;
  arcs: BorderArc[];
  /** 陆地接壤的省份对 "a-b"（a < b）；其余邻接关系是海峡或航线 */
  landLinks: Set<string>;
  bounds: [number, number, number, number];
  /** 东西向环绕的周期（平面单位）：全球地图为 360°，节点图不环绕 */
  wrap: number | null;
  /** 节点图没有可简化的轮廓 */
  lod: Lod | null;
}

interface Topology {
  type: "Topology";
  transform?: { scale: [number, number]; translate: [number, number] };
  arcs: number[][][];
  objects: Record<string, { type: "GeometryCollection"; geometries: TopoGeometry[] }>;
}

type TopoGeometry =
  | { type: "Polygon"; arcs: number[][]; properties: { id: ProvinceId } }
  | { type: "MultiPolygon"; arcs: number[][][]; properties: { id: ProvinceId } }
  | { type: null; properties: { id: ProvinceId } };

export const linkKey = (a: ProvinceId, b: ProvinceId) => (a < b ? `${a}-${b}` : `${b}-${a}`);

/** 解码量化、差分编码的弧段，返回投影后的坐标 */
function decodeArcs(topo: Topology): Point[][] {
  const t = topo.transform;
  return topo.arcs.map((arc) => {
    let x = 0;
    let y = 0;
    return arc.map(([dx, dy]) => {
      if (t) {
        x += dx;
        y += dy;
        return project([x * t.scale[0] + t.translate[0], y * t.scale[1] + t.translate[1]]);
      }
      return project([dx, dy]);
    });
  });
}

function emptyBounds(): [number, number, number, number] {
  return [Infinity, Infinity, -Infinity, -Infinity];
}

function extend(b: [number, number, number, number], [x, y]: Point) {
  b[0] = Math.min(b[0], x);
  b[1] = Math.min(b[1], y);
  b[2] = Math.max(b[2], x);
  b[3] = Math.max(b[3], y);
}

export function buildGeometry(topo: Topology, provinces: Province[]): MapGeometry {
  const decoded = decodeArcs(topo);
  const objectName = "provinces" in topo.objects ? "provinces" : Object.keys(topo.objects)[0];
  const geometries = topo.objects[objectName].geometries;

  const shapes = new Map<ProvinceId, ProvinceShape>();
  const provinceRings = new Map<ProvinceId, number[][]>();
  const arcUsers: ProvinceId[][] = decoded.map(() => []);
  const bounds = emptyBounds();
  const centerOf = new Map(provinces.map((p) => [p.id, project(p.center)]));

  for (const g of geometries) {
    const id = g.properties.id;
    const center = centerOf.get(id);
    if (g.type === null || center === undefined) continue;
    const rings = (g.type === "Polygon" ? [g.arcs] : g.arcs).flat();
    const bbox = emptyBounds();
    for (const ring of rings) {
      for (const index of ring) {
        arcUsers[index >= 0 ? index : ~index].push(id);
        for (const p of decoded[index >= 0 ? index : ~index]) extend(bbox, p);
      }
    }
    provinceRings.set(id, rings);
    shapes.set(id, { path: ringsPath(rings, decoded), bbox, center });
    extend(bounds, [bbox[0], bbox[1]]);
    extend(bounds, [bbox[2], bbox[3]]);
  }

  // 只保留被省份引用的弧段，并重新编号
  const landLinks = new Set<string>();
  const borderArcs: BorderArc[] = [];
  const points: Point[][] = [];
  const renumber = new Map<number, number>();
  decoded.forEach((pts, i) => {
    const users = [...new Set(arcUsers[i])];
    if (users.length === 0) return;
    const bbox = emptyBounds();
    pts.forEach((p) => extend(bbox, p));
    renumber.set(i, points.length);
    points.push(pts);
    borderArcs.push({ left: users[0], right: users[1] ?? null, bbox });
    if (users.length >= 2) landLinks.add(linkKey(users[0], users[1]));
  });
  for (const rings of provinceRings.values()) {
    for (const ring of rings) {
      ring.forEach((index, k) => (ring[k] = index >= 0 ? renumber.get(index)! : ~renumber.get(~index)!));
    }
  }

  const lod: Lod = { points, rings: provinceRings, levels: new Map() };
  const wrap = bounds[2] - bounds[0] >= GLOBAL_SPAN * WORLD_WIDTH ? WORLD_WIDTH : null;
  return { hasShapes: true, shapes, arcs: borderArcs, landLinks, bounds, wrap, lod };
}

/** 由弧段拼出闭合环 */
function ringsPath(rings: number[][], arcs: Point[][]): Path2D {
  const path = new Path2D();
  for (const ring of rings) {
    ring.forEach((index, k) => {
      const arc = arcs[index >= 0 ? index : ~index];
      const n = arc.length;
      // 相邻弧段首尾重合，去掉重复点
      for (let j = k === 0 ? 0 : 1; j < n; j++) {
        const [x, y] = arc[index >= 0 ? j : n - 1 - j];
        if (k === 0 && j === 0) path.moveTo(x, y);
        else path.lineTo(x, y);
      }
    });
    path.closePath();
  }
  return path;
}

const onDateline = (x: number) => Math.abs(Math.abs(x) - WORLD_WIDTH / 2) < 0.5;

/** 边界折线；两端都在 ±180° 经线上的线段是数据切开地球时产生的假海岸线，跳过不画 */
function polyline(pts: Point[]): Path2D {
  const path = new Path2D();
  pts.forEach(([x, y], k) => {
    if (k === 0 || (onDateline(x) && onDateline(pts[k - 1][0]))) path.moveTo(x, y);
    else path.lineTo(x, y);
  });
  return path;
}

/** 点到线段距离的平方 */
function segmentDist2([px, py]: Point, [ax, ay]: Point, [bx, by]: Point): number {
  const dx = bx - ax;
  const dy = by - ay;
  const len2 = dx * dx + dy * dy;
  const t = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / len2));
  const ex = ax + t * dx - px;
  const ey = ay + t * dy - py;
  return ex * ex + ey * ey;
}

/** Douglas–Peucker 简化，保留首尾点（共享弧段两侧因此仍严丝合缝） */
function simplify(pts: Point[], tolerance: number): Point[] {
  const n = pts.length;
  if (n <= 2) return pts;
  const keep = new Uint8Array(n);
  keep[0] = keep[n - 1] = 1;
  const tol2 = tolerance * tolerance;
  const stack = [0, n - 1];
  while (stack.length) {
    const b = stack.pop()!;
    const a = stack.pop()!;
    let max = 0;
    let at = -1;
    for (let i = a + 1; i < b; i++) {
      const d = segmentDist2(pts[i], pts[a], pts[b]);
      if (d > max) {
        max = d;
        at = i;
      }
    }
    if (max > tol2) {
      keep[at] = 1;
      stack.push(a, at, at, b);
    }
  }
  // 闭合的单弧环（小岛）至少保留一个中间点，避免退化为线段后消失
  if (keep.reduce((s, k) => s + k, 0) === 2 && pts[0][0] === pts[n - 1][0] && pts[0][1] === pts[n - 1][1]) {
    keep[n >> 1] = 1;
  }
  return pts.filter((_, i) => keep[i]);
}

/** 简化容差为 2^k 平面单位；低于 2^MIN_LEVEL 时直接使用原始精度 */
const MIN_LEVEL = -4;
/** 屏幕上允许的简化误差（像素） */
const LOD_PX = 0.5;

const nodeLevels = new WeakMap<MapGeometry, GeometryLevel>();

/** 取适合当前缩放的几何层级（按需构建并缓存） */
export function geometryLevel(geo: MapGeometry, scale: number): GeometryLevel {
  const lod = geo.lod;
  if (!lod) {
    let level = nodeLevels.get(geo);
    if (!level) {
      level = { shapes: new Map([...geo.shapes].map(([id, s]) => [id, s.path])), arcs: [] };
      nodeLevels.set(geo, level);
    }
    return level;
  }
  const k = Math.max(MIN_LEVEL - 1, Math.floor(Math.log2(LOD_PX / scale)));
  let level = lod.levels.get(k);
  if (!level) {
    const points = k < MIN_LEVEL ? lod.points : lod.points.map((pts) => simplify(pts, 2 ** k));
    const shapes = new Map<ProvinceId, Path2D>();
    for (const [id, rings] of lod.rings) shapes.set(id, ringsPath(rings, points));
    level = { shapes, arcs: points.map(polyline) };
    lod.levels.set(k, level);
  }
  return level;
}

/** 无轮廓模组：每个省份画成一个圆形节点，邻接全部按连线绘制 */
export function buildNodeGeometry(provinces: Province[]): MapGeometry {
  const shapes = new Map<ProvinceId, ProvinceShape>();
  const bounds = emptyBounds();
  for (const p of provinces) {
    const center = project(p.center);
    const path = new Path2D();
    path.arc(center[0], center[1], NODE_RADIUS, 0, Math.PI * 2);
    const bbox: [number, number, number, number] = [
      center[0] - NODE_RADIUS,
      center[1] - NODE_RADIUS,
      center[0] + NODE_RADIUS,
      center[1] + NODE_RADIUS,
    ];
    shapes.set(p.id, { path, bbox, center });
    extend(bounds, [bbox[0], bbox[1]]);
    extend(bounds, [bbox[2], bbox[3]]);
  }
  return { hasShapes: false, shapes, arcs: [], landLinks: new Set(), bounds, wrap: null, lod: null };
}

export async function loadGeometry(mapUrl: string | null, provinces: Province[]): Promise<MapGeometry> {
  if (!mapUrl) return buildNodeGeometry(provinces);
  const res = await fetch(mapUrl);
  if (!res.ok) throw new Error(`读取地图轮廓 ${mapUrl} 失败：HTTP ${res.status}`);
  return buildGeometry((await res.json()) as Topology, provinces);
}

/** 把横坐标归一到 [-period/2, period/2) */
export function wrapX(x: number, period: number): number {
  return ((((x + period / 2) % period) + period) % period) - period / 2;
}

/** 环绕地图上 bbox 与 [x0, x1] 相交的各个副本的横向偏移；不环绕时可见则为 [0] */
export function copyOffsets(bbox: readonly number[], x0: number, x1: number, period: number | null): number[] {
  if (!period) return bbox[2] >= x0 && bbox[0] <= x1 ? [0] : [];
  const out: number[] = [];
  for (let k = Math.ceil((x0 - bbox[2]) / period); k <= Math.floor((x1 - bbox[0]) / period); k++) out.push(k * period);
  return out;
}

/** 屏幕上小于该尺寸（像素）的省份按此半径的圆点命中，并绘制标记 */
export const TINY_PX = 8;

/** 省份在当前缩放下是否小到需要按圆点处理 */
export function isTiny(s: ProvinceShape, scale: number): boolean {
  return Math.max(s.bbox[2] - s.bbox[0], s.bbox[3] - s.bbox[1]) * scale < TINY_PX;
}

/** 平面坐标命中测试：微小省份（直布罗陀、马耳他……）按圆点优先命中；其余先用包围盒筛选，再做精确的路径判定 */
export function hitTest(geo: MapGeometry, ctx: CanvasRenderingContext2D, [rawX, y]: Point, scale: number): ProvinceId | null {
  const x = geo.wrap ? wrapX(rawX, geo.wrap) : rawX;
  const radius = TINY_PX / scale;
  let tiny: ProvinceId | null = null;
  let best = radius * radius;
  for (const [id, s] of geo.shapes) {
    if (!isTiny(s, scale)) continue;
    const dx = geo.wrap ? wrapX(s.center[0] - x, geo.wrap) : s.center[0] - x;
    const d = dx ** 2 + (s.center[1] - y) ** 2;
    if (d <= best) {
      best = d;
      tiny = id;
    }
  }
  if (tiny !== null) return tiny;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  for (const [id, s] of geo.shapes) {
    const [x0, y0, x1, y1] = s.bbox;
    if (x < x0 || x > x1 || y < y0 || y > y1) continue;
    if (ctx.isPointInPath(s.path, x, y, "evenodd")) return id;
  }
  return null;
}
