import { z } from "zod";
import { serverEnv } from "@/lib/server-env";

export const runtime = "nodejs";

// Suggested-action chips for one-button play. After each narration turn the
// client asks the same local Gemma server for 3–4 short, distinct things the
// player could do next (AI-Dungeon / D&D "quick actions"). Decoupled from the
// story tool-calling so it never affects the streamed prose.

const requestSchema = z.object({
  passage: z.string().min(1).max(8000),
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
function parseActions(raw: string): Array<{ emoji?: string; label: string }> {
  const out: Array<{ emoji?: string; label: string }> = [];
  for (const line of raw.split(/\r?\n/)) {
    const cleaned = line.replace(/^[\s\-*•\d.)]+/, "").trim();
    if (!cleaned) continue;
    const [maybeEmoji, ...rest] = cleaned.split("|");
    if (rest.length) {
      out.push({ emoji: maybeEmoji.trim() || undefined, label: rest.join("|").trim() });
    } else {
      out.push({ label: cleaned });
    }
    if (out.length >= 4) break;
  }
  return out.filter((action) => action.label.length > 0);
}

export async function POST(request: Request) {
  const body = requestSchema.parse(await request.json());

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
        "Ты предлагаешь игроку варианты действий в текстовой ролевой игре (стиль D&D). По-русски. Дай РОВНО 3–4 коротких, разных, конкретных варианта действия от первого лица (3–7 слов каждый). Каждый на отдельной строке в формате «эмодзи | текст действия». Без нумерации, без пояснений, только строки.",
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
      body: JSON.stringify({ model, messages, temperature: 0.9, max_tokens: 220 }),
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
