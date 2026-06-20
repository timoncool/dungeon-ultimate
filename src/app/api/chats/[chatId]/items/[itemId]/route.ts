import { z } from "zod";
import { setItemEquipped } from "@/lib/db";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

type ItemRouteContext = {
  params: Promise<{ chatId: string; itemId: string }>;
};

const patchItemSchema = z.object({ equipped: z.boolean() });

export async function PATCH(request: Request, context: ItemRouteContext) {
  const { chatId, itemId } = await context.params;
  const body = patchItemSchema.parse(await request.json());
  const item = setItemEquipped(chatId, itemId, body.equipped);
  if (!item) {
    return Response.json({ error: "Item not found." }, { status: 404 });
  }
  return Response.json({ item });
}
