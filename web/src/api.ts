// Thin client for the Rust API (/api/v1). Same origin, cookie session, JSON only.

export type Role = "trainer" | "staff" | "admin";

export interface User {
  id: string;
  username: string;
  display_name: string;
  role: Role;
  preferred_locale: string | null;
  /** Changes with each new profile picture; null without one. */
  avatar_version: number | null;
  /** What the trainer asked to be called ("My profile"). */
  preferred_name: string | null;
}

/** The signed-in trainer's profile picture (a new URL for each version, so the browser may cache it). */
export const avatarUrl = (u: User) => (u.avatar_version ? `/api/v1/me/avatar?v=${u.avatar_version}` : null);

export interface InfoView {
  topic: string;
  title: string;
  content: string;
  source_type: SourceType;
  source_ref: string;
  updated_on: string;
}

export type SourceType = "official" | "anime" | "community" | "reference" | "project";

export interface ServiceView {
  id: string;
  name: string;
  category: string;
  unit_minutes: number;
  capacity: number;
  fee_jpy: number;
  available: boolean;
  location: string;
  notes: string;
  source_type: SourceType;
}

export interface EventView {
  id: string;
  title: string;
  kind: string;
  starts_at: string;
  ends_at: string;
  affects_services: string[];
  source_type: SourceType;
  /** Present on race-schedule entries (`kind === "race"`). */
  race?: RaceView;
}

export interface RaceView {
  grade: "G1" | "G2" | "G3";
  racecourse: string;
  surface: "turf" | "dirt";
  distance_m: number;
  direction: "right" | "left" | "straight";
  classes: ("junior" | "classic" | "senior")[];
  source_ref: string;
}

export interface PlanItem {
  service: string;
  sessions: number;
  minutes_per_session?: number;
}

export interface PlanResult {
  complete: boolean;
  lines: {
    service_id: string;
    name: string;
    sessions: number;
    minutes_per_session: number;
    minutes: number;
    fee_per_session_jpy: number;
    fee_jpy: number;
    available: boolean;
  }[];
  problems: { item: string; kind: string; candidates: string[] }[];
  total_minutes: number | null;
  total_fee_jpy: number | null;
  formula: string | null;
  schedule: { date: string; start: string; end: string; blocks: { service_id: string; name: string; start: string; end: string }[] } | null;
  conflicts: { kind: string; service_id: string; event_id: string | null; event_title: string | null; detail: string }[];
  note: string;
}

export interface Conversation {
  id: string;
  title: string;
  status: "active" | "handoff" | "closed";
  pinned: boolean;
  created_at: string;
  updated_at: string;
}

export interface Message {
  id: string;
  role: "user" | "assistant" | "director" | "notice";
  content: string;
  run_id: string | null;
  author_name: string | null;
  created_at: string;
}

export interface ToolCall {
  run_id: string;
  step: number;
  call_id: string;
  tool_name: string;
  arguments: unknown;
  result: unknown;
  status: "ok" | "rejected" | "error";
  duration_ms: number;
}

export interface Ticket {
  id: string;
  ticket_no: string;
  requester_id: string;
  requester_name: string;
  conversation_id: string | null;
  reason: string;
  summary: string;
  origin: "agent" | "form";
  status: "waiting" | "in_progress" | "resolved" | "returned";
  assignee_name: string | null;
  created_at: string;
  accepted_at: string | null;
  closed_at: string | null;
}

export interface TicketEvent {
  id: string;
  kind: string;
  body: string;
  actor_name: string | null;
  actor_role: string | null;
  created_at: string;
}

export interface ConversationDetail {
  conversation: Conversation;
  messages: Message[];
  runs: { id: string; status: string; model: string; locale: string; steps: number; error_code: string | null }[];
  tool_calls: ToolCall[];
  active_ticket: Ticket | null;
}

export interface MemoryItem {
  id: string;
  kind: "preference" | "fact" | "summary";
  content: string;
  source: string;
  created_at: string;
  updated_at: string;
}

export interface Profile {
  preferred_name: string | null;
  reply_locale: string | null;
}

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
  ) {
    super(message);
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api/v1${path}`, {
    method,
    credentials: "same-origin",
    headers: body === undefined ? {} : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!res.ok) {
    let code = "http_error";
    let message = res.statusText;
    try {
      const err = await res.json();
      code = err.error?.code ?? code;
      message = err.error?.message ?? message;
    } catch {
      /* not JSON */
    }
    throw new ApiError(res.status, code, message);
  }
  const text = await res.text();
  return (text ? JSON.parse(text) : undefined) as T;
}

/** Sends a form (file uploads); the server reads these as multipart, like the microphone input. */
async function upload<T>(method: string, path: string, form: FormData): Promise<T> {
  const res = await fetch(`/api/v1${path}`, { method, credentials: "same-origin", body: form });
  if (!res.ok) {
    let code = "http_error";
    let message = res.statusText;
    try {
      const err = await res.json();
      code = err.error?.code ?? code;
      message = err.error?.message ?? message;
    } catch {
      /* not JSON */
    }
    throw new ApiError(res.status, code, message);
  }
  return (await res.json()) as T;
}

export const api = {
  get: <T>(path: string) => request<T>("GET", path),
  post: <T>(path: string, body: unknown = {}) => request<T>("POST", path, body),
  put: <T>(path: string, body: unknown) => request<T>("PUT", path, body),
  patch: <T>(path: string, body: unknown) => request<T>("PATCH", path, body),
  del: <T>(path: string) => request<T>("DELETE", path),
  upload: <T>(path: string, form: FormData) => upload<T>("PUT", path, form),
};

/** One event of the chat stream (docs/api/stream-events.md). */
export interface StreamEvent {
  v: number;
  seq: number;
  type: string;
  data: Record<string, unknown>;
}

/** POST a chat message and read the SSE answer. EventSource cannot POST, so this parses the stream. */
export async function sendMessage(
  conversationId: string,
  content: string,
  locale: string,
  onEvent: (e: StreamEvent) => void,
  signal?: AbortSignal,
): Promise<void> {
  const res = await fetch(`/api/v1/conversations/${conversationId}/messages`, {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json", Accept: "text/event-stream" },
    body: JSON.stringify({ content, locale }),
    signal,
  });
  if (!res.ok || !res.body) {
    let message = res.statusText;
    try {
      message = (await res.json()).error?.message ?? message;
    } catch {
      /* not JSON */
    }
    throw new ApiError(res.status, "stream_error", message);
  }
  const reader = res.body.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += value;
    let cut: number;
    while ((cut = buffer.indexOf("\n\n")) >= 0) {
      const chunk = buffer.slice(0, cut);
      buffer = buffer.slice(cut + 2);
      const data = chunk
        .split("\n")
        .filter((l) => l.startsWith("data:"))
        .map((l) => l.slice(5).trimStart())
        .join("\n");
      if (data) onEvent(JSON.parse(data) as StreamEvent);
    }
  }
}
