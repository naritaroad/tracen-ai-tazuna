// Tazuna, present on every trainer page (docs/character/avatar-contract.md §5).
//
// One character for the whole site: the 3D player loads once and is moved between "slots" — the
// chat page's character column and the floating concierge on the other pages — so changing pages
// never reloads it. One avatar controller drives her wherever she is, and the concierge keeps
// talking in the conversation the trainer last had open, through the same agent and tools as the chat.

import { createContext, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { useLocation } from "react-router";

import { api, sendMessage, type Conversation, type ConversationDetail } from "../api";
import { useAuth } from "../auth";
import { AvatarStage } from "../avatar/AvatarStage";
import { useAvatarController, type AvatarIntent } from "../avatar/states";
import { pageOf } from "../avatar/taps.ts";
import { useConversations } from "../conversations";
import { errorMessage } from "../i18n/errors";

export const CHARACTER = "9001_hayakawa_tazuna";

// The followed conversation survives a reload (the Android app's WebView reloads around permission
// prompts), but not the tab: a new visit starts fresh. Never sent anywhere; the server checks ownership.
const ACTIVE_KEY = "tracen.concierge.conversation";

function savedActive(): string | null {
  try {
    return sessionStorage.getItem(ACTIVE_KEY);
  } catch {
    return null;
  }
}

/** The concierge's latest exchange: the question and her answer as it arrives. */
export interface Turn {
  question: string;
  reply: string;
  status: "thinking" | "working" | "done" | "error";
  /** The tool she is using right now (tool.started). */
  tool: string | null;
  error: string | null;
  /** The case went to the Director's Office. */
  handoff: boolean;
}

interface SlotState {
  visible: boolean;
  greeting?: string;
  /** Show her in front of the background scene (the chat page). */
  backdrop?: boolean;
}

interface ConciergeValue {
  avatar: ReturnType<typeof useAvatarController>;
  /** The conversation the concierge continues: the one last open on the chat page or started here. */
  activeId: string | null;
  /** The chat page reports the conversation it shows. */
  follow: (id: string | null) => void;
  turn: Turn | null;
  busy: boolean;
  ask: (text: string) => Promise<void>;
  stop: () => void;
  /** A new topic: the next question starts a new conversation. */
  reset: () => void;
  /** Shows the last exchange of the active conversation when the concierge has none of its own. */
  recall: () => void;
  attach: (el: HTMLElement, state: SlotState) => () => void;
  update: (el: HTMLElement, state: SlotState) => void;
}

const ConciergeContext = createContext<ConciergeValue | null>(null);

export function useConcierge(): ConciergeValue {
  const value = useContext(ConciergeContext);
  if (!value) throw new Error("useConcierge outside ConciergeProvider");
  return value;
}

export function ConciergeProvider({ children }: { children: ReactNode }) {
  const { t, i18n } = useTranslation();
  const { user, loading } = useAuth();
  const { pathname } = useLocation();
  const { reload } = useConversations();
  const avatar = useAvatarController();

  const [activeId, setActiveId] = useState<string | null>(savedActive);
  const activeRef = useRef<string | null>(activeId);
  const [turn, setTurnState] = useState<Turn | null>(null);
  const turnRef = useRef<Turn | null>(null);
  const [busy, setBusy] = useState(false);
  const abort = useRef<AbortController | null>(null);

  const setTurn = useCallback((next: Turn | null | ((t: Turn | null) => Turn | null)) => {
    turnRef.current = typeof next === "function" ? next(turnRef.current) : next;
    setTurnState(turnRef.current);
  }, []);
  const setActive = useCallback((id: string | null) => {
    activeRef.current = id;
    setActiveId(id);
    try {
      if (id) sessionStorage.setItem(ACTIVE_KEY, id);
      else sessionStorage.removeItem(ACTIVE_KEY);
    } catch {
      /* storage blocked: the concierge starts a new conversation after a reload */
    }
  }, []);

  const follow = useCallback(
    (id: string | null) => {
      if (id === activeRef.current) return;
      setActive(id);
      // Another conversation: the concierge's own exchange belonged to the previous one.
      if (!abort.current) setTurn(null);
    },
    [setActive, setTurn],
  );

  const reset = useCallback(() => {
    abort.current?.abort();
    setActive(null);
    setTurn(null);
  }, [setActive, setTurn]);

  // Signing out (or another account) forgets the conversation; the first check waits for the session.
  useEffect(() => {
    if (!loading && !user) reset();
  }, [loading, user, reset]);

  const recall = useCallback(() => {
    const id = activeRef.current;
    if (!id || turnRef.current) return;
    api
      .get<ConversationDetail>(`/conversations/${id}`)
      .then((d) => {
        if (activeRef.current !== id || turnRef.current) return;
        const last = (roles: string[]) => [...d.messages].reverse().find((m) => roles.includes(m.role));
        const q = last(["user"]);
        const a = last(["assistant", "director"]);
        if (q || a) setTurn({ question: q?.content ?? "", reply: a?.content ?? "", status: "done", tool: null, error: null, handoff: d.conversation.status === "handoff" });
      })
      // Gone or another account's: start afresh.
      .catch(() => activeRef.current === id && setActive(null));
  }, [setActive, setTurn]);

  const ask = useCallback(
    async (text: string) => {
      const content = text.trim();
      if (!content || abort.current) return;
      const controller = new AbortController();
      abort.current = controller;
      setBusy(true);
      setTurn({ question: content, reply: "", status: "thinking", tool: null, error: null, handoff: false });
      avatar.userSent();
      // The server picks the reply language from the message (run.started), not from the interface.
      let replyLang = i18n.language;
      try {
        let id = activeRef.current;
        if (!id) {
          id = (await api.post<Conversation>("/conversations")).id;
          setActive(id);
        }
        await sendMessage(
          id,
          content,
          i18n.language,
          (e) => {
            const d = e.data as Record<string, unknown>;
            if (e.type === "run.started" && typeof d.locale === "string") replyLang = d.locale;
            if (e.type === "avatar.intent") {
              const intent = d as unknown as AvatarIntent;
              avatar.push(intent);
              if (intent.state === "handoff") setTurn((tt) => tt && { ...tt, handoff: true });
            }
            if (e.type === "tool.started") setTurn((tt) => tt && { ...tt, status: "working", tool: String(d.name) });
            if (e.type === "message.delta") {
              const reply = String(d.text ?? "");
              avatar.speak(reply, replyLang);
              setTurn((tt) => tt && { ...tt, reply });
            }
            if (e.type === "run.failed")
              setTurn((tt) => tt && { ...tt, status: "error", error: t(`notice.run.failed.${String(d.code)}`, { defaultValue: String(d.message ?? d.code) }) });
          },
          controller.signal,
        );
        setTurn((tt) => (tt && tt.status !== "error" ? { ...tt, status: "done" } : tt));
      } catch (e) {
        const stopped = e instanceof DOMException && e.name === "AbortError";
        setTurn((tt) => tt && (stopped ? { ...tt, status: "done" } : { ...tt, status: "error", error: errorMessage(e) }));
      } finally {
        abort.current = null;
        setBusy(false);
        reload();
      }
    },
    [avatar, i18n, reload, setActive, setTurn, t],
  );

  const stop = useCallback(() => abort.current?.abort(), []);

  // ---------- the single character and its slots ----------
  const host = useMemo(() => {
    const el = document.createElement("div");
    el.className = "character-host";
    return el;
  }, []);
  const slots = useRef<{ el: HTMLElement; state: SlotState }[]>([]);
  const [slot, setSlot] = useState<SlotState | null>(null);
  // The newest slot shows her; when it goes, she returns to the previous one (or rests unseen).
  const sync = useCallback(() => {
    const top = slots.current[slots.current.length - 1];
    if (top && host.parentElement !== top.el) top.el.appendChild(host);
    if (!top) host.remove();
    setSlot(top ? { ...top.state } : null);
  }, [host]);
  const attach = useCallback(
    (el: HTMLElement, state: SlotState) => {
      slots.current.push({ el, state });
      sync();
      return () => {
        slots.current = slots.current.filter((s) => s.el !== el);
        sync();
      };
    },
    [sync],
  );
  const update = useCallback(
    (el: HTMLElement, state: SlotState) => {
      const s = slots.current.find((x) => x.el === el);
      if (!s) return;
      s.state = state;
      sync();
    },
    [sync],
  );

  const value = useMemo(
    () => ({ avatar, activeId, follow, turn, busy, ask, stop, reset, recall, attach, update }),
    [avatar, activeId, follow, turn, busy, ask, stop, reset, recall, attach, update],
  );

  return (
    <ConciergeContext.Provider value={value}>
      {children}
      {user &&
        createPortal(
          <AvatarStage snapshot={avatar.snapshot} characterId={CHARACTER} greeting={slot?.greeting} active={!!slot?.visible} page={pageOf(pathname)} backdrop={!!slot?.backdrop} />,
          host,
        )}
    </ConciergeContext.Provider>
  );
}

/** Where the character appears. Only one slot is on screen at a time (the chat page or the concierge). */
export function CharacterSlot({ visible, greeting, backdrop = false, className = "" }: SlotState & { className?: string }) {
  const { attach, update } = useConcierge();
  const ref = useRef<HTMLDivElement>(null);
  const first = useRef({ visible, greeting, backdrop });
  useLayoutEffect(() => attach(ref.current!, first.current), [attach]);
  useEffect(() => update(ref.current!, { visible, greeting, backdrop }), [update, visible, greeting, backdrop]);
  return <div ref={ref} className={`character-slot ${className}`} />;
}
