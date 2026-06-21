import { serverEnv } from "@/lib/server-env";

// Resolve a user-entered backend URL to its /chat/completions endpoint.
// Accepts a bare host (http://127.0.0.1:8080), a versioned base (.../v1), or
// the full endpoint, so people can paste whatever their server prints.
export function customChatEndpoint(baseUrl: string): string {
  const url = baseUrl.trim().replace(/\/+$/, "");
  if (/\/chat\/completions$/.test(url)) return url;
  if (/\/v\d+$/.test(url)) return `${url}/chat/completions`;
  return `${url}/v1/chat/completions`;
}

type CustomTextSettings = {
  customBaseUrl?: string;
  customModel?: string;
  customApiKey?: string;
};

// ok:false carries the HTTP status, or 0 when the request never completed
// (network/timeout/abort) — callers use that to pick the right message.
export type ChatCompletionResult =
  | { ok: true; content: string }
  | { ok: false; status: number; detail: string };

// One buffered (non-streaming, no-tool) OpenAI-compatible chat completion against
// the per-chat custom server, with env fallbacks + an abort timeout. Shared by the
// suggest + actions routes; the story route keeps its own streaming/tool path.
export async function requestChatCompletion(opts: {
  settings: CustomTextSettings;
  messages: Array<{ role: "system" | "user" | "assistant"; content: string }>;
  temperature: number;
  maxTokens: number;
  timeoutMs: number;
}): Promise<ChatCompletionResult> {
  const baseUrl =
    opts.settings.customBaseUrl?.trim() ||
    serverEnv("OPENAI_COMPAT_BASE_URL", "http://127.0.0.1:8080/v1");
  const model =
    opts.settings.customModel?.trim() || serverEnv("OPENAI_COMPAT_MODEL", "gemma-4-12b-uncensored");
  const apiKey = opts.settings.customApiKey?.trim() || serverEnv("OPENAI_COMPAT_API_KEY");

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), opts.timeoutMs);
  try {
    const upstream = await fetch(customChatEndpoint(baseUrl), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(apiKey ? { Authorization: `Bearer ${apiKey}` } : {}),
      },
      body: JSON.stringify({
        model,
        messages: opts.messages,
        temperature: opts.temperature,
        max_tokens: opts.maxTokens,
      }),
      signal: controller.signal,
    });
    if (!upstream.ok) {
      const detail = await upstream.text();
      return { ok: false, status: upstream.status, detail: detail.slice(0, 500) };
    }
    const data = (await upstream.json()) as {
      choices?: Array<{ message?: { content?: unknown } }>;
    };
    const raw = data?.choices?.[0]?.message?.content;
    return { ok: true, content: typeof raw === "string" ? raw : "" };
  } catch (error) {
    return { ok: false, status: 0, detail: error instanceof Error ? error.message : String(error) };
  } finally {
    clearTimeout(timeout);
  }
}
