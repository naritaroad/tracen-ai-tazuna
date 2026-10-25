// 將 VRM 轉成 base64 內嵌腳本，讓 index.html 以 file:// 直接開啟也能載入模型（僅供本機測試，勿上傳）
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const src = resolve(here, '../../../assets/characters/9001_hayakawa_tazuna/vrm/HayakawaTazuna.vrm');
const out = resolve(here, '../dist/vrm-data.js');
const b64 = readFileSync(src).toString('base64');
writeFileSync(out, `window.__VRM_B64__=${JSON.stringify(b64)};\n`);
console.log(`embedded ${src} -> ${out} (${(b64.length / 1048576).toFixed(2)} MB)`);

// 動畫：animations.json + 各 .vrma 內嵌成 vrma-data.js（file:// 模式用）
import { existsSync } from 'node:fs';
const animDir = resolve(here, '../../../assets/characters/9001_hayakawa_tazuna/animation');
const manPath = resolve(animDir, 'animations.json');
if (existsSync(manPath)) {
  const manifest = JSON.parse(readFileSync(manPath, 'utf8'));
  const data = {};
  for (const c of manifest.clips) data[c.id] = readFileSync(resolve(animDir, c.file)).toString('base64');
  writeFileSync(resolve(here, '../dist/vrma-data.js'), `window.__VRMA_PACK__=${JSON.stringify({ manifest, data })};\n`);
  console.log(`embedded ${manifest.clips.length} VRMA clips`);
}
