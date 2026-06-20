import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

export const runtime = "nodejs";

// Max size for an uploaded clone reference clip.
const MAX_FILE_SIZE = 12 * 1024 * 1024;

function safeName(value: string) {
  return value.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 80);
}

// Upload a custom voice-clone reference: a multipart .mp3 saved to
// public/uploads/voices/<name>.mp3. The od-tts-server.py reader resolves
// voices by id (filename without extension) from this same directory, so the
// returned `voice` id can be sent straight to POST /api/tts as { voice }.
export async function POST(request: Request) {
  let form: FormData;
  try {
    form = await request.formData();
  } catch {
    return Response.json({ error: "Ожидается multipart-загрузка." }, { status: 400 });
  }

  const file = form.get("file");
  if (!(file instanceof File) || file.size === 0) {
    return Response.json({ error: "Файл не передан." }, { status: 400 });
  }
  if (file.size > MAX_FILE_SIZE) {
    return Response.json({ error: "Файл больше 12МБ." }, { status: 413 });
  }

  const lowerName = file.name.toLowerCase();
  const isMp3 = file.type === "audio/mpeg" || lowerName.endsWith(".mp3");
  if (!isMp3) {
    return Response.json({ error: "Поддерживается только формат .mp3." }, { status: 415 });
  }

  const buffer = Buffer.from(await file.arrayBuffer());
  if (!buffer.length) {
    return Response.json({ error: "Пустой файл." }, { status: 400 });
  }

  // Build a stable, collision-resistant voice id from the original name.
  const stem = safeName(lowerName.replace(/\.mp3$/, "")) || "voice";
  const voiceId = `${stem}_${crypto.randomUUID().slice(0, 8)}`;
  const filename = `${voiceId}.mp3`;
  const uploadDir = path.join(process.cwd(), "public", "uploads", "voices");
  await mkdir(uploadDir, { recursive: true });
  await writeFile(path.join(uploadDir, filename), buffer);

  return Response.json({
    voice: voiceId,
    name: file.name,
    url: `/uploads/voices/${filename}`,
  });
}
