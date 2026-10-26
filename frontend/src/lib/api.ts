// Снимок железа для монитора ресурсов. Остальные вызовы интерфейс делает напрямую, как в
// оригинальной версии: пути относительные, приложение и API живут на одном origin.

import { SELF_POLL } from "./studioHub";

export type HwSnapshot = {
  gpuName: string; totalVram: number; usedVram: number; freeVram: number;
  gpuUtilization: number; temperature: number; powerDraw: number; powerLimit: number;
  processRam: number; totalRam: number; usedRam: number; message: string;
};

export const api = {
  hwSnapshot: async (): Promise<HwSnapshot> => {
    const response = await fetch("/api/hw", SELF_POLL);
    if (!response.ok) throw new Error("монитор недоступен");
    return (await response.json()) as HwSnapshot;
  },
};
