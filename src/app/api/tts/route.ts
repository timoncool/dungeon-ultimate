import { createHash } from "node:crypto";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { getCharactersByIds } from "@/lib/db";
import { serverEnv } from "@/lib/server-env";
import { safeName } from "@/lib/tts";
import { LANGUAGE_TTS_CODES, isLanguage } from "@/lib/types";

export const runtime = "nodejs";

// Voice a narrator passage via the local TTS reader (od-tts-server.py :8081).
// When a messageId is given, the WAV is saved per-message under
// public/generated/tts and replayed from there — so audio is kept with the
// dialog and never regenerated. Without a messageId the audio is streamed back
// directly (ad-hoc preview).
export async function POST(request: Request) {
  const body = (await request.json().catch(() => ({}))) as {
    text?: unknown;
    voice?: unknown;
    messageId?: unknown;
    chunkIndex?: unknown;
    // Optional multi-voice hook: when given, the speaking character's saved
    // voice (if any) overrides `voice`. Lets the caller ask for a character's
    // line without knowing the voice id. Bare { text, voice } is unaffected.
    chatId?: unknown;
    characterId?: unknown;
    // The story's language (client sends settings.language); maps to the TTS
    // worker's language code. Falls back to "ru" when absent or unrecognized.
    language?: unknown;
  };
  const text = typeof body.text === "string" ? body.text : "";
  if (!text.trim()) {
    return Response.json({ error: "Пустой текст для озвучки." }, { status: 400 });
  }
  const requestedVoice =
    typeof body.voice === "string" && body.voice ? body.voice : "RU_Male_Gabidullin_ruslan";
  // Resolve an explicit speaker to their per-character voice, falling back to the
  // requested/default voice when the character is unknown or has none set.
  const chatId = typeof body.chatId === "string" ? body.chatId : "";
  const characterId = typeof body.characterId === "string" ? body.characterId : "";
  let voice = requestedVoice;
  if (chatId && characterId) {
    const character = getCharactersByIds(chatId, [characterId])[0];
    if (character?.voice?.trim()) {
      voice = character.voice.trim();
    }
  }
  const language = isLanguage(body.language) ? LANGUAGE_TTS_CODES[body.language] : "ru";
  const messageId = typeof body.messageId === "string" ? body.messageId : "";
  const chunkIndex = typeof body.chunkIndex === "number" ? body.chunkIndex : null;
  const base = serverEnv("TTS_WORKER_URL", "http://127.0.0.1:8081").replace(/\/$/, "");

  // Call the worker and return the upstream response, or a 502 error Response
  // (upstream non-ok, or worker unreachable). Shared by both POST branches.
  async function synthesize(): Promise<Response> {
    let upstream: Response;
    try {
      upstream = await fetch(`${base}/tts`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ text, voice, language }),
      });
    } catch {
      return Response.json(
        { error: "Сервер озвучки не запущен (порт 8081)." },
        { status: 502 },
      );
    }
    if (!upstream.ok) {
      const detail = await upstream.text();
      return Response.json(
        { error: `Сбой озвучки (${upstream.status}).`, detail: detail.slice(0, 300) },
        { status: 502 },
      );
    }
    return upstream;
  }

  if (messageId) {
    // Fold a short content hash in so editing a passage regenerates audio
    // instead of replaying the stale cached WAV for the same id/chunk/voice.
    const hash = createHash("sha1").update(text).digest("hex").slice(0, 8);
    const fileName =
      chunkIndex === null
        ? `${safeName(messageId)}__${safeName(voice)}__${hash}.wav`
        : `${safeName(messageId)}__c${chunkIndex}__${safeName(voice)}__${hash}.wav`;
    const dir = path.join(process.cwd(), "public", "generated", "tts");
    const filePath = path.join(dir, fileName);
    const url = `/generated/tts/${fileName}`;
    if (existsSync(filePath)) {
      return Response.json({ url, cached: true });
    }
    const upstream = await synthesize();
    if (!upstream.ok) return upstream;
    const audio = Buffer.from(await upstream.arrayBuffer());
    mkdirSync(dir, { recursive: true });
    writeFileSync(filePath, audio);
    return Response.json({ url, cached: false });
  }

  const upstream = await synthesize();
  if (!upstream.ok) return upstream;
  return new Response(await upstream.arrayBuffer(), {
    headers: { "Content-Type": "audio/wav", "Cache-Control": "no-store" },
  });
}
