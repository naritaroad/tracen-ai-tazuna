import { useEffect, useMemo, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api";
import { errorMessage } from "../i18n/errors";
import { SourceBadge } from "../components/Layout";

type Section = { section: string; chapter: string; title: string; content: string; lang: string; visibility: string };
type Hit = Section & { doc: string; distance: number; source_type: string; source_ref: string };
type SearchOutcome = { found: boolean; method: "vector" | "keyword"; results: Hit[] };

/** The trainer handbook: the same search the agent's search_academy_rules tool uses, and the full text by chapter. */
export function HandbookPage() {
  const { t, i18n } = useTranslation();
  const [sections, setSections] = useState<Section[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [outcome, setOutcome] = useState<SearchOutcome | null>(null);
  const [searching, setSearching] = useState(false);

  useEffect(() => {
    api.get<Section[]>(`/handbook?locale=${i18n.language}`).then(setSections).catch((e) => setError(errorMessage(e)));
  }, [i18n.language]);

  const chapters = useMemo(() => {
    const out: { chapter: string; items: Section[] }[] = [];
    for (const s of sections ?? []) {
      const last = out[out.length - 1];
      if (last && last.chapter === s.chapter) last.items.push(s);
      else out.push({ chapter: s.chapter, items: [s] });
    }
    return out;
  }, [sections]);

  async function search(e: FormEvent) {
    e.preventDefault();
    const q = query.trim();
    if (!q) return;
    setSearching(true);
    setError(null);
    try {
      setOutcome(await api.get<SearchOutcome>(`/handbook/search?q=${encodeURIComponent(q)}&locale=${i18n.language}`));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSearching(false);
    }
  }

  const translated = (lang: string) => lang === i18n.language || (lang === "zh-TW" && i18n.language.startsWith("zh"));

  return (
    <div className="stack">
      <h1>{t("handbook.title")}</h1>
      <p className="muted">{t("handbook.intro")}</p>

      <form className="row handbook-search" onSubmit={search}>
        <input
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("handbook.placeholder")}
          aria-label={t("handbook.search")}
          maxLength={300}
        />
        <button className="btn primary" type="submit" disabled={searching || !query.trim()}>
          {t("handbook.search")}
        </button>
      </form>

      {error && <p className="error">{t("common.error", { message: error })}</p>}

      {outcome && (
        <section className="stack" aria-live="polite">
          <h2 style={{ margin: 0 }}>{t("handbook.results")}</h2>
          {!outcome.found && <p className="muted">{t("handbook.noResult")}</p>}
          {outcome.method === "keyword" && <p className="muted small">{t("handbook.keywordMode")}</p>}
          {outcome.results.map((h) => (
            <article key={`${h.doc}-${h.section}`} className="card stack" style={{ gap: 8 }}>
              <h3 style={{ margin: 0 }}>
                {h.section.startsWith("§") ? `${h.section} ` : ""}
                {h.title}
              </h3>
              <p style={{ margin: 0, whiteSpace: "pre-line" }}>{h.content}</p>
              <div className="row muted small" style={{ alignItems: "center" }}>
                <SourceBadge type={h.source_type} />
                <span>{h.chapter}</span>
                {!translated(h.lang) && <span>{t("handbook.untranslated")}</span>}
              </div>
            </article>
          ))}
        </section>
      )}

      {!sections && !error && <p className="muted">{t("common.loading")}</p>}
      {chapters.map((c) => (
        <section key={c.chapter} className="stack" style={{ gap: 8 }}>
          <h2 style={{ margin: "12px 0 0" }}>{c.chapter}</h2>
          {c.items.map((s) => (
            <details key={s.section} className="card handbook-section">
              <summary>
                <strong>{s.section}</strong> {s.title}
              </summary>
              <p style={{ whiteSpace: "pre-line" }}>{s.content}</p>
              {!translated(s.lang) && <p className="muted small">{t("handbook.untranslated")}</p>}
            </details>
          ))}
        </section>
      ))}
      {sections && <p className="muted small">{t("handbook.notice")}</p>}
    </div>
  );
}
