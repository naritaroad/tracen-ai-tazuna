// The floating concierge on trainer pages other than the chat: Tazuna in the corner, her words in a
// speech bubble above her head, questions that fit the page laid over her, and a box to ask her.
// Wide screens dock it at the bottom right; phones open it as a sheet from a button.

import { useEffect, useRef, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { NavLink, useLocation, useNavigate } from "react-router";

import { pageOf } from "../avatar/taps.ts";
import { Icon } from "../components/Icon";
import { useShell } from "../components/Layout";
import { CharacterSlot, useConcierge } from "./ConciergeProvider";

/** Questions offered on each page (language-file keys `concierge.ask.<page>.<n>`). */
const ASK_COUNT: Record<string, number> = { info: 3, calendar: 3, services: 3, planner: 3, director: 3, memory: 3 };

const OPEN_KEY = "tracen.concierge.open";

function savedOpen(): boolean {
  try {
    return localStorage.getItem(OPEN_KEY) !== "0";
  } catch {
    return true;
  }
}

function saveOpen(open: boolean): void {
  try {
    if (open) localStorage.removeItem(OPEN_KEY);
    else localStorage.setItem(OPEN_KEY, "0");
  } catch {
    /* storage blocked */
  }
}

export function Concierge() {
  const { t } = useTranslation();
  const { mobile } = useShell();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const c = useConcierge();
  const page = pageOf(pathname);
  // Phones start closed: the page comes first. Wide screens remember the trainer's choice.
  const [open, setOpen] = useState(() => !mobile && savedOpen());
  const [draft, setDraft] = useState("");
  const [unseen, setUnseen] = useState(false);
  const bubble = useRef<HTMLDivElement>(null);
  const status = c.turn?.status;

  const show = (next: boolean) => {
    setOpen(next);
    if (!mobile) saveOpen(next);
    if (next) setUnseen(false);
  };

  useEffect(() => {
    if (open) c.recall();
  }, [open, c.recall, c.activeId]);

  // A finished answer while she is put away: a dot on the button.
  const prev = useRef(status);
  useEffect(() => {
    if (prev.current !== status && (status === "done" || status === "error") && prev.current && !open) setUnseen(true);
    prev.current = status;
  }, [status, open]);

  useEffect(() => {
    bubble.current?.scrollTo({ top: 0 });
  }, [c.turn?.question]);

  useEffect(() => {
    if (!open || !mobile) return;
    const esc = (e: KeyboardEvent) => e.key === "Escape" && show(false);
    document.addEventListener("keydown", esc);
    return () => document.removeEventListener("keydown", esc);
  });

  const submit = (e?: FormEvent) => {
    e?.preventDefault();
    if (!draft.trim() || c.busy) return;
    void c.ask(draft);
    setDraft("");
    c.avatar.listening(false);
  };

  if (!open)
    return (
      <button className={`concierge-launcher ${mobile ? "round" : ""}`} onClick={() => show(true)} aria-label={t("concierge.open")} title={t("concierge.open")}>
        <Icon name="character" />
        {!mobile && <span>{t("concierge.open")}</span>}
        {(unseen || c.busy) && <span className={`dot ${c.busy ? "busy" : ""}`} aria-hidden="true" />}
      </button>
    );

  const turn = c.turn;
  const toolName = turn?.tool ? t(`tools.${turn.tool}`, { defaultValue: turn.tool }) : "";
  const asks = page ? Array.from({ length: ASK_COUNT[page] ?? 0 }, (_, i) => t(`concierge.ask.${page}.${i + 1}`)) : [];

  return (
    <>
      {mobile && <div className="concierge-scrim" onClick={() => show(false)} />}
      <section className={`concierge ${mobile ? "sheet" : "dock"}`} aria-label={t("concierge.label")}>
        <div className="concierge-head">
          <span className="concierge-name">
            {t("chat.tazuna")}
            <small>{t(`avatar.state.${c.avatar.snapshot.intent.state}`)}</small>
          </span>
          <button className="icon-btn small" onClick={() => c.activeId && navigate(`/c/${c.activeId}`)} disabled={!c.activeId} aria-label={t("concierge.openChat")} title={t("concierge.openChat")}>
            <Icon name="chat" />
          </button>
          <button className="icon-btn small" onClick={c.reset} disabled={!c.activeId && !turn} aria-label={t("concierge.newTopic")} title={t("concierge.newTopic")}>
            <Icon name="newChat" />
          </button>
          <button className="icon-btn small" onClick={() => show(false)} aria-label={t("concierge.close")} title={t("concierge.close")}>
            <Icon name={mobile ? "close" : "arrowDown"} />
          </button>
        </div>

        <div className="concierge-stage">
          <CharacterSlot visible greeting={!turn ? t("concierge.greeting") : undefined} />

          {/* Her words, in the room the stage keeps above her head. */}
          <div className="concierge-bubble">
            <div className="concierge-bubble-body" ref={bubble} aria-live="polite">
              {!turn && <p>{t("concierge.greeting")}</p>}
              {turn?.question && <div className="concierge-q">{t("concierge.asked", { q: turn.question })}</div>}
              {turn?.status === "thinking" && <p className="bubble thinking">{t("chat.thinking")}</p>}
              {turn?.status === "working" && !turn.reply && <p className="bubble thinking">{t("concierge.working", { tool: toolName })}</p>}
              {turn?.reply && <p className="concierge-reply">{turn.reply}</p>}
              {turn?.status === "done" && !turn.reply && <p className="muted">{t("concierge.noReply")}</p>}
              {turn?.status === "error" && <p className="error">{turn.error}</p>}
              {turn && (turn.handoff || status === "done") && (
                <div className="concierge-links">
                  {turn.handoff && <NavLink to="/director">{t("concierge.handoff")}</NavLink>}
                  {status === "done" && c.activeId && <NavLink to={`/c/${c.activeId}`}>{t("concierge.more")}</NavLink>}
                </div>
              )}
            </div>
          </div>

          {asks.length > 0 && !c.busy && (
            <div className="concierge-asks" role="group" aria-label={t("chat.samples")}>
              {asks.map((q) => (
                <button key={q} type="button" onClick={() => void c.ask(q)}>
                  {q}
                </button>
              ))}
            </div>
          )}
        </div>

        <form className="concierge-input" onSubmit={submit}>
          <input
            value={draft}
            maxLength={2000}
            placeholder={t("concierge.placeholder")}
            aria-label={t("concierge.placeholder")}
            onChange={(e) => {
              setDraft(e.target.value);
              c.avatar.listening(e.target.value.length > 0);
            }}
            onBlur={() => !draft && c.avatar.listening(false)}
          />
          {c.busy ? (
            <button type="button" className="send-btn" onClick={c.stop} aria-label={t("chat.stop")} title={t("chat.stop")}>
              <Icon name="stop" />
            </button>
          ) : (
            <button type="submit" className="send-btn" disabled={!draft.trim()} aria-label={t("chat.send")} title={t("chat.send")}>
              <Icon name="arrowUp" />
            </button>
          )}
        </form>
      </section>
    </>
  );
}
