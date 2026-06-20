import { serverEnv } from "@/lib/server-env";
import { LOCAL_TEXT_MODEL_IDS } from "@/lib/text-models";

export const runtime = "nodejs";

export async function GET() {
  const workerUrl = serverEnv("FLUX_WORKER_URL", "http://127.0.0.1:7869");
  const ollamaUrl = serverEnv("OLLAMA_BASE_URL", "http://127.0.0.1:11434");

  let flux = { ok: false, loaded: false };
  try {
    const response = await fetch(`${workerUrl.replace(/\/$/, "")}/health`, {
      cache: "no-store",
    });
    if (response.ok) {
      flux = await response.json();
    }
  } catch {
    flux = { ok: false, loaded: false };
  }

  let localText = { ok: false, installedModels: [] as string[] };
  try {
    const response = await fetch(`${ollamaUrl.replace(/\/$/, "")}/api/tags`, {
      cache: "no-store",
    });
    if (response.ok) {
      const data = (await response.json()) as { models?: Array<{ name?: string }> };
      const installed = new Set(
        (data.models || []).map((model) => model.name || "").filter(Boolean),
      );
      localText = {
        ok: true,
        installedModels: LOCAL_TEXT_MODEL_IDS.filter((id) => installed.has(id)),
      };
    }
  } catch {
    localText = { ok: false, installedModels: [] };
  }

  // Probe the configured custom server so the default text path has a health
  // signal even when Ollama is absent.
  const customBaseUrl = serverEnv("OPENAI_COMPAT_BASE_URL", "http://127.0.0.1:8080/v1");
  let customText = { ok: false, models: [] as string[] };
  try {
    const base = customBaseUrl.replace(/\/$/, "");
    const modelsUrl = /\/v\d+$/.test(base) ? `${base}/models` : `${base}/v1/models`;
    const response = await fetch(modelsUrl, { cache: "no-store" });
    if (response.ok) {
      const data = (await response.json()) as { data?: Array<{ id?: string }> };
      customText = {
        ok: true,
        models: (data.data || []).map((m) => m.id || "").filter(Boolean),
      };
    }
  } catch {
    customText = { ok: false, models: [] };
  }

  return Response.json({
    openRouterConfigured: Boolean(serverEnv("OPENROUTER_API_KEY")),
    model: serverEnv("OPENROUTER_MODEL", "google/gemini-3.5-flash"),
    maxTokens: Number.parseInt(serverEnv("OPENROUTER_MAX_TOKENS", "16384"), 10),
    customText,
    localText,
    flux,
  });
}
