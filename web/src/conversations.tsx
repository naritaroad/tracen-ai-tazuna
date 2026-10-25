// The trainer's conversation list. The sidebar shows it on every trainer page, and the chat page
// reloads it after each reply, so both read one copy.

import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

import { api, type Conversation } from "./api";
import { useAuth } from "./auth";

interface ConversationsValue {
  list: Conversation[];
  reload: () => void;
  /** Pins a conversation to the top of the list, or unpins it. */
  pin: (id: string, pinned: boolean) => Promise<void>;
}

const ConversationsContext = createContext<ConversationsValue>({ list: [], reload: () => undefined, pin: async () => undefined });

export function ConversationsProvider({ children }: { children: ReactNode }) {
  const { user } = useAuth();
  const [list, setList] = useState<Conversation[]>([]);
  const reload = useCallback(() => {
    // A failed refresh keeps the last list; the chat page reports its own errors.
    api.get<Conversation[]>("/conversations").then(setList).catch(() => undefined);
  }, []);

  useEffect(() => {
    if (user) reload();
    else setList([]);
  }, [user, reload]);

  const pin = useCallback(
    async (id: string, pinned: boolean) => {
      // Shown at once; the server's order (pinned first) arrives with the reload.
      setList((l) => l.map((c) => (c.id === id ? { ...c, pinned } : c)));
      try {
        await api.patch(`/conversations/${id}`, { pinned });
      } finally {
        reload();
      }
    },
    [reload],
  );

  const value = useMemo(() => ({ list, reload, pin }), [list, reload, pin]);
  return <ConversationsContext.Provider value={value}>{children}</ConversationsContext.Provider>;
}

export const useConversations = () => useContext(ConversationsContext);
