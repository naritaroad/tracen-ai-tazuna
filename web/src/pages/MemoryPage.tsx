import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, type MemoryItem, type Profile } from "../api";
import { useAuth } from "../auth";
import { UserPicture } from "../components/Layout";
import { PictureCropper } from "../components/PictureCropper";
import { errorMessage } from "../i18n/errors";
import { formatDateTime } from "../format";
import { LOCALES } from "../i18n";

interface MemoryEvent {
  id: string;
  content: string;
  conversation_id: string | null;
  created_at: string;
}

function ItemRow({ item, onChanged }: { item: MemoryItem; onChanged: () => void }) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [content, setContent] = useState(item.content);
  return (
    <div className="row" style={{ alignItems: "center" }}>
      <span className="badge" style={{ flex: "0 0 auto" }}>
        {item.kind === "fact" ? t("memory.kindFact") : t("memory.kindPreference")}
      </span>
      <span className={`badge ${item.source === "agent" ? "reference" : ""}`} style={{ flex: "0 0 auto" }}>
        {item.source === "agent" ? t("memory.sourceAgent") : t("memory.sourceTrainer")}
      </span>
      {editing ? <input value={content} maxLength={500} onChange={(e) => setContent(e.target.value)} /> : <span style={{ flex: "1 1 200px" }}>{item.content}</span>}
      {editing ? (
        <button
          className="btn small"
          onClick={() => {
            void api.patch(`/me/memory/${item.id}`, { kind: item.kind, content }).then(() => {
              setEditing(false);
              onChanged();
            });
          }}
        >
          {t("memory.save")}
        </button>
      ) : (
        <button className="btn ghost small" onClick={() => setEditing(true)}>
          {t("memory.edit")}
        </button>
      )}
      <button className="btn ghost small" onClick={() => void api.del(`/me/memory/${item.id}`).then(onChanged)}>
        {t("memory.delete")}
      </button>
    </div>
  );
}

/** Side of the uploaded square; the site shows it at most 96 px wide on a 2x screen. */
const PICTURE_PX = 256;

function PictureCard() {
  const { t } = useTranslation();
  const { user, patch } = useAuth();
  const input = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** The chosen photo, open in the cropper until it is used or dropped. */
  const [photo, setPhoto] = useState<ImageBitmap | null>(null);
  if (!user) return null;

  const close = () => {
    photo?.close();
    setPhoto(null);
    if (input.current) input.current.value = "";
  };

  const choose = async (file: File | undefined) => {
    if (!file) return;
    setError(null);
    try {
      setPhoto(await createImageBitmap(file));
    } catch {
      setError(t("memory.avatarBadFile"));
      if (input.current) input.current.value = "";
    }
  };

  const save = async (blob: Blob) => {
    close();
    setBusy(true);
    try {
      const form = new FormData();
      form.append("image", blob, "avatar");
      patch(await api.upload<{ avatar_version: number }>("/me/avatar", form));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setError(null);
    try {
      await api.del("/me/avatar");
      patch({ avatar_version: null });
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <div className="card picture-card">
      <UserPicture user={user} size="lg" />
      <div className="stack">
        <h2>{t("memory.avatarTitle")}</h2>
        <p className="muted small">{t("memory.avatarIntro")}</p>
        <div className="row">
          <input ref={input} type="file" accept="image/png,image/jpeg,image/webp" hidden onChange={(e) => void choose(e.target.files?.[0])} />
          <button className="btn" disabled={busy} onClick={() => input.current?.click()}>
            {busy ? t("memory.avatarSaving") : user.avatar_version ? t("memory.avatarChange") : t("memory.avatarUpload")}
          </button>
          {user.avatar_version && (
            <button className="btn ghost" disabled={busy} onClick={() => void remove()}>
              {t("memory.avatarRemove")}
            </button>
          )}
        </div>
        {error && <p className="error small">{error}</p>}
      </div>
      {photo && <PictureCropper image={photo} size={PICTURE_PX} onCancel={close} onDone={(blob) => void save(blob)} />}
    </div>
  );
}

export function MemoryPage() {
  const { t, i18n } = useTranslation();
  const { patch } = useAuth();
  const [events, setEvents] = useState<MemoryEvent[]>([]);
  const [profile, setProfile] = useState<Profile>({ preferred_name: "", reply_locale: "" });
  const [items, setItems] = useState<MemoryItem[]>([]);
  const [draft, setDraft] = useState("");
  const [kind, setKind] = useState<"preference" | "fact">("preference");
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadItems = useCallback(() => {
    api.get<MemoryItem[]>("/me/memory").then(setItems).catch((e) => setError(errorMessage(e)));
    api.get<MemoryEvent[]>("/me/memory/events").then(setEvents).catch((e) => setError(errorMessage(e)));
  }, []);

  useEffect(() => {
    api.get<Profile>("/me/profile").then(setProfile).catch((e) => setError(errorMessage(e)));
    loadItems();
  }, [loadItems]);

  const saveProfile = async () => {
    setSaved(false);
    try {
      const next = await api.put<Profile>("/me/profile", { preferred_name: profile.preferred_name || null, reply_locale: profile.reply_locale || null });
      setProfile(next);
      // The account button shows this name.
      patch({ preferred_name: next.preferred_name });
      setSaved(true);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <div className="stack">
      <h1>{t("memory.title")}</h1>
      <p className="muted">{t("memory.intro")}</p>
      <PictureCard />
      <div className="card row">
        <label>
          {t("memory.preferredName")}
          <input maxLength={40} value={profile.preferred_name ?? ""} onChange={(e) => setProfile({ ...profile, preferred_name: e.target.value })} />
        </label>
        <label>
          {t("memory.replyLocale")}
          <select value={profile.reply_locale ?? ""} onChange={(e) => setProfile({ ...profile, reply_locale: e.target.value })}>
            <option value="">{t("memory.auto")}</option>
            {LOCALES.map((l) => (
              <option key={l.code} value={l.code}>
                {l.label}
              </option>
            ))}
          </select>
        </label>
        <button className="btn" onClick={() => void saveProfile()}>
          {t("memory.save")}
        </button>
        {saved && <span className="muted">{t("memory.saved")}</span>}
      </div>

      <div className="card stack">
        <h2>{t("memory.items")}</h2>
        {items.length === 0 && <p className="muted">{t("memory.empty")}</p>}
        {items.map((i) => (
          <ItemRow key={i.id} item={i} onChanged={loadItems} />
        ))}
        <div className="row">
          <select value={kind} onChange={(e) => setKind(e.target.value as "preference" | "fact")} style={{ flex: "0 0 140px" }}>
            <option value="preference">{t("memory.kindPreference")}</option>
            <option value="fact">{t("memory.kindFact")}</option>
          </select>
          <input maxLength={500} value={draft} onChange={(e) => setDraft(e.target.value)} />
          <button
            className="btn secondary"
            disabled={!draft.trim()}
            onClick={() => {
              void api.post("/me/memory", { kind, content: draft }).then(() => {
                setDraft("");
                loadItems();
              });
            }}
          >
            {t("memory.add")}
          </button>
        </div>
      </div>
      <div className="card stack">
        <h2>{t("memory.events")}</h2>
        <p className="muted small">{t("memory.eventsIntro")}</p>
        {events.length === 0 && <p className="muted">{t("memory.eventsEmpty")}</p>}
        {events.map((ev) => (
          <div key={ev.id} className="row" style={{ alignItems: "center" }}>
            <span className="muted small" style={{ flex: "0 0 auto" }}>
              {formatDateTime(ev.created_at, i18n.language)}
            </span>
            <span style={{ flex: "1 1 220px" }}>{ev.content}</span>
            <button className="btn ghost small" onClick={() => void api.del(`/me/memory/events/${ev.id}`).then(loadItems)}>
              {t("memory.delete")}
            </button>
          </div>
        ))}
      </div>
      {error && <p className="error">{t("common.error", { message: error })}</p>}
    </div>
  );
}
