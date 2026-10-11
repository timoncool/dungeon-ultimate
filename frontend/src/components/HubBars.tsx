import { useEffect, useState, type ReactNode } from "react";
import { X } from "lucide-react";
import { HUB_GRADIENTS, HUB_TEXT, hubChanged, hubText, reportNotice, stackTheme, useHubState, type HubItem } from "@/lib/studioHub";
import { useUiLanguage } from "@/lib/ui-text-context";
import { panelText } from "@/lib/ui-text-panels";

const MAX_VISIBLE = 3;

export function inline(text: string, key: string): ReactNode[] {
  const out: ReactNode[] = [];
  const pattern = /\*\*([^*]+)\*\*|\[([^\]]+)\]\((https:\/\/[^)\s]+)\)/g;
  let last = 0;
  let index = 0;
  for (const match of text.matchAll(pattern)) {
    if (match.index > last) out.push(text.slice(last, match.index));
    const id = `${key}-${index++}`;
    if (match[1]) out.push(<strong key={id} className="font-semibold">{match[1]}</strong>);
    else out.push(<a key={id} href={match[3]} target="_blank" rel="noreferrer" className="underline underline-offset-2 hover:opacity-90">{match[2]}</a>);
    last = match.index + match[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

/** Полоски хаба вверху окна: во всю ширину, текст по центру со ссылками, крестик; не больше трёх. */
export function HubBars() {
  const lang = useUiLanguage();
  const panel = panelText(lang);
  const hub = useHubState(lang);
  const [closed, setClosed] = useState<Set<string>>(() => new Set());
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    const started = Date.now();
    const timer = window.setInterval(() => setElapsed(Math.floor((Date.now() - started) / 1000)), 1000);
    return () => window.clearInterval(timer);
  }, []);
  const items: HubItem[] = hub?.items ?? [];
  const visible = items
    .filter((item) => item.kind === "bar" && item.eligible && !closed.has(item.id) && elapsed >= (item.rules?.delay_s ?? 0))
    .slice(0, MAX_VISIBLE);
  const shownIds = visible.map((item) => item.id).join(",");
  useEffect(() => {
    for (const id of shownIds ? shownIds.split(",") : []) reportNotice(id, "shown").catch((e: Error) => console.warn("[hub] shown not recorded:", e.message));
  }, [shownIds]);
  if (!visible.length) return null;
  const close = (id: string) => {
    setClosed((previous) => new Set(previous).add(id));
    reportNotice(id, "dismissed").catch((e: Error) => console.warn("[hub] close not recorded:", e.message)).finally(hubChanged);
  };
  return (
    <div role="region" aria-label={panel.hubNotices} className="-mx-3 -mt-3 mb-3 shrink-0 sm:-mx-4 md:-mx-8 md:-mt-4 2xl:-mx-12">
      {visible.map((item, index) => {
        const theme = stackTheme(item.theme, index);
        const content = hubText(item, lang);
        return (
          <div key={item.id} className="flex items-start justify-between gap-3 px-6 py-2 text-[13px] leading-6"
               style={{ background: HUB_GRADIENTS[theme], color: HUB_TEXT[theme] }}
               onClick={(e) => { if ((e.target as HTMLElement).closest("a")) reportNotice(item.id, "clicked", "link").catch((err: Error) => console.warn("[hub] click not recorded:", err.message)); }}>
            <div className="w-full text-center">{inline((content?.body || content?.title || "").replace(/\s*\n\s*/g, " "), item.id)}</div>
            {item.dismissible && (
              <button type="button" onClick={() => close(item.id)} aria-label={panel.hubClose} className="shrink-0 rounded-md p-0.5 opacity-85 hover:opacity-100">
                <X size={16} />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
