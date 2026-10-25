import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useParams } from "react-router";

import { api, ApiError, sendMessage, type Conversation, type ConversationDetail, type Message, type Ticket, type ToolCall } from "../api";
import { errorMessage } from "../i18n/errors";
import type { AvatarIntent } from "../avatar/states";
import { Icon, type IconName } from "../components/Icon";
import { Mark, MenuButton, useShell } from "../components/Layout";
import { CharacterSlot, useConcierge } from "../concierge/ConciergeProvider";
import { useConversations } from "../conversations";
import { formatDateTime } from "../format";
import { useRecorder } from "../voice/useRecorder";

const SAMPLES: [string, IconName][] = [
  ["sample1", "info"],
  ["sample2", "track"],
  ["sample3", "clock"],
  ["sample4", "calendar"],
  ["sample5", "director"],
];

// Wide screens remember whether the character column is open. Phones decide per reply (see ChatPage).
const AVATAR_KEY = "tracen.avatar.open";

function savedAvatarOpen(): boolean {
  try {
    return localStorage.getItem(AVATAR_KEY) !== "0";
  } catch {
    return true;
  }
}

function saveAvatarOpen(open: boolean): void {
  try {
    if (open) localStorage.removeItem(AVATAR_KEY);
    else localStorage.setItem(AVATAR_KEY, "0");
  } catch {
    /* storage blocked */
  }
}

interface LiveRun {
  calls: ToolCall[];
  error: string | null;
}

function Trace({ calls, model }: { calls: ToolCall[]; model?: string }) {
  const { t } = useTranslation();
  return (
    <details className="trace">
      <summary>
        {t("chat.traceSummary", { count: calls.length })}
        {model && <span className="muted"> · {t("chat.traceModel", { model })}</span>}
      </summary>
      {calls.length === 0 && <div className="muted">{t("chat.traceEmpty")}</div>}
      {calls.map((c) => (
        <div className="call" key={`${c.run_id}-${c.call_id}-${c.step}`}>
          <div>
            <strong>{t(`tools.${c.tool_name}`, { defaultValue: c.tool_name })}</strong> <code>{c.tool_name}</code>{" "}
            <span className={`badge ${c.status === "ok" ? "ok" : "bad"}`}>{t(`status.${c.status}`)}</span>{" "}
            <span className="muted">{t("chat.stepMeta", { step: c.step, duration: t("common.durationMs", { n: c.duration_ms }) })}</span>
          </div>
          <div className="muted">{t("chat.arguments")}</div>
          <pre>{JSON.stringify(c.arguments, null, 2)}</pre>
          <div className="muted">{t("chat.result")}</div>
          {c.result === undefined ? <div className="muted">{t("chat.resultPending")}</div> : <pre>{JSON.stringify(c.result, null, 2)}</pre>}
        </div>
      ))}
    </details>
  );
}

/** Copies a message. The Android app may load the site over plain HTTP, where the Clipboard API is missing. */
function copyText(text: string): Promise<void> {
  if (navigator.clipboard && window.isSecureContext) return navigator.clipboard.writeText(text);
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.append(area);
  area.select();
  const ok = document.execCommand("copy");
  area.remove();
  return ok ? Promise.resolve() : Promise.reject(new Error("copy"));
}

function CopyButton({ text }: { text: string }) {
  const { t } = useTranslation();
  const [done, setDone] = useState(false);
  useEffect(() => {
    if (!done) return;
    const id = window.setTimeout(() => setDone(false), 1600);
    return () => window.clearTimeout(id);
  }, [done]);
  const label = done ? t("chat.copied") : t("chat.copy");
  return (
    <button className="icon-btn small" aria-label={label} title={label} onClick={() => void copyText(text).then(() => setDone(true), () => undefined)}>
      <Icon name={done ? "check" : "copy"} />
    </button>
  );
}

function Bubble({ m, calls, model }: { m: Message; calls: ToolCall[]; model?: string }) {
  const { t, i18n } = useTranslation();
  if (m.role === "notice") return <div className="notice">{t(`notice.${m.content}`, { defaultValue: m.content })}</div>;
  const time = formatDateTime(m.created_at, i18n.language);
  if (m.role === "user")
    return (
      <div className="msg user">
        <div className="bubble">{m.content}</div>
        <div className="msg-time">{time}</div>
      </div>
    );
  return (
    <div className={`msg ${m.role}`}>
      <div className="who">
        <span>{m.role === "director" ? t("chat.director") : t("chat.tazuna")}</span>
        <span className="msg-time">{time}</span>
      </div>
      <div className="bubble">{m.content}</div>
      <div className="msg-foot">
        <CopyButton text={m.content} />
        {m.role === "assistant" && <Trace calls={calls} model={model} />}
      </div>
    </div>
  );
}

export function ChatPage() {
  const { t, i18n } = useTranslation();
  const { conversationId } = useParams();
  const navigate = useNavigate();
  const { mobile } = useShell();
  const { reload: reloadList } = useConversations();
  const [detail, setDetail] = useState<ConversationDetail | null>(null);
  const [waiting, setWaiting] = useState<Ticket | null>(null);
  const [draft, setDraft] = useState("");
  const [pending, setPending] = useState<string | null>(null);
  const [live, setLive] = useState<LiveRun | null>(null);
  const [error, setError] = useState<string | null>(null);
  const abort = useRef<AbortController | null>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  const following = useRef(true);
  const [showJump, setShowJump] = useState(false);
  // The one character of the site; the concierge on other pages continues this conversation.
  const { avatar, follow } = useConcierge();
  const [voiceNote, setVoiceNote] = useState(false);
  // Speech becomes editable text; the trainer still presses send.
  const recorder = useRecorder(
    i18n.language,
    useCallback((text: string) => {
      setDraft((d) => (d.trim() ? `${d.trim()} ${text}` : text));
      setVoiceNote(true);
    }, []),
  );

  // The character: on wide screens a column the trainer opens or closes. On phones it greets a new
  // chat, then steps aside after each reply so the answer has the screen, unless the trainer reopened it.
  const [avatarOpen, setAvatarOpen] = useState(() => (mobile ? !conversationId : savedAvatarOpen()));
  const keepAvatar = useRef(false);
  useEffect(() => {
    if (!mobile) setAvatarOpen(savedAvatarOpen());
    else if (!keepAvatar.current) setAvatarOpen(!conversationId);
  }, [conversationId, mobile]);
  const toggleAvatar = () => {
    const next = !avatarOpen;
    setAvatarOpen(next);
    if (mobile) keepAvatar.current = next;
    else saveAvatarOpen(next);
  };

  useEffect(() => follow(conversationId ?? null), [conversationId, follow]);

  const loadDetail = useCallback(async (id: string) => {
    const [d, tickets] = await Promise.all([api.get<ConversationDetail>(`/conversations/${id}`), api.get<Ticket[]>("/director/tickets")]);
    setDetail(d);
    setWaiting(tickets.find((tk) => tk.conversation_id === id && tk.status === "waiting") ?? null);
  }, []);

  useEffect(() => {
    setDetail(null);
    setWaiting(null);
    setError(null);
    following.current = true;
    if (conversationId)
      loadDetail(conversationId).catch((e) => {
        // A remembered conversation that is gone (or another account's): start a new one instead.
        if (e instanceof ApiError && e.status === 404) navigate("/", { replace: true });
        else setError(errorMessage(e));
      });
    // Wide screens: ready to type. Phones would raise the keyboard over the greeting.
    else if (!window.matchMedia("(pointer: coarse)").matches) input.current?.focus();
  }, [conversationId, loadDetail, navigate]);

  // While a person may answer, check for their replies.
  const personInvolved = detail?.conversation.status === "handoff" || waiting !== null;
  useEffect(() => {
    if (!conversationId || !personInvolved || pending) return;
    const id = window.setInterval(() => void loadDetail(conversationId).catch(() => undefined), 5000);
    return () => window.clearInterval(id);
  }, [conversationId, personInvolved, pending, loadDetail]);

  // Follow new text only while the reader is at the end; scrolling up to read stops it.
  const toEnd = (smooth: boolean) => scroller.current?.scrollTo({ top: scroller.current.scrollHeight, behavior: smooth ? "smooth" : "auto" });
  useEffect(() => {
    if (following.current) toEnd(!!pending);
  }, [detail, pending, live]);
  const onScroll = () => {
    const el = scroller.current;
    if (!el) return;
    const gap = el.scrollHeight - el.scrollTop - el.clientHeight;
    following.current = gap < 80;
    setShowJump(gap > 240);
  };

  // The box grows with the text, up to a limit, like desktop AI assistants.
  useLayoutEffect(() => {
    const el = input.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  }, [draft]);

  const callsByRun = useMemo(() => {
    const map = new Map<string, ToolCall[]>();
    for (const c of detail?.tool_calls ?? []) map.set(c.run_id, [...(map.get(c.run_id) ?? []), c]);
    return map;
  }, [detail]);
  const modelByRun = useMemo(() => new Map((detail?.runs ?? []).map((r) => [r.id, r.model])), [detail]);

  const send = async (text: string) => {
    const content = text.trim();
    if (!content || pending) return;
    setError(null);
    let id = conversationId;
    if (!id) {
      const c = await api.post<Conversation>("/conversations");
      id = c.id;
    }
    const created = id !== conversationId;
    setDraft("");
    setVoiceNote(false);
    setPending(content);
    setLive({ calls: [], error: null });
    following.current = true;
    avatar.userSent();
    abort.current = new AbortController();
    // The server picks the reply language from the message (run.started), not from the interface.
    let replyLang = i18n.language;
    try {
      await sendMessage(
        id,
        content,
        i18n.language,
        (e) => {
          const d = e.data as Record<string, unknown>;
          if (e.type === "run.started" && typeof d.locale === "string") replyLang = d.locale;
          if (e.type === "avatar.intent") avatar.push(d as unknown as AvatarIntent);
          // The reply text: the character "says" it (mouth and nods), in the reply's language.
          if (e.type === "message.delta") avatar.speak(String(d.text ?? ""), replyLang);
          if (e.type === "tool.started")
            setLive((l) => ({
              error: l?.error ?? null,
              calls: [...(l?.calls ?? []), { run_id: "", step: d.step as number, call_id: d.call_id as string, tool_name: d.name as string, arguments: d.arguments, result: undefined, status: "ok", duration_ms: 0 }],
            }));
          if (e.type === "tool.completed")
            setLive((l) => ({
              error: l?.error ?? null,
              calls: (l?.calls ?? []).map((c) =>
                c.call_id === d.call_id && c.step === d.step ? { ...c, result: d.result, status: d.status as ToolCall["status"], duration_ms: d.duration_ms as number } : c,
              ),
            }));
          if (e.type === "run.failed")
            setLive((l) => ({ calls: l?.calls ?? [], error: t(`notice.run.failed.${String(d.code)}`, { defaultValue: String(d.message ?? d.code) }) }));
        },
        abort.current.signal,
      );
    } catch (e) {
      if (!(e instanceof DOMException && e.name === "AbortError")) setError(errorMessage(e));
    } finally {
      setPending(null);
      setLive((l) => (l?.error ? l : null));
      if (mobile && !keepAvatar.current) setAvatarOpen(false);
      // Change the URL only after the stream, so the page is not reset mid-answer.
      if (created) navigate(`/c/${id}`, { replace: true });
      else await loadDetail(id).catch(() => undefined);
      reloadList();
    }
  };

  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      void send(draft);
    }
  };

  const active = detail?.active_ticket;
  const messages = detail?.messages ?? [];
  const empty = messages.length === 0 && !pending && !conversationId;
  const busy = avatar.snapshot.intent.state !== "idle";
  const title = detail?.conversation.title || (conversationId ? t("chat.untitled") : "");

  return (
    <div className={`chat-page ${empty ? "empty" : ""}`}>
      <header className="chat-head">
        <MenuButton />
        <div className="chat-title" title={title}>
          {title}
        </div>
        <button
          className={`icon-btn avatar-toggle ${avatarOpen ? "on" : ""}`}
          aria-pressed={avatarOpen}
          aria-label={avatarOpen ? t("avatar.collapse") : t("avatar.expand")}
          title={avatarOpen ? t("avatar.collapse") : t("avatar.expand")}
          onClick={toggleAvatar}
        >
          <Icon name="character" />
          {!avatarOpen && busy && <span className="pulse" aria-hidden="true" />}
        </button>
        {mobile && (
          <button className="icon-btn" onClick={() => navigate("/")} disabled={!conversationId} aria-label={t("chat.newChat")} title={t("chat.newChat")}>
            <Icon name="newChat" />
          </button>
        )}
      </header>

      <div className="chat-body" ref={scroller} onScroll={onScroll}>
        <div className="chat-col">
          {empty ? (
            <div className="hero">
              <Mark />
              <h1>{t("chat.heroTitle")}</h1>
              <p className="muted">{t("chat.greeting")}</p>
            </div>
          ) : (
            <div className="messages" aria-live="polite">
              {messages.map((m) => (
                <Bubble key={m.id} m={m} calls={m.run_id ? (callsByRun.get(m.run_id) ?? []) : []} model={m.run_id ? modelByRun.get(m.run_id) : undefined} />
              ))}
              {pending && (
                <>
                  <div className="msg user">
                    <div className="bubble">{pending}</div>
                  </div>
                  <div className="msg assistant">
                    <div className="who">{t("chat.tazuna")}</div>
                    <div className="bubble thinking">{t("chat.thinking")}</div>
                    {live && live.calls.length > 0 && (
                      <div className="msg-foot">
                        <Trace calls={live.calls} />
                      </div>
                    )}
                  </div>
                </>
              )}
              {live?.error && <div className="notice error">{t("chat.failed", { message: live.error })}</div>}
            </div>
          )}
          {error && <div className="notice error">{t("common.error", { message: error })}</div>}
        </div>
      </div>

      {empty && (
        <div className="chat-after">
          <div className="chat-col suggestions" role="group" aria-label={t("chat.samples")}>
            {SAMPLES.map(([k, icon]) => (
              <button key={k} className="suggestion" onClick={() => void send(t(`chat.${k}`))}>
                <Icon name={icon} />
                <span>{t(`chat.${k}`)}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="composer">
        <div className="chat-col">
          {showJump && (
            <button className="jump" onClick={() => toEnd(true)} aria-label={t("chat.jumpLatest")} title={t("chat.jumpLatest")}>
              <Icon name="arrowDown" />
            </button>
          )}
          {active && <div className="banner active">{t("chat.handoffActive", { no: active.ticket_no })}</div>}
          {!active && waiting && <div className="banner waiting">{t("chat.handoffWaiting", { no: waiting.ticket_no })}</div>}
          {recorder.state === "recording" && <div className="banner waiting">{t("voice.recording", { s: recorder.seconds })}</div>}
          {recorder.state === "transcribing" && <div className="muted small">{t("voice.transcribing")}</div>}
          {voiceNote && recorder.state === "idle" && <div className="muted small">{t("voice.review")}</div>}
          {recorder.error && (
            <div className="error small">
              {recorder.error === "denied" ? t("voice.denied") : recorder.error === "unsupported" ? t("voice.unsupported") : t("voice.failed", { message: recorder.error.message })}
            </div>
          )}
          <div className="composer-box">
            <textarea
              ref={input}
              rows={1}
              value={draft}
              placeholder={t("chat.placeholder")}
              aria-label={t("chat.placeholder")}
              maxLength={2000}
              onChange={(e) => {
                setDraft(e.target.value);
                avatar.listening(e.target.value.length > 0);
              }}
              onKeyDown={onKey}
            />
            <div className="composer-row">
              <span className="composer-hint">{t("chat.composerHint")}</span>
              <button
                type="button"
                className={`icon-btn ${recorder.state === "recording" ? "recording" : ""}`}
                aria-label={recorder.state === "recording" ? t("voice.stop") : t("voice.record")}
                title={recorder.state === "recording" ? t("voice.stop") : t("voice.record")}
                disabled={!!pending || recorder.state === "transcribing"}
                onClick={() => {
                  if (recorder.state === "recording") recorder.stop();
                  else {
                    avatar.listening(true);
                    void recorder.start();
                  }
                }}
              >
                <Icon name={recorder.state === "recording" ? "stop" : "mic"} />
              </button>
              {pending ? (
                <button className="send-btn" onClick={() => abort.current?.abort()} aria-label={t("chat.stop")} title={t("chat.stop")}>
                  <Icon name="stop" />
                </button>
              ) : (
                <button className="send-btn" disabled={!draft.trim()} onClick={() => void send(draft)} aria-label={t("chat.send")} title={t("chat.send")}>
                  <Icon name="arrowUp" />
                </button>
              )}
            </div>
          </div>
        </div>
      </div>

      <aside className="avatar-stage" aria-label={t("avatar.label")} hidden={!avatarOpen}>
        <CharacterSlot visible={avatarOpen} greeting={!conversationId && !pending ? t("chat.greeting") : undefined} backdrop />
      </aside>
    </div>
  );
}
