# 背景場景暫存區（`_staging/scene/`）

日期：2026-10-25。對話頁（首頁）3D 角色窗的背景：遊戲特寫過場 1053 號的學園走廊。

## 內容

| 檔案 | 說明 | 進版本控制 |
|---|---|---|
| `scene.json` | 場景清單：模型檔名、相對角色的位置與轉向、是否不受光（`unlit`）、霧 | 是 |
| `build_scene.py` | 由本機 PMX 建出 `corridor.glb` 的 Blender 腳本 | 是 |
| `corridor.glb` | 網站載入的場景（約 780 KB，貼圖為 WebP） | **否**（遊戲衍生資料） |
| `corridor_build.blend` | 建置時存下的 Blender 檔，方便檢查 | **否** |

## 來源與加工

- 來源：`vrm/scene/pfb_env_cutin1053_00_00_corridor (UmaContainerProp).pmx` 與 `Texture2D/` 的三張貼圖集（`base00`、`alpha00`、`patramp00`），只留本機。
- 匯出的 PMX **沒有材質表**（0 個材質），MMD Tools 匯入時會把所有面清掉。`build_scene.py` 改用 MMD Tools 的 PMX 讀取器直接建網格，0.08 比例（走廊寬約 4 m、高 3.9 m、長 37 m），再把每個網格島分配給 UV 所落位置不是黑色、也不是透明的貼圖集；其餘用 `base00`。結果：4480 面用 `base00`、318 面用 `patramp00`（火災警鈴等小物件）、2 面用 `alpha00`。
- 重新產生（不需開啟介面）：

```bash
"C:/Program Files/Blender Foundation/Blender 5.1/blender.exe" -b --python _staging/scene/build_scene.py
```

## 網站載入

- 在 `.env` 設 `AVATAR_SCENE_DIR=_staging/scene`（另需 `AVATAR_ASSET_DIR`）。角色清單檔 `GET /api/v1/avatar/characters/{id}` 會多出 `scene`，模型由 `GET /avatar-assets/{id}/scene/corridor.glb` 提供。
- 只有對話頁的角色窗顯示背景；其他頁面的諮詢窗維持透明。貼圖以不受光材質顯示（遊戲背景的光影是畫在貼圖上的），稍微壓暗並加淡紫色霧，讓角色浮出來。
- 位置調整改 `scene.json` 的 `position`（公尺，three.js 座標，角色在原點面向 +Z）與 `rotation_y_deg`，重新整理頁面即可，不必重啟 API。

## 權利

遊戲衍生資料，著作權屬 Cygames, Inc.，只做本機研究使用，**公開部署不得設定 `AVATAR_SCENE_DIR`**（見 [`docs/character/character-assets.md`](../../docs/character/character-assets.md) 的「權利與版本控制規則」）。
