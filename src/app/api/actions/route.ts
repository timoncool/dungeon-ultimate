import { z } from "zod";
import { requestChatCompletion, requestStructuredJson } from "@/lib/llm";
import { LANGUAGE_VALUES } from "@/lib/types";
import { promptsFor } from "@/lib/prompts";

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
      language: z.enum(LANGUAGE_VALUES).default("ru"),
    })
    .default({ customBaseUrl: "", customModel: "", customApiKey: "", language: "ru" }),
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
    // Must contain a real letter in ANY script (Latin/Cyrillic/CJK/kana/…) so the
    // chips work in every offered language, not just ru/en — but a pure
    // emoji/punctuation line is still dropped.
    if (!/\p{L}/u.test(cleaned)) continue;
    if (emoji && emoji.length > 4) emoji = undefined;
    out.push({ emoji, label: cleaned });
    if (out.length >= 4) break;
  }
  return out;
}

// Grammar schema for the structured path: a small, strictly-shaped object so the
// local server constrains sampling to exactly {actions:[{emoji,label}]}.
const ACTIONS_SCHEMA = {
  type: "object",
  additionalProperties: false,
  properties: {
    actions: {
      type: "array",
      minItems: 3,
      maxItems: 4,
      items: {
        type: "object",
        additionalProperties: false,
        properties: {
          emoji: { type: "string", maxLength: 8 },
          label: { type: "string", minLength: 1, maxLength: 48 },
        },
        required: ["emoji", "label"],
      },
    },
  },
  required: ["actions"],
};

// Normalize one structured action: split a stray "emoji | text", lift a leading
// emoji out of the label, and drop an over-long "emoji" that's really a word.
function cleanAction(action: { emoji?: string; label?: string }): { emoji?: string; label: string } {
  let label = (action.label ?? "").trim();
  let emoji = (action.emoji ?? "").trim() || undefined;
  if (label.includes("|")) {
    const [head, ...rest] = label.split("|");
    label = rest.join("|").trim();
    if (!emoji && head.trim()) emoji = head.trim();
  }
  if (!emoji) {
    const lead = label.match(/^(\p{Extended_Pictographic})\s*/u);
    if (lead) {
      emoji = lead[1];
      label = label.slice(lead[0].length).trim();
    }
  }
  if (emoji && emoji.length > 4) emoji = undefined;
  return { emoji, label };
}

export async function POST(request: Request) {
  // Best-effort route: a malformed/oversized body must degrade to empty chips,
  // never a 500 that the comment + catch below promise it won't be.
  const parsed = requestSchema.safeParse(await request.json().catch(() => null));
  if (!parsed.success) {
    return Response.json({ actions: [] });
  }
  const body = parsed.data;

  // The in-language actions.system carries the intent (3–4 short, distinct,
  // imperative actions); the user turn spells out the JSON shape, since the schema
  // constrains the grammar but is never shown to the model.
  const messages = [
    {
      role: "system" as const,
      content: promptsFor(body.settings.language).actions.system,
    },
    {
      role: "user" as const,
      content: `Последняя сцена:\n${body.passage.slice(-4000)}\n\nВерни JSON-объект {"actions":[{"emoji":"<один эмодзи>","label":"<короткое действие, 3–6 слов, повелительно>"}, …]} — РОВНО 3–4 разных конкретных действия, которые герой-игрок может совершить прямо сейчас.`,
    },
  ];

  // Structured (grammar-constrained) chips: the schema forces a valid JSON shape,
  // so the fragile "emoji | action" text parsing never has to fire on the happy path.
  const structured = await requestStructuredJson<{ actions?: Array<{ emoji?: string; label?: string }> }>({
    settings: body.settings,
    messages,
    schema: ACTIONS_SCHEMA,
    temperature: 0.5,
    maxTokens: 220,
    timeoutMs: 45_000,
  });
  if (structured.ok && Array.isArray(structured.data.actions)) {
    const actions = structured.data.actions
      .map(cleanAction)
      .filter((action) => action.label && /\p{L}/u.test(action.label))
      .slice(0, 4);
    if (actions.length) {
      return Response.json({ actions });
    }
  }

  // Fallback: a server that ignored response_format still answers as text — parse
  // the lenient "emoji | action" form so chips degrade rather than vanish.
  const text = await requestChatCompletion({
    settings: body.settings,
    messages,
    temperature: 0.5,
    maxTokens: 200,
    timeoutMs: 45_000,
  });
  return Response.json({ actions: text.ok ? parseActions(text.content) : [] });
}
