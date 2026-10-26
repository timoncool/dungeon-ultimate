import { useEffect, useState } from "react";
import { ChevronRight, Newspaper } from "lucide-react";
import { hubMediaUrl, hubText, reportNotice, reportShown, useHubState, type HubButton, type HubItem, type HubTarget } from "@/lib/studioHub";
import { useUiLanguage } from "@/lib/ui-text-context";
import { panelText } from "@/lib/ui-text-panels";
import { inline } from "./HubBars";

// Новости хаба — как вкладка новостей Dub Studio: новая доходит до игры без релиза.

function NewsBody({ text, id }: { text: string; id: string }) {
  return (
    <div
      className="mt-1.5 space-y-2 text-xs leading-relaxed text-stone-400"
      onClickCapture={(event) => {
        if ((event.target as HTMLElement).closest("a")) {
          reportNotice(id, "clicked", "link").catch((e: Error) => console.warn("[hub] news link not recorded:", e.message));
        }
      }}
    >
      {text.split(/\n\s*\n/).map((block, i) => {
        const lines = block.split("\n").filter((l) => l.trim());
        if (lines.length && lines.every((l) => l.trimStart().startsWith("- "))) {
          return (
            <ul key={i} className="list-disc space-y-1 pl-4">
              {lines.map((l, j) => <li key={j}>{inline(l.trimStart().slice(2), `${id}-${i}-${j}`)}</li>)}
            </ul>
          );
        }
        return <p key={i}>{inline(block, `${id}-${i}`)}</p>;
      })}
    </div>
  );
}

export default function HubNews({ onOpen }: { onOpen: (target: HubTarget) => void }) {
  const lang = useUiLanguage();
  const panel = panelText(lang);
  const hub = useHubState(lang);
  const [open, setOpen] = useState(false);
  const news: HubItem[] = (hub?.items ?? [])
    .filter((item) => item.kind === "news" && item.eligible)
    .sort((a, b) => (b.date ?? "").localeCompare(a.date ?? ""));
  const shownIds = open ? news.map((n) => n.id).join(",") : "";
  useEffect(() => {
    for (const id of shownIds ? shownIds.split(",") : []) reportShown(id);
  }, [shownIds]);

  const press = (item: HubItem, button: HubButton, index: number) => {
    const event = button.action === "dismiss" ? "dismissed" : "clicked";
    reportNotice(item.id, event, button.action === "dismiss" ? undefined : `b${index}`).catch((e: Error) =>
      console.warn("[hub] news button not recorded:", e.message),
    );
    if (button.action === "url" && button.url) window.open(button.url, "_blank", "noopener,noreferrer");
    if (button.action === "open" && button.target) onOpen(button.target);
  };

  return (
    <section className="space-y-2.5">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="group flex w-full min-w-0 items-center gap-3 rounded border border-stone-800 bg-stone-950/70 p-2 text-left transition hover:border-amber-800/60 hover:bg-stone-900/70"
      >
        <span className="flex size-12 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-amber-200/15 bg-stone-950 text-amber-200 shadow-[0_0_16px_rgba(251,191,36,0.1)]">
          <Newspaper className="size-6" aria-hidden="true" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm font-medium text-stone-300">{panel.news}</span>
          <span className="block truncate text-xs text-stone-500">{news.length ? hubText(news[0], lang)?.title : panel.newsEmpty}</span>
        </span>
        <ChevronRight
          className={`ml-auto size-4 shrink-0 text-stone-500 transition group-hover:text-amber-200/80 ${open ? "rotate-90 text-amber-200" : ""}`}
          aria-hidden="true"
        />
      </button>
      {open && (
        <div className="space-y-2">
          {news.length === 0 && <p className="text-xs text-stone-500">{panel.newsEmpty}</p>}
          {news.map((item) => {
            const content = hubText(item, lang);
            if (!content) return null;
            return (
              <article key={item.id} className="overflow-hidden rounded border border-stone-800 bg-stone-950">
                {item.image && <img src={hubMediaUrl(item.image)} alt="" className="max-h-40 w-full object-cover" />}
                <div className="px-3 py-2">
                  <div className="flex items-baseline justify-between gap-2">
                    <h3 className="text-sm text-stone-200">{content.title}</h3>
                    <span className="shrink-0 text-[11px] tabular-nums text-stone-600">{(item.date ?? "").slice(0, 10)}</span>
                  </div>
                  <NewsBody text={content.body} id={item.id} />
                  {item.ad?.label && (
                    <p className="mt-1.5 text-[11px] text-stone-600">
                      {panel.hubAd}
                      {item.ad.erid ? ` · erid ${item.ad.erid}` : ""}
                    </p>
                  )}
                  {content.buttons.length > 0 && (
                    <div className="mt-2 flex flex-wrap gap-2">
                      {content.buttons.map((button, index) => (
                        <button
                          key={index}
                          type="button"
                          onClick={() => press(item, button, index)}
                          className={
                            button.style === "primary"
                              ? "rounded border border-amber-700/60 bg-amber-900/30 px-2.5 py-1 text-xs text-amber-100 hover:bg-amber-900/50"
                              : "rounded border border-stone-700 px-2.5 py-1 text-xs text-stone-300 hover:bg-stone-900"
                          }
                        >
                          {button.label}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}
