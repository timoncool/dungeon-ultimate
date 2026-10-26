import { useCallback, useEffect } from "react";
import { X } from "lucide-react";
import { HUB_GRADIENTS, HUB_TEXT, hubMediaUrl, hubText, reportNotice, reportShown, stackTheme, type HubButton, type HubItem, type HubTarget } from "@/lib/studioHub";
import { useUiLanguage } from "@/lib/ui-text-context";
import { panelText } from "@/lib/ui-text-panels";
import { inline } from "./HubBars";

/** Окно хаба в стиле диалогов игры: шапка в градиенте темы, картинка, текст, кнопки. Одно за раз. */
export function HubPopup({ item, onClose, onOpen }: { item: HubItem; onClose: () => void; onOpen: (target: HubTarget) => void }) {
  const lang = useUiLanguage();
  const panel = panelText(lang);
  const content = hubText(item, lang);
  const theme = stackTheme(item.theme, 0);

  useEffect(() => {
    reportShown(item.id);
  }, [item.id]);

  const dismiss = useCallback(() => {
    reportNotice(item.id, "dismissed").catch((e: Error) => console.warn("[hub] popup close not recorded:", e.message));
    onClose();
  }, [item.id, onClose]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && item.dismissible) dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [item.dismissible, dismiss]);

  if (!content) return null;

  const press = (button: HubButton, index: number) => {
    const event = button.action === "dismiss" ? "dismissed" : "clicked";
    reportNotice(item.id, event, button.action === "dismiss" ? undefined : `b${index}`).catch((e: Error) =>
      console.warn("[hub] popup button not recorded:", e.message),
    );
    if (button.action === "url" && button.url) window.open(button.url, "_blank", "noopener,noreferrer");
    if (button.action === "open" && button.target) onOpen(button.target);
    onClose();
  };

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-stone-950/70 p-4"
      onClick={(event) => {
        if (event.target === event.currentTarget && item.dismissible) dismiss();
      }}
    >
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby={`hub-${item.id}`}
        className="w-full max-w-md overflow-hidden rounded-2xl border border-stone-700 bg-[#130d09] shadow-2xl"
      >
        <div className="flex items-center justify-between gap-3 px-4 py-3" style={{ background: HUB_GRADIENTS[theme], color: HUB_TEXT[theme] }}>
          <h2 id={`hub-${item.id}`} className="text-base font-semibold leading-snug">{content.title}</h2>
          {item.dismissible && (
            <button type="button" onClick={dismiss} aria-label={panel.hubClose} className="rounded p-1 opacity-85 hover:opacity-100">
              <X className="size-4" aria-hidden="true" />
            </button>
          )}
        </div>
        {item.image && <img src={hubMediaUrl(item.image)} alt="" className="max-h-56 w-full object-cover" />}
        <div
          className="space-y-2 px-4 py-3 text-sm leading-relaxed text-stone-300 [&_a]:text-amber-200 [&_a]:underline"
          onClickCapture={(event) => {
            if ((event.target as HTMLElement).closest("a")) {
              reportNotice(item.id, "clicked", "link").catch((e: Error) => console.warn("[hub] popup link not recorded:", e.message));
            }
          }}
        >
          {content.body.split(/\n\s*\n/).map((block, index) => (
            <p key={index}>{inline(block, `${item.id}-${index}`)}</p>
          ))}
        </div>
        {item.ad?.label && (
          <p className="px-4 pb-2 text-xs text-stone-500">
            {panel.hubAd}
            {item.ad.erid ? ` · erid ${item.ad.erid}` : ""}
          </p>
        )}
        {content.buttons.length > 0 && (
          <div className="flex justify-end gap-2 border-t border-stone-800 px-4 py-3">
            {content.buttons.map((button, index) => (
              <button
                key={index}
                type="button"
                onClick={() => press(button, index)}
                className={
                  button.style === "primary"
                    ? "rounded px-3 py-1.5 text-sm font-semibold hover:opacity-90"
                    : "rounded border border-stone-700 px-3 py-1.5 text-sm text-stone-300 hover:bg-stone-900"
                }
                style={button.style === "primary" ? { background: HUB_GRADIENTS[theme], color: HUB_TEXT[theme] } : undefined}
              >
                {button.label}
              </button>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
