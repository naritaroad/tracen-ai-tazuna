import { createContext, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type PointerEvent, type ReactNode, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import { NavLink, useLocation, useNavigate } from "react-router";

import { avatarUrl, type Conversation, type Role, type User } from "../api";
import { useAuth } from "../auth";
import { useCrest } from "../brand";
import { Concierge } from "../concierge/Concierge";
import { useConversations } from "../conversations";
import { LOCALES } from "../i18n";
import { forgetLastPath } from "../routeMemory";
import { applyTheme, loadTheme, type ThemeChoice } from "../theme";
import { Icon, type IconName } from "./Icon";

/** Academy mark: the official crest when this machine has it (src/brand.ts), else a gold horseshoe on uniform violet. */
export function Mark() {
  const crest = useCrest();
  if (crest) return <span className="mark crest" aria-hidden="true" style={{ "--crest": `url(${crest})` } as CSSProperties} />;
  return (
    <svg className="mark" viewBox="0 0 64 64" aria-hidden="true">
      <rect width="64" height="64" rx="14" fill="var(--brand)" />
      <path d="M20 46V30a12 12 0 0 1 24 0v16" fill="none" stroke="var(--gold)" strokeWidth="7" strokeLinecap="round" />
    </svg>
  );
}

/** Language and theme pickers; `labelled` shows their names (the account menu), otherwise only aria-labels (the login card). */
export function Controls({ labelled = false }: { labelled?: boolean }) {
  const { t, i18n } = useTranslation();
  const [theme, setTheme] = useState<ThemeChoice>(loadTheme);
  const language = (
    <select aria-label={t("settings.language")} value={i18n.resolvedLanguage} onChange={(e) => void i18n.changeLanguage(e.target.value)}>
      {LOCALES.map((l) => (
        <option key={l.code} value={l.code}>
          {l.label}
        </option>
      ))}
    </select>
  );
  const themes = (
    <select
      aria-label={t("settings.theme")}
      value={theme}
      onChange={(e) => {
        const v = e.target.value as ThemeChoice;
        setTheme(v);
        applyTheme(v);
      }}
    >
      <option value="system">{t("settings.system")}</option>
      <option value="light">{t("settings.light")}</option>
      <option value="dark">{t("settings.dark")}</option>
    </select>
  );
  if (!labelled)
    return (
      <div className="controls">
        {language}
        {themes}
      </div>
    );
  return (
    <div className="controls labelled">
      <label>
        {t("settings.language")}
        {language}
      </label>
      <label>
        {t("settings.theme")}
        {themes}
      </label>
    </div>
  );
}

export const MOBILE_QUERY = "(max-width: 760px)";

export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const mq = window.matchMedia(query);
    const on = () => setMatches(mq.matches);
    on();
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, [query]);
  return matches;
}

interface Shell {
  /** Phone width: the sidebar is a drawer and the pages sit in the side bubble. */
  mobile: boolean;
  openDrawer: () => void;
}

const ShellContext = createContext<Shell>({ mobile: false, openDrawer: () => undefined });
export const useShell = () => useContext(ShellContext);

/** The phone's menu button; pages with their own top bar (the chat) put it at the start. */
export function MenuButton() {
  const { t } = useTranslation();
  const { mobile, openDrawer } = useShell();
  if (!mobile) return null;
  return (
    <button className="icon-btn" onClick={openDrawer} aria-label={t("shell.openMenu")} title={t("shell.openMenu")}>
      <Icon name="menu" />
    </button>
  );
}

interface NavItem {
  to: string;
  key: string;
  icon: IconName;
}

function navItems(admin: boolean, role: Role | undefined): NavItem[] {
  if (admin)
    return [
      { to: "/admin", key: "nav.inbox", icon: "inbox" },
      { to: "/admin/catalog", key: "nav.catalog", icon: "database" },
      { to: "/admin/runs", key: "nav.runs", icon: "activity" },
      { to: "/admin/settings", key: "nav.settings", icon: "sliders" },
      ...(role === "admin" ? [{ to: "/admin/users", key: "nav.users", icon: "users" as const }] : []),
      { to: "/", key: "nav.trainerSite", icon: "back" },
    ];
  return [
    { to: "/", key: "nav.chat", icon: "chat" },
    { to: "/info", key: "nav.info", icon: "info" },
    { to: "/calendar", key: "nav.calendar", icon: "calendar" },
    { to: "/services", key: "nav.services", icon: "track" },
    { to: "/planner", key: "nav.planner", icon: "clock" },
    { to: "/director", key: "nav.director", icon: "director" },
    ...(role && role !== "trainer" ? [{ to: "/admin", key: "nav.admin", icon: "shield" as const }] : []),
  ];
}

const isMac = /Mac|iPhone|iPad/.test(navigator.userAgent);
const shortcut = (key: string, shift: boolean) => (isMac ? `⌘${shift ? "⇧" : ""}${key}` : `Ctrl+${shift ? "Shift+" : ""}${key}`);

const RAIL_KEY = "tracen.sidebar.rail";

function loadRail(): boolean {
  try {
    return localStorage.getItem(RAIL_KEY) === "1";
  } catch {
    return false;
  }
}

function saveRail(rail: boolean): void {
  try {
    if (rail) localStorage.setItem(RAIL_KEY, "1");
    else localStorage.removeItem(RAIL_KEY);
  } catch {
    /* storage blocked: the choice lasts for this page only */
  }
}

/** The "My profile" page lives in the account menu, not among the pages. */
const PROFILE: NavItem = { to: "/memory", key: "nav.memory", icon: "user" };

function ConversationLink({ c }: { c: Conversation }) {
  const { t } = useTranslation();
  const { pin } = useConversations();
  const title = c.title || t("chat.untitled");
  const label = c.pinned ? t("shell.unpin") : t("shell.pin");
  return (
    <div className={`sb-conv-row ${c.pinned ? "pinned" : ""}`}>
      <NavLink to={`/c/${c.id}`} className="sb-conv" title={title}>
        {c.status === "handoff" && <span className="dot director" role="img" aria-label={t("chat.handoffTag")} title={t("chat.handoffTag")} />}
        <span>{title}</span>
      </NavLink>
      <button type="button" className="sb-pin" aria-pressed={c.pinned} aria-label={`${label}: ${title}`} title={label} onClick={() => void pin(c.id, !c.pinned).catch(() => undefined)}>
        <Icon name="pin" />
      </button>
    </div>
  );
}

function History({ query }: { query: string }) {
  const { t } = useTranslation();
  const { list } = useConversations();
  const q = query.trim().toLowerCase();
  const shown = q ? list.filter((c) => (c.title || t("chat.untitled")).toLowerCase().includes(q)) : list;
  const pinned = shown.filter((c) => c.pinned);
  const rest = shown.filter((c) => !c.pinned);
  return (
    <>
      {pinned.length > 0 && (
        <div className="sb-history">
          <div className="sb-heading">{t("shell.pinned")}</div>
          {pinned.map((c) => (
            <ConversationLink key={c.id} c={c} />
          ))}
        </div>
      )}
      <div className="sb-history">
        <div className="sb-heading">{t("chat.history")}</div>
        {rest.map((c) => (
          <ConversationLink key={c.id} c={c} />
        ))}
        {shown.length === 0 && <div className="sb-empty">{q ? t("shell.noMatch") : t("shell.noHistory")}</div>}
      </div>
    </>
  );
}

/** The name the trainer asked to be called ("My profile"), else the account's name. */
export const shownName = (user: User) => user.preferred_name?.trim() || user.display_name;

/** The trainer's picture, or the first letter of the name on violet. */
export function UserPicture({ user, size = "md" }: { user: User; size?: "md" | "lg" }) {
  const { t } = useTranslation();
  const url = avatarUrl(user);
  const [broken, setBroken] = useState<string | null>(null);
  const name = shownName(user);
  if (url && broken !== url) return <img className={`user-pic ${size}`} src={url} alt={t("memory.avatarAlt", { name })} onError={() => setBroken(url)} />;
  return (
    <span className={`user-pic initial ${size}`} aria-hidden="true">
      {Array.from(name.trim())[0] ?? ""}
    </span>
  );
}

/** Name in large type over a quieter line: the account's name when a preferred name is set, else the role. */
function WhoLines({ user }: { user: User }) {
  const { t } = useTranslation();
  const role = t(`role.${user.role}`);
  const preferred = !!user.preferred_name?.trim();
  return (
    <span className="who-lines">
      <span className="who-name">{shownName(user)}</span>
      <span className="who-sub">{preferred ? t("shell.whoSub", { name: user.display_name, role }) : role}</span>
    </span>
  );
}

function Account({ rail }: { rail: boolean }) {
  const { t } = useTranslation();
  const { user, logout } = useAuth();
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const outside = (e: globalThis.PointerEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  if (!user) return null;
  return (
    <div className={`account ${rail ? "rail" : ""}`} ref={box}>
      {open && (
        <div className="account-menu" role="dialog" aria-label={t("shell.account")}>
          <div className="account-who">
            <UserPicture user={user} size="lg" />
            <WhoLines user={user} />
          </div>
          <NavLink to={PROFILE.to} className="sb-item" onClick={() => setOpen(false)}>
            <Icon name={PROFILE.icon} />
            <span>{t(PROFILE.key)}</span>
          </NavLink>
          <Controls labelled />
          <button
            className="sb-item"
            onClick={() => {
              forgetLastPath();
              void logout().then(() => navigate("/login"));
            }}
          >
            <Icon name="logout" />
            <span>{t("nav.logout")}</span>
          </button>
        </div>
      )}
      <button className={`account-btn ${open ? "open" : ""}`} aria-expanded={open} aria-haspopup="dialog" title={t("shell.account")} onClick={() => setOpen((o) => !o)}>
        <UserPicture user={user} />
        <span className="sb-label account-text">
          <WhoLines user={user} />
        </span>
        <span className="sb-label account-chevron">
          <Icon name="chevronUp" />
        </span>
      </button>
    </div>
  );
}

interface SidebarProps {
  admin: boolean;
  items: NavItem[];
  rail: boolean;
  mobile: boolean;
  open: boolean;
  onRail: () => void;
  onClose: () => void;
  search: RefObject<HTMLInputElement | null>;
  onSearch: () => void;
}

function Sidebar({ admin, items, rail, mobile, open, onRail, onClose, search, onSearch }: SidebarProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const [query, setQuery] = useState("");
  // The chat is reached by "new conversation" and the history, so the page list skips it.
  const pages = admin ? items : items.filter((i) => i.to !== "/");
  const homeActive = !admin && pathname === "/";

  return (
    <aside className={`sidebar ${rail ? "rail" : ""} ${mobile ? "drawer" : ""} ${open ? "open" : ""}`} aria-label={t("shell.sidebar")} inert={mobile && !open}>
      <div className="sb-head">
        <NavLink to={admin ? "/admin" : "/"} end className="brand" title={t("app.name")}>
          <Mark />
          <span className="sb-label">
            {t("app.short")}
            {admin && <em>{t("nav.admin")}</em>}
          </span>
        </NavLink>
        <button
          className="icon-btn"
          onClick={mobile ? onClose : onRail}
          aria-label={mobile ? t("shell.closeMenu") : rail ? t("shell.expand") : t("shell.collapse")}
          title={mobile ? t("shell.closeMenu") : rail ? t("shell.expand") : t("shell.collapse")}
        >
          <Icon name={mobile ? "close" : "sidebar"} />
        </button>
      </div>

      {!admin && (
        <div className="sb-actions">
          <button
            className={`sb-item new ${homeActive ? "active" : ""}`}
            onClick={() => navigate("/")}
            aria-label={t("chat.newChat")}
            title={t("shell.newChatHint", { keys: shortcut("O", true) })}
          >
            <Icon name="newChat" />
            <span className="sb-label">{t("chat.newChat")}</span>
          </button>
          {rail ? (
            <button className="sb-item" onClick={onSearch} aria-label={t("shell.search")} title={t("shell.searchHint", { keys: shortcut("K", false) })}>
              <Icon name="search" />
            </button>
          ) : (
            <label className="sb-search">
              <Icon name="search" />
              <input
                ref={search}
                type="search"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={t("shell.search")}
                aria-label={t("shell.search")}
                title={t("shell.searchHint", { keys: shortcut("K", false) })}
              />
            </label>
          )}
        </div>
      )}

      <div className="sb-scroll">
        {/* On phones the pages live in the side bubble instead. */}
        {!mobile && (
          <nav className="sb-nav" aria-label={t("shell.pages")}>
            {pages.map((i) => (
              <NavLink key={i.to} to={i.to} end={i.to === "/" || i.to === "/admin"} className="sb-item" title={t(i.key)}>
                <Icon name={i.icon} />
                <span className="sb-label">{t(i.key)}</span>
              </NavLink>
            ))}
          </nav>
        )}
        {!admin && !rail && <History query={query} />}
      </div>

      <Account rail={rail} />
    </aside>
  );
}

const BUBBLE_KEY = "tracen.navBubble";
const BUBBLE = 52;

interface BubbleSpot {
  side: "left" | "right";
  /** Height as a share of the window, so a rotation keeps the bubble on screen. */
  y: number;
}

function loadSpot(): BubbleSpot {
  try {
    const v = JSON.parse(localStorage.getItem(BUBBLE_KEY) ?? "null") as BubbleSpot | null;
    if (v && (v.side === "left" || v.side === "right") && typeof v.y === "number") return v;
  } catch {
    /* storage blocked or a bad value */
  }
  return { side: "right", y: 0.66 };
}

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

/** Phones: the page links sit in a bubble on the screen edge. Drag it along either side; tap to open. */
function NavBubble({ items }: { items: NavItem[] }) {
  const { t } = useTranslation();
  const { pathname } = useLocation();
  const [spot, setSpot] = useState(loadSpot);
  const [drag, setDrag] = useState<{ x: number; y: number } | null>(null);
  const [open, setOpen] = useState(false);
  const [typing, setTyping] = useState(false);
  const [menuTop, setMenuTop] = useState(0);
  const press = useRef<{ x: number; y: number; moved: boolean } | null>(null);
  const dragged = useRef(false);
  const menu = useRef<HTMLElement>(null);

  useEffect(() => setOpen(false), [pathname]);

  // The spot is a share of the window height, so a rotation or resize moves it with the window.
  const [, setHeight] = useState(window.innerHeight);
  useEffect(() => {
    const on = () => setHeight(window.innerHeight);
    window.addEventListener("resize", on);
    return () => window.removeEventListener("resize", on);
  }, []);

  // Step aside while the keyboard is up, so the bubble never covers what is being typed.
  useEffect(() => {
    const check = () => {
      const el = document.activeElement;
      setTyping(!!el && (el.tagName === "TEXTAREA" || (el.tagName === "INPUT" && (el as HTMLInputElement).type !== "checkbox")));
    };
    const later = () => window.setTimeout(check, 0);
    document.addEventListener("focusin", check);
    document.addEventListener("focusout", later);
    return () => {
      document.removeEventListener("focusin", check);
      document.removeEventListener("focusout", later);
    };
  }, []);

  useEffect(() => {
    if (!open) return;
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("keydown", esc);
    return () => document.removeEventListener("keydown", esc);
  }, [open]);

  const centreY = spot.y * window.innerHeight;
  useLayoutEffect(() => {
    if (!open || !menu.current) return;
    const h = menu.current.offsetHeight;
    setMenuTop(clamp(centreY - h / 2, 12, window.innerHeight - h - 12));
  }, [open, centreY]);

  const down = (e: PointerEvent<HTMLButtonElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    press.current = { x: e.clientX, y: e.clientY, moved: false };
  };
  const move = (e: PointerEvent<HTMLButtonElement>) => {
    const p = press.current;
    if (!p) return;
    if (!p.moved && Math.hypot(e.clientX - p.x, e.clientY - p.y) > 6) p.moved = true;
    if (p.moved) setDrag({ x: e.clientX, y: e.clientY });
  };
  const up = (e: PointerEvent<HTMLButtonElement>) => {
    const p = press.current;
    press.current = null;
    if (!p?.moved) return;
    dragged.current = true;
    const next: BubbleSpot = { side: e.clientX < window.innerWidth / 2 ? "left" : "right", y: clamp(e.clientY / window.innerHeight, 0.1, 0.86) };
    setSpot(next);
    setDrag(null);
    try {
      localStorage.setItem(BUBBLE_KEY, JSON.stringify(next));
    } catch {
      /* storage blocked */
    }
  };
  const cancel = () => {
    press.current = null;
    setDrag(null);
  };

  const style = drag
    ? { left: drag.x - BUBBLE / 2, top: drag.y - BUBBLE / 2 }
    : { top: centreY - BUBBLE / 2, [spot.side]: 12 };

  return (
    <>
      {open && <div className="bubble-scrim" onClick={() => setOpen(false)} />}
      {open && (
        <nav ref={menu} className={`bubble-menu ${spot.side}`} style={{ top: menuTop }} aria-label={t("shell.pages")}>
          {items.map((i) => (
            <NavLink key={i.to} to={i.to} end={i.to === "/" || i.to === "/admin"} className={({ isActive }) => `bubble-item ${isActive || (i.to === "/" && pathname.startsWith("/c/")) ? "active" : ""}`}>
              <Icon name={i.icon} />
              <span>{t(i.key)}</span>
            </NavLink>
          ))}
        </nav>
      )}
      <button
        className={`nav-bubble ${drag ? "dragging" : ""} ${open ? "open" : ""} ${typing && !open ? "away" : ""} ${spot.side}`}
        style={style}
        aria-expanded={open}
        aria-label={open ? t("shell.bubbleClose") : t("shell.bubble")}
        title={open ? t("shell.bubbleClose") : t("shell.bubble")}
        onPointerDown={down}
        onPointerMove={move}
        onPointerUp={up}
        onPointerCancel={cancel}
        onClick={() => {
          // A drag ends with a click; only a tap opens the menu.
          if (dragged.current) dragged.current = false;
          else setOpen((o) => !o);
        }}
      >
        <Icon name={open ? "close" : "grid"} />
      </button>
    </>
  );
}

export function Layout({ admin = false, chat = false, children }: { admin?: boolean; chat?: boolean; children: ReactNode }) {
  const { t } = useTranslation();
  const { user } = useAuth();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const mobile = useMediaQuery(MOBILE_QUERY);
  const [rail, setRail] = useState(loadRail);
  const [drawer, setDrawer] = useState(false);
  const search = useRef<HTMLInputElement>(null);
  const items = navItems(admin, user?.role);

  useEffect(() => setDrawer(false), [pathname, mobile]);

  const toggleRail = useCallback(() => {
    setRail((r) => !r);
    saveRail(!rail);
  }, [rail]);

  const focusSearch = useCallback(() => {
    if (mobile) setDrawer(true);
    else if (rail) {
      setRail(false);
      saveRail(false);
    }
    // After the sidebar has opened and rendered the field.
    window.setTimeout(() => search.current?.focus(), 60);
  }, [mobile, rail]);

  // Shortcuts as in desktop AI assistants: new conversation, search conversations, close the drawer.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setDrawer(false);
      if (admin || !(e.ctrlKey || e.metaKey) || e.altKey) return;
      const key = e.key.toLowerCase();
      if (e.shiftKey && key === "o") {
        e.preventDefault();
        navigate("/");
      } else if (!e.shiftKey && key === "k") {
        e.preventDefault();
        focusSearch();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [admin, navigate, focusSearch]);

  const shell = useMemo(() => ({ mobile, openDrawer: () => setDrawer(true) }), [mobile]);
  const here = [...items, PROFILE].find((i) => i.to === pathname);

  return (
    <ShellContext.Provider value={shell}>
      <div className={`app ${rail && !mobile ? "rail" : ""} ${admin ? "admin" : ""}`}>
        <Sidebar
          admin={admin}
          items={items}
          rail={rail && !mobile}
          mobile={mobile}
          open={drawer}
          onRail={toggleRail}
          onClose={() => setDrawer(false)}
          search={search}
          onSearch={focusSearch}
        />
        {mobile && drawer && <div className="scrim" onClick={() => setDrawer(false)} />}
        <div className={`app-main ${chat ? "chat" : ""}`}>
          {!chat && mobile && (
            <header className="mobilebar">
              <MenuButton />
              <span className="mobilebar-title">{here ? t(here.key) : t("app.short")}</span>
            </header>
          )}
          <main className={chat ? "page-chat" : "page"}>{children}</main>
        </div>
        {/* Tazuna on every trainer page; the chat page has her in its own column. */}
        {!admin && !chat && <Concierge />}
        {mobile && <NavBubble items={items} />}
      </div>
    </ShellContext.Provider>
  );
}

export function SourceBadge({ type }: { type: string }) {
  const { t } = useTranslation();
  return <span className={`badge ${type}`}>{t(`source.${type}`)}</span>;
}
