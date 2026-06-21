import { serverEnv } from "@/lib/server-env";
import type { GeneratedImage, ImageBackend, ImageMode } from "@/lib/types";

// The worker keys references off name/url/dataUrl only.
export type WorkerReference = {
  name: string;
  dataUrl?: string;
  url: string;
};

export type CallFluxWorkerOptions = {
  prompt: string;
  mode: ImageMode;
  backend: string;
  aspect: string;
  width: number;
  height: number;
  seed?: number;
  references: WorkerReference[];
};

// Discriminated failure the route handlers return verbatim, so each keeps its own
// success-path bookkeeping while sharing one worker call + error shape.
export type CallFluxWorkerResult =
  | { ok: true; image: GeneratedImage }
  | { ok: false; response: Response };

const WORKER_DOWN_EXPECTED =
  "Откройте Images и нажмите Start, или запустите npm run image:server из папки Open Dungeon.";

// MFLUX is Apple-Silicon only; on Windows/Linux the censored (sdnq-hs) and Mac
// (mflux-hs) backends resolve to the uncensored FLUX.2-klein CUDA backend so this
// NSFW app keeps the uncensored text encoder unless flux-uncensored is explicit.
export function resolveImageBackend(requested: ImageBackend): string {
  // Default to the UNGATED sdnq-hs backend (FLUX.2-klein SDNQ). The
  // flux-uncensored model pulls its text encoder from a gated HF repo
  // (ponpoke/flux2-klein-4b-uncensored-text-encoder) and 401s without an HF
  // token, so it must stay opt-in via IMAGE_SERVER_DEFAULT_BACKEND — never the
  // out-of-the-box default, or image generation fails on a fresh install.
  return requested === "mflux-hs" || requested === "sdnq-hs"
    ? serverEnv("IMAGE_SERVER_DEFAULT_BACKEND", "sdnq-hs")
    : requested;
}

// POST the scene to the local flux worker. steps/guidance are honored only by the
// Apple-Silicon mflux path; the full-step CUDA backends force their own values.
export async function callFluxWorker(opts: CallFluxWorkerOptions): Promise<CallFluxWorkerResult> {
  const workerUrl = serverEnv("FLUX_WORKER_URL", "http://127.0.0.1:7869");

  try {
    const upstream = await fetch(`${workerUrl.replace(/\/$/, "")}/generate`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        prompt: opts.prompt,
        mode: opts.mode,
        backend: opts.backend,
        aspect: opts.aspect,
        width: opts.width,
        height: opts.height,
        steps: 4,
        guidance: 0.0,
        seed: opts.seed,
        references: opts.references,
      }),
    });

    if (!upstream.ok) {
      const detail = await upstream.text();
      return {
        ok: false,
        response: Response.json(
          { error: `Flux worker failed (${upstream.status}).`, detail: detail.slice(0, 1000) },
          { status: 502 },
        ),
      };
    }

    return { ok: true, image: (await upstream.json()) as GeneratedImage };
  } catch (error) {
    return {
      ok: false,
      response: Response.json(
        {
          error: "Flux worker is not running.",
          detail: error instanceof Error ? error.message : String(error),
          expected: WORKER_DOWN_EXPECTED,
        },
        { status: 503 },
      ),
    };
  }
}
