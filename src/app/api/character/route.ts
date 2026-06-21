import { z } from "zod";
import { requestChatCompletion } from "@/lib/llm";
import { LANGUAGE_PROMPT_NAMES, LANGUAGE_VALUES } from "@/lib/types";

export const runtime = "nodejs";

// Character autofill. One buffered call to the same local server asks for a
// vivid, original RPG character and returns ONLY compact JSON, so the player can
// populate a whole sheet (name/details/inventory/skills/spells) in one click.

const requestSchema = z.object({
  settings: z
    .object({
      customBaseUrl: z.string().trim().max(500).default(""),
      customModel: z.string().trim().max(200).default(""),
      customApiKey: z.string().trim().max(400).default(""),
      language: z.enum(LANGUAGE_VALUES).default("ru"),
    })
    .default({ customBaseUrl: "", customModel: "", customApiKey: "", language: "ru" }),
  hint: z.string().trim().max(2000).optional(),
});

// Lenient JSON extraction: strip code fences, then take the first {...} block.
function extractJson(raw: string): Record<string, unknown> | null {
  const noFence = raw.replace(/```(?:json)?/gi, "").trim();
  const start = noFence.indexOf("{");
  const end = noFence.lastIndexOf("}");
  if (start === -1 || end === -1 || end <= start) return null;
  try {
    const parsed = JSON.parse(noFence.slice(start, end + 1));
    return parsed && typeof parsed === "object" ? (parsed as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

const str = (value: unknown): string => (typeof value === "string" ? value.trim() : "");

export async function POST(request: Request) {
  const parsedBody = requestSchema.safeParse(await request.json().catch(() => null));
  if (!parsedBody.success) {
    return Response.json({ error: "Некорректный запрос." }, { status: 400 });
  }
  const body = parsedBody.data;
  const langName = LANGUAGE_PROMPT_NAMES[body.settings.language];

  const messages = [
    {
      role: "system" as const,
      content: `You invent vivid, original tabletop-RPG characters. Reply with ONLY one compact JSON object, no prose, no code fences. Write every field's text in ${langName}.`,
    },
    {
      role: "user" as const,
      content:
        `Invent a vivid, original RPG character${body.hint ? ` based on this hint: ${body.hint}` : ""}.\n` +
        `Return ONLY this JSON, all values as strings in ${langName}:\n` +
        `{"name","details","inventory","skills","spells"}\n` +
        `- name: the character's name only.\n` +
        `- details: appearance + personality, 1-2 sentences.\n` +
        `- inventory: a short comma-separated list of items.\n` +
        `- skills: a short comma-separated list of skills.\n` +
        `- spells: a short comma-separated list of spells (empty string if non-magical).`,
    },
  ];

  const result = await requestChatCompletion({
    settings: body.settings,
    messages,
    temperature: 1.0,
    // Headroom for a full sheet in token-heavy scripts (zh/ja/ru) so the JSON
    // isn't truncated before its closing brace.
    maxTokens: 800,
    timeoutMs: 60_000,
  });

  if (!result.ok) {
    return Response.json(
      result.status
        ? { error: `Сервер не ответил (${result.status}).`, detail: result.detail }
        : { error: "Не удалось создать персонажа. Запущен ли текстовый сервер?", detail: result.detail },
      { status: 502 },
    );
  }

  const parsed = extractJson(result.content);
  if (!parsed) {
    return Response.json({ error: "Модель вернула некорректный JSON." }, { status: 502 });
  }

  return Response.json({
    character: {
      name: str(parsed.name),
      details: str(parsed.details),
      inventory: str(parsed.inventory),
      skills: str(parsed.skills),
      spells: str(parsed.spells),
    },
  });
}
