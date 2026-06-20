import { z } from "zod";
import { serverEnv } from "@/lib/server-env";

export const runtime = "nodejs";

// Suggested-action chips for one-button play. After each narration turn the
// client asks the same local Gemma server for 3–4 short, distinct things the
// player could do next (AI-Dungeon / D&D "quick actions"). Decoupled from the
// story tool-calling so it never affects the streamed prose.

const requestSchema = z.object({
  // Only the last 4000 chars are used (see below), so a generous cap just keeps a
  // very long passage from 500ing this best-effort route instead of degrading.
  passage: z.string().min(1).max(40000),
  settings: z
    .object({
      customBaseUrl: z.string().trim().max(500).default(""),
      customModel: z.string().trim().max(200).default(""),
      customApiKey: z.string().trim().max(400).default(""),
    })
    .default({ customBaseUrl: "", customModel: "", customApiKey: "" }),
});

function customChatEndpoint(baseUrl: string): string {
  const url = baseUrl.trim().replace(/\/+$/, "");
  if (/\/chat\/completions$/.test(url)) return url;
  if (/\/v\d+$/.test(url)) return `${url}/chat/completions`;
  return `${url}/v1/chat/completions`;
}

// Parse "emoji | action text" lines (lenient: also accepts plain lines and
// numbered/bulleted lists). Returns up to four {emoji,label} actions.
const META_NOISE =
  /[*#]{2,}|представляет собой|вариант\s*\d|трансформир|метафор|абсурдизм|^\s*(если|чтобы|судя)\b/i;

function parseActions(raw: string): Array<{ emoji?: string; label: string }> {
  const out: Array<{ emoji?: string; label: string }> = [];
  for (const line of raw.split(/\r?\n/)) {
    if (META_NOISE.test(line)) continue; // drop analysis/markdown, never render it
    let cleaned = line.replace(/^[\s\-*•>#\d.)]+/, "").trim();
    if (!cleaned) continue;
    let emoji: string | undefined;
    if (cleaned.includes("|")) {
      const [head, ...rest] = cleaned.split("|");
      emoji = head.trim() || undefined;
      cleaned = rest.join("|").trim();
    } else {
      const lead = cleaned.match(/^(\p{Extended_Pictographic})\s*/u);
      if (lead) {
        emoji = lead[1];
        cleaned = cleaned.slice(lead[0].length).trim();
      }
    }
    // A quick action is one short clause — reject sentences/paragraphs outright.
    if (!cleaned || cleaned.length > 48 || cleaned.split(/\s+/).length > 8) continue;
    if (!/[а-яёa-z]/i.test(cleaned)) continue;
    if (emoji && emoji.length > 4) emoji = undefined;
    out.push({ emoji, label: cleaned });
    if (out.length >= 4) break;
  }
  return out;
}

export async function POST(request: Request) {
  // Best-effort route: a malformed/oversized body must degrade to empty chips,
  // never a 500 that the comment + catch below promise it won't be.
  const parsed = requestSchema.safeParse(await request.json().catch(() => null));
  if (!parsed.success) {
    return Response.json({ actions: [] });
  }
  const body = parsed.data;

  const baseUrl =
    body.settings.customBaseUrl.trim() ||
    serverEnv("OPENAI_COMPAT_BASE_URL", "http://127.0.0.1:8080/v1");
  const model =
    body.settings.customModel.trim() ||
    serverEnv("OPENAI_COMPAT_MODEL", "gemma-4-12b-uncensored");
  const apiKey = body.settings.customApiKey.trim() || serverEnv("OPENAI_COMPAT_API_KEY");

  const messages = [
    {
      role: "system" as const,
      content:
        "Ты — генератор быстрых действий для текстовой ролевой игры (D&D). НЕ анализируй, НЕ комментируй и НЕ пересказывай текст. Прочитай последнюю сцену и предложи РОВНО 3–4 коротких, конкретных и РАЗНЫХ действия, которые герой-игрок может совершить прямо сейчас (повелительно, 3–6 слов). Каждое — на отдельной строке СТРОГО в формате: эмодзи | действие. Никаких заголовков, нумерации, пояснений, разбора — ТОЛЬКО 3–4 такие строки.\n\nПример:\n⚔️ | Атаковать ближайшую тварь\n🛡️ | Закрыться и отступить к стене\n👁️ | Осмотреть тёмный проход\n🗣️ | Крикнуть, чтобы спугнуть их",
    },
    {
      role: "user" as const,
      content: `Последняя сцена:\n${body.passage.slice(-4000)}\n\nПредложи 3–4 варианта действия.`,
    },
  ];

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 45_000);
  try {
    const upstream = await fetch(customChatEndpoint(baseUrl), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(apiKey ? { Authorization: `Bearer ${apiKey}` } : {}),
      },
      body: JSON.stringify({ model, messages, temperature: 0.5, max_tokens: 200 }),
      signal: controller.signal,
    });

    if (!upstream.ok) {
      return Response.json({ actions: [] });
    }

    const data = (await upstream.json()) as {
      choices?: Array<{ message?: { content?: unknown } }>;
    };
    const raw = typeof data?.choices?.[0]?.message?.content === "string"
      ? (data.choices[0].message!.content as string)
      : "";

    return Response.json({ actions: parseActions(raw) });
  } catch {
    // Best-effort: no chips on failure, never blocks play.
    return Response.json({ actions: [] });
  } finally {
    clearTimeout(timeout);
  }
}
