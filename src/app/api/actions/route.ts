import { z } from "zod";
import { requestChatCompletion } from "@/lib/llm";

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

// Parse "emoji | action text" lines (lenient: also accepts plain lines and
// numbered/bulleted lists). Returns up to four {emoji,label} actions.
// `\b` is ASCII-only, so it never fires after a Cyrillic letter; use a Unicode
// letter lookahead (with /u) so the hedging-word prefixes are actually dropped.
const META_NOISE =
  /[*#]{2,}|представляет собой|вариант\s*\d|трансформир|метафор|абсурдизм|^\s*(если|чтобы|судя)(?!\p{L})/iu;

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

  // Best-effort: any failure (server down, non-ok, timeout) degrades to no chips.
  const result = await requestChatCompletion({
    settings: body.settings,
    messages,
    temperature: 0.5,
    maxTokens: 200,
    timeoutMs: 45_000,
  });
  return Response.json({ actions: result.ok ? parseActions(result.content) : [] });
}
