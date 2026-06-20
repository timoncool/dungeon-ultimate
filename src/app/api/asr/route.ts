import { serverEnv } from "@/lib/server-env";

export const runtime = "nodejs";

// Proxies a recorded WAV blob to the local Parakeet ASR server (od-asr-server.py
// on :8082) and returns the transcription for voice input. The browser records
// 16 kHz mono WAV so the server can hand it straight to onnx-asr.
export async function POST(request: Request) {
  const asrUrl = serverEnv("ASR_WORKER_URL", "http://127.0.0.1:8082").replace(/\/$/, "");
  const audio = await request.arrayBuffer();

  if (!audio.byteLength) {
    return Response.json({ error: "Пустая запись." }, { status: 400 });
  }

  try {
    const upstream = await fetch(`${asrUrl}/asr`, {
      method: "POST",
      headers: { "Content-Type": "audio/wav" },
      body: audio,
    });

    if (!upstream.ok) {
      const detail = await upstream.text();
      return Response.json(
        { error: `ASR-сервер не ответил (${upstream.status}).`, detail: detail.slice(0, 400) },
        { status: 502 },
      );
    }

    const data = (await upstream.json()) as { text?: string };
    return Response.json({ text: data.text ?? "" });
  } catch (error) {
    return Response.json(
      {
        error: "ASR-сервер недоступен. Запущен ли od-asr-server.py на порту 8082?",
        detail: error instanceof Error ? error.message : String(error),
      },
      { status: 503 },
    );
  }
}
