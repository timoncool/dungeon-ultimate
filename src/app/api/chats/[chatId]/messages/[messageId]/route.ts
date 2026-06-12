import { z } from "zod";
import { deleteMessageAndAfter, getChat, updateMessageContent } from "@/lib/db";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

type MessageRouteContext = {
  params: Promise<{ chatId: string; messageId: string }>;
};

const patchSchema = z.object({
  content: z.string().min(1),
});

export async function PATCH(request: Request, context: MessageRouteContext) {
  const { chatId, messageId } = await context.params;
  const body = patchSchema.parse(await request.json());

  if (!updateMessageContent(messageId, body.content)) {
    return Response.json({ error: "Message not found." }, { status: 404 });
  }

  const chat = getChat(chatId);
  return Response.json({ chat });
}

export async function DELETE(request: Request, context: MessageRouteContext) {
  const { chatId, messageId } = await context.params;
  const url = new URL(request.url);
  // ?after=1 also discards everything that follows this message.
  const includeAfter = url.searchParams.get("after") === "1";

  if (!deleteMessageAndAfter(messageId, includeAfter)) {
    return Response.json({ error: "Message not found." }, { status: 404 });
  }

  const chat = getChat(chatId);
  return Response.json({ chat });
}
