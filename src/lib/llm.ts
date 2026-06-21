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
  // OpenAI/llama.cpp `response_format` — e.g. {type:"json_object", schema:{…}}.
  // The local server turns the schema into a GBNF grammar that constrains
  // sampling, so the reply is always valid JSON of that shape.
  responseFormat?: unknown;
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
        ...(opts.responseFormat ? { response_format: opts.responseFormat } : {}),
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

export type StructuredJsonResult<T> =
  | { ok: true; data: T }
  | { ok: false; status: number; detail: string };

// Grammar-constrained JSON completion: the JSON Schema is sent as response_format
// so the local server constrains sampling to it (llama.cpp from_json_schema) — the
// reply is therefore always valid JSON of the right shape, no fragile text parsing.
// NOTE: the schema is NOT shown to the model (it only shapes the grammar), so the
// caller's prompt must still describe the fields in words. Format ≠ semantics: the
// grammar guarantees the shape; the prompt + downstream validation guard meaning.
export async function requestStructuredJson<T = unknown>(opts: {
  settings: CustomTextSettings;
  messages: Array<{ role: "system" | "user" | "assistant"; content: string }>;
  schema: Record<string, unknown>;
  temperature?: number;
  maxTokens: number;
  timeoutMs: number;
}): Promise<StructuredJsonResult<T>> {
  const result = await requestChatCompletion({
    settings: opts.settings,
    messages: opts.messages,
    temperature: opts.temperature ?? 0.4,
    maxTokens: opts.maxTokens,
    timeoutMs: opts.timeoutMs,
    responseFormat: { type: "json_object", schema: opts.schema },
  });
  if (!result.ok) {
    return result;
  }
  const parse = (text: string): T | undefined => {
    try {
      return JSON.parse(text) as T;
    } catch {
      return undefined;
    }
  };
  // Grammar guarantees valid JSON; the regex salvage is only a guard for a server
  // that silently ignored response_format (so we degrade instead of throwing).
  const data = parse(result.content) ?? parse(result.content.match(/\{[\s\S]*\}/)?.[0] ?? "");
  if (data === undefined) {
    return { ok: false, status: 0, detail: "structured output was not valid JSON" };
  }
  return { ok: true, data };
}
