/** Java `org.omegat.core.data.DataUtils`. */

export type NearString = {
  comesFrom: "TM" | "MEMORY" | "GLOSSARY" | string;
  projs: string[];
};

/** Java `DataUtils.isFromMTMemory`: `comesFrom == TM` and path is under `tm/mt`. */
export function isFromMTMemory(near: NearString | null, tmRoot: string): boolean {
  if (near == null) return false;
  if (near.comesFrom !== "TM") return false;
  const proj = near.projs[0];
  if (!proj) return false;
  const mt = tmRoot.replace(/\\/g, "/").replace(/\/?$/, "/") + "mt";
  const p = proj.replace(/\\/g, "/");
  return p === mt || p.startsWith(mt + "/") || p.includes("/mt/");
}
