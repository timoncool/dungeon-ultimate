import { configuredDefaultStorySettings } from "@/lib/runtime-defaults";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET() {
  return Response.json({ settings: configuredDefaultStorySettings() });
}
