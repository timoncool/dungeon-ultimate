import { serverEnv } from "@/lib/server-env";

export const runtime = "nodejs";

// List the TTS voice pack from the local reader (od-tts-server.py on :8081).
// Returns { default, voices: [...] }; falls back to an empty list when the
// reader is not running so the UI degrades gracefully.
export async function GET() {
  const base = serverEnv("TTS_WORKER_URL", "http://127.0.0.1:8081").replace(/\/$/, "");
  try {
    const upstream = await fetch(`${base}/voices`, { cache: "no-store" });
    if (!upstream.ok) {
      return Response.json({ default: "", voices: [] as string[] });
    }
    return Response.json(await upstream.json());
  } catch {
    return Response.json({ default: "", voices: [] as string[] });
  }
}
