export const SONIOX_RT_MODEL = "stt-rt-v6";
const legacy = (value: unknown) => value === "stt-rt-v5" || value === "stt-rt-v4";
const launchKeys = [
  "may.preferences", "may.caption.standalone",
  "may.task_assign.meeting", "may.task_assign.captionsonly",
  "innote.workflow-presets.v1", "may.speech.processing-levels",
];

/** Prepare an atomic upgrade of launch copies; recordings and API stores are never read. */
export function sonioxV6Preferences(storage: Pick<Storage, "getItem">): Record<string, string> {
  const entries: Record<string, string> = {};
  for (const key of launchKeys) {
    const raw = storage.getItem(key);
    if (!raw) continue;
    try {
      const value = JSON.parse(raw);
      let changed = false;
      const upgrade = (item: any): any => {
        if (!item || typeof item !== "object") return item;
        if (Array.isArray(item)) return item.map(upgrade);
        return Object.fromEntries(Object.entries(item).map(([field, content]) => {
          if (field === "model" && legacy(content)) { changed = true; return [field, SONIOX_RT_MODEL]; }
          return [field, upgrade(content)];
        }));
      };
      let next = upgrade(value);
      if (key === "may.speech.processing-levels" && next && typeof next === "object" && !Array.isArray(next)) {
        const upgraded: Record<string, unknown> = {};
        for (const [field, content] of Object.entries(next)) {
          try {
            const parts = JSON.parse(field);
            if (Array.isArray(parts) && parts.length === 5 && legacy(parts[2])) {
              parts[2] = SONIOX_RT_MODEL;
              const target = JSON.stringify(parts);
              // An explicit v6 choice wins over an older v5 preference.
              upgraded[target] = Object.hasOwn(next, target) ? next[target] : content;
              changed = true;
              continue;
            }
          } catch { /* Preserve unrelated processing preferences. */ }
          upgraded[field] = content;
        }
        next = upgraded;
      }
      if (changed) entries[key] = JSON.stringify(next);
    } catch { /* Preserve corrupt values for the existing recovery paths. */ }
  }
  return entries;
}
