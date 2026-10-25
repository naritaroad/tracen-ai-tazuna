import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api";
import { errorMessage } from "../i18n/errors";

interface UserRow {
  id: string;
  username: string;
  display_name: string;
  role: string;
  created_at: string;
  disabled: boolean;
}

export function AdminUsers() {
  const { t } = useTranslation();
  const [users, setUsers] = useState<UserRow[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api.get<UserRow[]>("/admin/users").then(setUsers).catch((e) => setError(errorMessage(e)));
  }, []);
  return (
    <div className="stack">
      <h1>{t("admin.usersTitle")}</h1>
      {error && <p className="error">{t("common.error", { message: error })}</p>}
      <div className="card table-wrap">
        <table>
          <thead>
            <tr>
              <th>{t("login.username")}</th>
              <th />
              <th>{t("admin.role")}</th>
            </tr>
          </thead>
          <tbody>
            {users.map((u) => (
              <tr key={u.id}>
                <td>
                  <code>{u.username}</code>
                </td>
                <td>{u.display_name}</td>
                <td>
                  <span className="badge">{t(`role.${u.role}`, { defaultValue: u.role })}</span>
                  {u.disabled && <span className="badge bad">{t("admin.disabled")}</span>}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
