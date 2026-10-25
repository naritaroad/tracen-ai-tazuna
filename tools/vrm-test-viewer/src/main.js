// VRM 測試窗口：載入、驗收、表情 / 視線 / 姿勢 / 彈簧骨互動測試
import * as THREE from 'three';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import {
  VRMLoaderPlugin, VRMUtils, VRMExpressionPresetName,
  VRMSpringBoneColliderHelper, VRMSpringBoneJointHelper, VRMLookAtHelper,
} from '@pixiv/three-vrm';
import { VRMAnimationLoaderPlugin, createVRMAnimationClip, VRMLookAtQuaternionProxy } from '@pixiv/three-vrm-animation';

const MODEL_URL = '../../assets/characters/9001_hayakawa_tazuna/vrm/HayakawaTazuna.vrm';
const ANIM_DIR = '../../assets/characters/9001_hayakawa_tazuna/animation/';
const REF_VIDEO_DIR = '../../example_pose/9001_Hayakawa%20Tazuna/exsample_source_vedio/';
const $ = (s) => document.querySelector(s);
const log = (msg) => { const el = $('#log'); el.textContent += msg + '\n'; el.scrollTop = el.scrollHeight; console.log('[viewer]', msg); };

// ---------- 場景 ----------
const canvas = $('#c');
const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, preserveDrawingBuffer: true });
renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
const scene = new THREE.Scene();
scene.background = new THREE.Color(0xdfe7ee);
const camera = new THREE.PerspectiveCamera(30, 1, 0.05, 50);
camera.position.set(0, 1.35, 3.2);
const controls = new OrbitControls(camera, canvas);
controls.target.set(0, 1.05, 0);
controls.enableDamping = true;
controls.update();
const light = new THREE.DirectionalLight(0xffffff, Math.PI);
light.position.set(1, 1.5, 1.8);
scene.add(light);
scene.add(new THREE.AmbientLight(0xffffff, 0.6));
const grid = new THREE.GridHelper(4, 16, 0x8899aa, 0xc0ccd8);
scene.add(grid);

function resize() {
  const w = canvas.clientWidth, h = canvas.clientHeight;
  renderer.setSize(w, h, false);
  camera.aspect = w / h; camera.updateProjectionMatrix();
}
window.addEventListener('resize', resize);

// ---------- 狀態 ----------
let vrm = null;
let helpers = [];
const state = {
  autoBlink: true, lipSync: false, lookMouse: true, pose: 'idle', spin: 0, spinT: 0, jumpT: 0,
  breathing: true, blinkTimer: 0, lipT: 0, manual: {},
};
// 動畫播放
const anim = { mixer: null, clips: {}, meta: {}, current: null, action: null, fade: 0.3, speed: 1, hasFace: false, queue: null };
const lookTarget = new THREE.Object3D();
scene.add(lookTarget);
lookTarget.position.set(0, 1.45, 2);

// ---------- 載入 ----------
const loader = new GLTFLoader();
loader.register((parser) => new VRMLoaderPlugin(parser));
const animLoader = new GLTFLoader();
animLoader.register((parser) => new VRMAnimationLoaderPlugin(parser));

function b64ToBuf(b64) {
  const bin = atob(b64); const u8 = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
  return u8.buffer;
}

async function loadAnimations() {
  let manifest = null;
  const buffers = {};
  if (location.protocol.startsWith('http')) {
    try {
      const r = await fetch(ANIM_DIR + 'animations.json');
      if (r.ok) {
        manifest = await r.json();
        for (const c of manifest.clips) buffers[c.id] = await (await fetch(ANIM_DIR + c.file)).arrayBuffer();
        log(`動畫：由伺服器載入 ${manifest.clips.length} 段`);
      }
    } catch (e) { manifest = null; }
  }
  if (!manifest && window.__VRMA_PACK__) {
    manifest = window.__VRMA_PACK__.manifest;
    for (const c of manifest.clips) buffers[c.id] = b64ToBuf(window.__VRMA_PACK__.data[c.id]);
    log(`動畫：由內嵌資料載入 ${manifest.clips.length} 段`);
  }
  if (!manifest) { log('未找到動畫資料（animations.json / vrma-data.js）'); return; }
  anim.mixer = new THREE.AnimationMixer(vrm.scene);
  anim.mixer.addEventListener('finished', (e) => onClipFinished(e));
  anim.clips = {}; anim.meta = {};
  for (const c of manifest.clips) {
    const gltf = await animLoader.parseAsync(buffers[c.id], '');
    const va = gltf.userData.vrmAnimations?.[0];
    if (!va) { log('⚠ 無法解析 ' + c.file); continue; }
    const clip = createVRMAnimationClip(va, vrm);
    clip.name = c.id;
    anim.clips[c.id] = clip; anim.meta[c.id] = c;
  }
  buildAnimUI(manifest);
}

function playClip(id, opts = {}) {
  const clip = anim.clips[id]; if (!clip) return;
  const meta = anim.meta[id];
  const act = anim.mixer.clipAction(clip);
  act.reset();
  act.setLoop(meta.loop ? THREE.LoopRepeat : THREE.LoopOnce, Infinity);
  act.clampWhenFinished = !meta.loop;
  act.setEffectiveTimeScale(anim.speed);
  act.enabled = true;
  if (anim.action && anim.action !== act && !opts.cut) {
    act.setEffectiveWeight(1);
    anim.action.crossFadeTo(act, anim.fade, false);
    act.play();
  } else {
    if (anim.action && anim.action !== act) anim.action.stop();
    act.play();
  }
  anim.action = act; anim.current = id;
  anim.hasFace = !!meta.face;
  anim.queue = meta.next || null;
  document.querySelectorAll('[data-clip]').forEach((b) => b.classList.toggle('on', b.dataset.clip === id));
  syncRefVideo(true);
  log(`播放：${meta.label}（${clip.duration.toFixed(2)} s${meta.loop ? '・循環' : ''}）`);
}

function onClipFinished(e) {
  if (e.action !== anim.action) return;
  const next = anim.queue;
  if (next && anim.clips[next]) playClip(next);
}

function stopClip() {
  if (anim.action) { anim.action.fadeOut(anim.fade); }
  setTimeout(() => { anim.mixer && anim.mixer.stopAllAction(); anim.action = null; anim.current = null; anim.hasFace = false;
    document.querySelectorAll('[data-clip]').forEach((b) => b.classList.remove('on')); }, anim.fade * 1000);
}

function syncRefVideo(force) {
  const v = $('#refVideo'); if (!v || !$('#refShow').checked || !anim.action) return;
  const meta = anim.meta[anim.current];
  if (!meta.source) { v.pause(); return; }
  const want = meta.source.start + (meta.source.reverse ? (anim.action.getClip().duration - anim.action.time) : anim.action.time) * (meta.source.rate || 1);
  const src = REF_VIDEO_DIR + meta.source.video + '.mp4';
  if (!v.src.endsWith(meta.source.video + '.mp4')) { v.src = src; v.muted = !$('#refAudio').checked; }
  if (force || Math.abs(v.currentTime - want) > 0.08) { try { v.currentTime = want; } catch (err) { /* not ready */ } }
  if (meta.source.reverse) { v.pause(); } else if (v.paused && anim.speed === 1) { v.play().catch(() => {}); }
  v.playbackRate = anim.speed;
}

function buildAnimUI(manifest) {
  const box = $('#clips'); box.innerHTML = '';
  for (const c of manifest.clips) {
    const b = document.createElement('button');
    b.dataset.clip = c.id; b.title = `${c.description}\n情緒：${c.emotion}\n情境：${c.usage}`;
    b.textContent = c.label + (c.loop ? ' ⟳' : '');
    b.addEventListener('click', () => playClip(c.id));
    box.appendChild(b);
  }
}

async function getModelBuffer() {
  if (location.protocol.startsWith('http')) {
    try {
      const r = await fetch(MODEL_URL);
      if (r.ok) { log('由伺服器載入：' + MODEL_URL); return { buf: await r.arrayBuffer(), src: 'http' }; }
    } catch (e) { /* fall through */ }
  }
  if (window.__VRM_B64__) {
    const bin = atob(window.__VRM_B64__);
    const u8 = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
    log('由內嵌資料 vrm-data.js 載入（file:// 模式）');
    return { buf: u8.buffer, src: 'embedded' };
  }
  throw new Error('找不到模型：請用本機伺服器開啟，或把 .vrm 拖進視窗');
}

async function loadFromBuffer(buf, label) {
  const t0 = performance.now();
  const gltf = await loader.parseAsync(buf, '');
  const v = gltf.userData.vrm;
  if (!v) throw new Error('檔案不是 VRM');
  if (vrm) { scene.remove(vrm.scene); VRMUtils.deepDispose(vrm.scene); helpers.forEach((h) => scene.remove(h)); helpers = []; }
  VRMUtils.rotateVRM0(v);
  VRMUtils.combineSkeletons(gltf.scene);
  v.scene.traverse((o) => { o.frustumCulled = false; });
  vrm = v;
  scene.add(vrm.scene);
  vrm.lookAt.target = lookTarget;
  const ms = (performance.now() - t0).toFixed(0);
  log(`載入完成（${label}），解析 ${ms} ms，大小 ${(buf.byteLength / 1048576).toFixed(2)} MB`);
  window.__VRM__ = vrm; window.__THREE__ = THREE;
  const proxy = new VRMLookAtQuaternionProxy(vrm.lookAt); proxy.name = 'VRMLookAtQuaternionProxy'; vrm.scene.add(proxy);
  buildHelpers();
  buildExpressionUI();
  showInfo(buf.byteLength);
  runSelfTest(buf.byteLength);
  await loadAnimations();
  if (Object.keys(anim.clips).length) runAnimTest();
}

// ---------- 資訊 ----------
function showInfo(bytes) {
  const meta = vrm.meta;
  let tris = 0, prims = 0, morphs = 0, mtoon = 0, mats = new Set();
  vrm.scene.traverse((o) => {
    if (o.isMesh) {
      prims++;
      const g = o.geometry; tris += (g.index ? g.index.count : g.attributes.position.count) / 3;
      morphs = Math.max(morphs, Object.keys(g.morphAttributes).length ? g.morphAttributes.position.length : 0);
      (Array.isArray(o.material) ? o.material : [o.material]).forEach((m) => { mats.add(m); if (m.isMToonMaterial) mtoon++; });
    }
  });
  const sb = vrm.springBoneManager;
  const rows = [
    ['名稱', meta?.name], ['VRM 規格', meta?.metaVersion === '1' ? 'VRM 1.0' : 'VRM 0.x'],
    ['作者', (meta?.authors || []).join(' / ')], ['檔案大小', (bytes / 1048576).toFixed(2) + ' MB'],
    ['三角面', Math.round(tris).toLocaleString()], ['網格 primitive', prims],
    ['材質（MToon / 全部）', `${[...mats].filter((m) => m.isMToonMaterial).length} / ${mats.size}（含描邊 pass）`],
    ['人形骨骼', Object.keys(vrm.humanoid.humanBones).length],
    ['表情', vrm.expressionManager.expressions.length], ['形態鍵', morphs],
    ['彈簧骨 joints / colliders', `${sb.joints.size} / ${sb.colliders.length}`],
    ['視線', (() => { const ty = vrm.lookAt?.applier?.type ?? vrm.lookAt?.applier?.constructor?.type; return ty === 'bone' ? '骨骼驅動（bone）' : ty === 'expression' ? '表情驅動（expression）' : (ty || '-'); })()],
  ];
  $('#info').innerHTML = rows.map(([k, v]) => `<div><span>${k}</span><b>${v ?? '-'}</b></div>`).join('');
}

// ---------- 表情 UI ----------
function buildExpressionUI() {
  const box = $('#expr'); box.innerHTML = '';
  const em = vrm.expressionManager;
  const presets = new Set(Object.values(VRMExpressionPresetName));
  const names = em.expressions.map((e) => e.expressionName);
  const ordered = [...names.filter((n) => presets.has(n)), ...names.filter((n) => !presets.has(n))];
  for (const n of ordered) {
    if (n.startsWith('look')) continue;
    const row = document.createElement('label');
    row.className = 'slider' + (presets.has(n) ? '' : ' custom');
    row.innerHTML = `<span>${n}</span><input type="range" min="0" max="1" step="0.01" value="0">`;
    const inp = row.querySelector('input');
    inp.addEventListener('input', () => { state.manual[n] = parseFloat(inp.value); });
    box.appendChild(row);
  }
}
function resetExpressions() {
  state.manual = {};
  document.querySelectorAll('#expr input').forEach((i) => { i.value = 0; });
}

// ---------- 輔助顯示 ----------
function buildHelpers() {
  const sb = vrm.springBoneManager;
  sb.colliders.forEach((c) => helpers.push(new VRMSpringBoneColliderHelper(c)));
  sb.joints.forEach((j) => helpers.push(new VRMSpringBoneJointHelper(j)));
  helpers.push(new VRMLookAtHelper(vrm.lookAt));
  helpers.forEach((h) => { h.visible = $('#helpers').checked; scene.add(h); });
}

// ---------- 姿勢 ----------
function bone(name) { return vrm.humanoid.getNormalizedBoneNode(name); }
function applyPose(t) {
  const names = ['spine', 'chest', 'upperChest', 'neck', 'head', 'hips',
    'leftUpperArm', 'rightUpperArm', 'leftLowerArm', 'rightLowerArm', 'leftHand', 'rightHand',
    'leftUpperLeg', 'rightUpperLeg', 'leftLowerLeg', 'rightLowerLeg'];
  names.forEach((n) => { const b = bone(n); if (b) b.rotation.set(0, 0, 0); });
  const breath = state.breathing ? Math.sin(t * 1.6) * 0.02 : 0;
  const p = state.pose;
  if (p === 'tpose') return;
  if (p === 'idle' || p === 'bow' || p === 'wave') {
    bone('leftUpperArm').rotation.z = -1.22; bone('rightUpperArm').rotation.z = 1.22;
    bone('leftLowerArm').rotation.z = -0.12; bone('rightLowerArm').rotation.z = 0.12;
    bone('chest').rotation.x = breath; bone('neck').rotation.x = -breath * 0.5;
  }
  if (p === 'bow') {
    const k = (Math.sin(t * 1.2) * 0.5 + 0.5);
    bone('spine').rotation.x = 0.32 * k; bone('chest').rotation.x = 0.18 * k; bone('neck').rotation.x = 0.15 * k;
    bone('leftUpperArm').rotation.y = 0.35; bone('rightUpperArm').rotation.y = -0.35;
    bone('leftLowerArm').rotation.y = 0.9; bone('rightLowerArm').rotation.y = -0.9;
  }
  if (p === 'wave') {
    bone('rightUpperArm').rotation.z = -1.1; bone('rightUpperArm').rotation.x = 0.1;
    bone('rightLowerArm').rotation.z = -1.0 + Math.sin(t * 7) * 0.35;
    bone('head').rotation.z = 0.08;
  }
  if (p === 'range') {
    bone('leftUpperArm').rotation.z = -1.2 + Math.sin(t * 2) * 1.0;
    bone('rightUpperArm').rotation.x = -1.2 + Math.sin(t * 2.3) * 0.8;
    bone('rightLowerArm').rotation.y = -1.5;
    bone('leftUpperLeg').rotation.x = -1.2 * (Math.sin(t * 1.7) * 0.5 + 0.5);
    bone('leftLowerLeg').rotation.x = 1.8 * (Math.sin(t * 1.7) * 0.5 + 0.5);
    bone('spine').rotation.y = Math.sin(t) * 0.4;
    bone('head').rotation.y = Math.sin(t * 0.8) * 0.6;
  }
}

// ---------- 主迴圈 ----------
const timer = new THREE.Timer();
const mouse = new THREE.Vector2();
canvas.addEventListener('pointermove', (e) => {
  const r = canvas.getBoundingClientRect();
  mouse.set(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
});
let fpsAcc = 0, fpsN = 0;
function tick(now) {
  requestAnimationFrame(tick);
  timer.update(now);
  const dt = Math.min(timer.getDelta(), 1 / 20);
  const t = timer.getElapsed();
  controls.update();
  if (vrm) {
    // 視線
    if (state.lookMouse) {
      const v = new THREE.Vector3(mouse.x, mouse.y, 0.5).unproject(camera);
      const dir = v.sub(camera.position).normalize();
      lookTarget.position.copy(camera.position).addScaledVector(dir, 2.5);
    } else lookTarget.position.copy(camera.position);
    // 轉身 / 跳躍（彈簧骨測試）
    if (state.spinT > 0) { state.spinT -= dt; vrm.scene.rotation.y = Math.sin((1 - state.spinT / 1.2) * Math.PI) * 1.2; if (state.spinT <= 0) vrm.scene.rotation.y = 0; }
    if (state.jumpT > 0) { state.jumpT -= dt; vrm.scene.position.y = Math.max(0, Math.sin((1 - state.jumpT / 0.6) * Math.PI)) * 0.25; if (state.jumpT <= 0) vrm.scene.position.y = 0; }
    if (!anim.action) applyPose(t);
    // 表情
    const em = vrm.expressionManager;
    em.expressions.forEach((e) => em.setValue(e.expressionName, 0));
    for (const [k, v] of Object.entries(state.manual)) em.setValue(k, v);
    if (state.autoBlink && !anim.hasFace) {
      state.blinkTimer -= dt;
      if (state.blinkTimer < 0) state.blinkTimer = 2.5 + Math.random() * 2.5;
      const b = state.blinkTimer < 0.15 ? Math.sin((state.blinkTimer / 0.15) * Math.PI) : 0;
      em.setValue('blink', Math.max(b, state.manual.blink || 0));
    }
    if (state.lipSync && !anim.hasFace) {
      state.lipT += dt;
      const vowels = ['aa', 'ih', 'ou', 'ee', 'oh'];
      const i = Math.floor(state.lipT * 6) % vowels.length;
      const amp = Math.abs(Math.sin(state.lipT * 6 * Math.PI)) * (0.6 + 0.4 * Math.sin(state.lipT * 1.3));
      em.setValue(vowels[i], amp);
    }
    if (anim.mixer) {
      anim.mixer.update(dt);
      syncRefVideo(false);
      if (anim.action) $('#animTime').textContent = `${anim.current}  ${anim.action.time.toFixed(2)} / ${anim.action.getClip().duration.toFixed(2)} s`;
    }
    vrm.update(dt);
  }
  renderer.render(scene, camera);
  fpsAcc += dt; fpsN++;
  if (fpsAcc > 0.5) {
    $('#perf').textContent = `${(fpsN / fpsAcc).toFixed(0)} FPS ｜ draw calls ${renderer.info.render.calls} ｜ 三角面 ${renderer.info.render.triangles.toLocaleString()}`;
    fpsAcc = 0; fpsN = 0;
  }
}

// ---------- 自動驗收 ----------
async function runSelfTest(bytes) {
  const R = [];
  const ok = (name, pass, detail = '') => R.push({ name, pass: !!pass, detail: String(detail) });
  const em = vrm.expressionManager, hum = vrm.humanoid, sb = vrm.springBoneManager;
  ok('VRM 1.0 中繼資料', vrm.meta?.metaVersion === '1' && vrm.meta?.name, vrm.meta?.name);
  const req = ['hips', 'spine', 'chest', 'neck', 'head', 'leftUpperArm', 'leftLowerArm', 'leftHand', 'rightUpperArm', 'rightLowerArm', 'rightHand',
    'leftUpperLeg', 'leftLowerLeg', 'leftFoot', 'rightUpperLeg', 'rightLowerLeg', 'rightFoot', 'leftEye', 'rightEye'];
  const missing = req.filter((n) => !hum.getRawBoneNode(n));
  ok('必要人形骨骼', missing.length === 0, missing.length ? '缺少 ' + missing.join(',') : `${Object.keys(hum.humanBones).length} 根`);
  // 身高
  vrm.scene.updateMatrixWorld(true);
  const box = new THREE.Box3().setFromObject(vrm.scene);
  const h = box.max.y - box.min.y;
  ok('身高維持原比例（約 1.67 m）', Math.abs(h - 1.672) < 0.03, h.toFixed(3) + ' m');
  // 面向 +Z（VRM 1.0）
  const lf = hum.getNormalizedBoneNode('leftUpperArm').getWorldPosition(new THREE.Vector3());
  ok('左手位於 +X（面向 +Z）', lf.x > 0, lf.x.toFixed(3));
  // 表情
  const needExpr = ['happy', 'angry', 'sad', 'relaxed', 'surprised', 'aa', 'ih', 'ou', 'ee', 'oh', 'blink', 'blinkLeft', 'blinkRight'];
  const missE = needExpr.filter((n) => !em.getExpression(n));
  ok('預設表情齊全', missE.length === 0, missE.length ? '缺少 ' + missE.join(',') : `${em.expressions.length} 個（含自訂）`);
  let face = null;
  vrm.scene.traverse((o) => { if (o.isMesh && o.morphTargetInfluences && o.morphTargetInfluences.length) face = face || o; });
  const exprWorks = [];
  for (const n of em.expressions.map((e) => e.expressionName).filter((n) => !n.startsWith('look') && n !== 'neutral')) {
    em.expressions.forEach((e) => em.setValue(e.expressionName, 0));
    em.setValue(n, 1); em.update();
    const s = face.morphTargetInfluences.reduce((a, b) => a + b, 0);
    exprWorks.push([n, s]);
  }
  em.expressions.forEach((e) => em.setValue(e.expressionName, 0)); em.update();
  const dead = exprWorks.filter(([, s]) => s < 0.5).map(([n]) => n);
  ok('每個表情都會驅動形態鍵', dead.length === 0, dead.length ? '無效：' + dead.join(',') : `${exprWorks.length} 個有效`);
  // 視線
  const eyeL = hum.getRawBoneNode('leftEye');
  const q0 = eyeL.quaternion.clone();
  lookTarget.position.set(1.5, 1.5, 1.0); vrm.lookAt.update(0.016);
  const ang = THREE.MathUtils.radToDeg(q0.angleTo(eyeL.quaternion));
  ok('視線（骨骼）可轉動眼球', ang > 1 && ang < 15, ang.toFixed(1) + '°');
  lookTarget.position.set(0, 1.45, 2); vrm.lookAt.update(0.016);
  // 彈簧骨
  // VRM 1.0 每條鏈的最後一個節點只作為末端，three-vrm 不建立 joint：54 節點 − 24 鏈 = 30 段
  ok('彈簧骨（24 鏈 / 30 段）', sb.joints.size === 30, `${sb.joints.size} 段`);
  ok('碰撞體', sb.colliders.length === 12, sb.colliders.length);
  // 模擬：靜置收斂 → 轉身 → 回正收斂，檢查 NaN 與殘餘抖動
  const joints = [...sb.joints];
  const pos = () => joints.map((j) => j.bone.getWorldPosition(new THREE.Vector3()));
  sb.reset();
  for (let i = 0; i < 120; i++) { vrm.scene.updateMatrixWorld(true); sb.update(1 / 60); }
  const a = pos();
  for (let i = 0; i < 30; i++) { vrm.scene.updateMatrixWorld(true); sb.update(1 / 60); }
  const b = pos();
  const residual = Math.max(...a.map((p, i) => p.distanceTo(b[i])));
  ok('靜置後無持續抖動', residual < 0.002, (residual * 1000).toFixed(2) + ' mm / 0.5 s');
  for (let i = 0; i < 40; i++) { vrm.scene.rotation.y = Math.sin(i / 40 * Math.PI) * 1.2; vrm.scene.updateMatrixWorld(true); sb.update(1 / 60); }
  const moved = pos();
  const swing = Math.max(...b.map((p, i) => p.distanceTo(moved[i])));
  vrm.scene.rotation.y = 0;
  for (let i = 0; i < 240; i++) { vrm.scene.updateMatrixWorld(true); sb.update(1 / 60); }
  const c = pos();
  const nan = c.some((p) => !Number.isFinite(p.x + p.y + p.z));
  const back = Math.max(...b.map((p, i) => p.distanceTo(c[i])));
  ok('轉身時次級動作會擺動', swing > 0.02, (swing * 100).toFixed(1) + ' cm');
  ok('回正後恢復且無 NaN', !nan && back < 0.03, (back * 100).toFixed(2) + ' cm');
  // 姿勢
  const hand = hum.getRawBoneNode('leftHand');
  const h0 = hand.getWorldPosition(new THREE.Vector3());
  hum.getNormalizedBoneNode('leftUpperArm').rotation.z = -1.22; hum.update(); vrm.scene.updateMatrixWorld(true);
  const h1 = hand.getWorldPosition(new THREE.Vector3());
  ok('正規化骨骼姿勢驅動', h1.y < h0.y - 0.3, `手部下降 ${(h0.y - h1.y).toFixed(2)} m`);
  hum.getNormalizedBoneNode('leftUpperArm').rotation.z = 0; hum.update();
  // 材質
  const mt = []; vrm.scene.traverse((o) => { if (o.isMesh) (Array.isArray(o.material) ? o.material : [o.material]).forEach((m) => mt.push(m)); });
  ok('材質皆為 MToon', mt.every((m) => m.isMToonMaterial), `${mt.filter((m) => m.isMToonMaterial).length}/${mt.length}`);
  ok('檔案大小 < 8 MB', bytes < 8 * 1048576, (bytes / 1048576).toFixed(2) + ' MB');
  sb.reset();
  const passed = R.filter((r) => r.pass).length;
  window.__TEST_RESULTS__ = { passed, total: R.length, results: R };
  $('#tests').innerHTML = `<div class="sum ${passed === R.length ? 'pass' : 'fail'}">自動驗收 ${passed} / ${R.length}</div>` +
    R.map((r) => `<div class="${r.pass ? 'pass' : 'fail'}">${r.pass ? '✔' : '✘'} ${r.name}<small>${r.detail}</small></div>`).join('');
  log(`自動驗收：${passed}/${R.length} 通過`);
}

// ---------- 動畫驗收 ----------
function runAnimTest() {
  const R = window.__TEST_RESULTS__ ? window.__TEST_RESULTS__.results.slice() : [];
  const ids = Object.keys(anim.clips);
  const handR = vrm.humanoid.getRawBoneNode('rightHand');
  let ok = 0; const detail = [];
  for (const id of ids) {
    const clip = anim.clips[id];
    const m = new THREE.AnimationMixer(vrm.scene); const a = m.clipAction(clip); a.play();
    const pos = [];
    for (const f of [0, 0.5, 1]) { m.setTime(clip.duration * f); vrm.humanoid.update(); vrm.scene.updateMatrixWorld(true); pos.push(handR.getWorldPosition(new THREE.Vector3())); }
    a.stop(); m.uncacheRoot(vrm.scene);
    const finite = pos.every((p) => Number.isFinite(p.x + p.y + p.z));
    const tracks = clip.tracks.length;
    if (finite && tracks > 10 && clip.duration > 0.2) ok++;
    detail.push(`${id}: ${clip.duration.toFixed(2)}s/${tracks}軌`);
  }
  R.push({ name: `動畫載入與取樣（${ids.length} 段）`, pass: ok === ids.length, detail: detail.join('；') });
  vrm.humanoid.resetNormalizedPose?.();
  const passed = R.filter((r) => r.pass).length;
  window.__TEST_RESULTS__ = { passed, total: R.length, results: R };
  $('#tests').innerHTML = `<div class="sum ${passed === R.length ? 'pass' : 'fail'}">自動驗收 ${passed} / ${R.length}</div>` +
    R.map((r) => `<div class="${r.pass ? 'pass' : 'fail'}">${r.pass ? '✔' : '✘'} ${r.name}<small>${r.detail}</small></div>`).join('');
  log(`動畫驗收：${ok}/${ids.length}`);
}

// ---------- UI 綁定 ----------
function bindUI() {
  $('#autoBlink').addEventListener('change', (e) => { state.autoBlink = e.target.checked; });
  $('#lipSync').addEventListener('change', (e) => { state.lipSync = e.target.checked; });
  $('#lookMouse').addEventListener('change', (e) => { state.lookMouse = e.target.checked; });
  $('#breathing').addEventListener('change', (e) => { state.breathing = e.target.checked; });
  $('#helpers').addEventListener('change', (e) => helpers.forEach((h) => { h.visible = e.target.checked; }));
  document.querySelectorAll('[data-pose]').forEach((b) => b.addEventListener('click', () => {
    state.pose = b.dataset.pose;
    document.querySelectorAll('[data-pose]').forEach((x) => x.classList.toggle('on', x === b));
  }));
  $('#spin').addEventListener('click', () => { state.spinT = 1.2; });
  $('#jump').addEventListener('click', () => { state.jumpT = 0.6; });
  $('#resetExpr').addEventListener('click', resetExpressions);
  $('#stopClip').addEventListener('click', stopClip);
  $('#fade').addEventListener('input', (e) => { anim.fade = parseFloat(e.target.value); $('#fadeV').textContent = anim.fade.toFixed(2) + ' s'; });
  $('#speed').addEventListener('change', (e) => { anim.speed = parseFloat(e.target.value); if (anim.action) anim.action.setEffectiveTimeScale(anim.speed); });
  $('#refShow').addEventListener('change', (e) => { $('#refWrap').style.display = e.target.checked ? 'block' : 'none'; if (!e.target.checked) $('#refVideo').pause(); else syncRefVideo(true); });
  $('#refAudio').addEventListener('change', (e) => { $('#refVideo').muted = !e.target.checked; });
  $('#camRef').addEventListener('click', () => { camera.position.set(-0.11, 2.30, 1.78); controls.target.set(0, 1.24, 0); camera.fov = 2 * Math.atan(724 / 2200) * 180 / Math.PI; camera.updateProjectionMatrix(); });
  $('#rerun').addEventListener('click', () => vrm && runSelfTest(window.__VRM_BYTES__ || 0));
  $('#shot').addEventListener('click', () => {
    const a = document.createElement('a'); a.href = canvas.toDataURL('image/png'); a.download = 'vrm-test.png'; a.click();
  });
  // 拖放 .vrm
  window.addEventListener('dragover', (e) => { e.preventDefault(); });
  window.addEventListener('drop', async (e) => {
    e.preventDefault();
    const f = e.dataTransfer.files[0]; if (!f) return;
    const buf = await f.arrayBuffer(); window.__VRM_BYTES__ = buf.byteLength;
    loadFromBuffer(buf, f.name).catch((err) => log('載入失敗：' + err.message));
  });
}

resize(); bindUI(); tick();
getModelBuffer()
  .then(({ buf, src }) => { window.__VRM_BYTES__ = buf.byteLength; return loadFromBuffer(buf, src); })
  .catch((err) => { log('⚠ ' + err.message); $('#tests').innerHTML = `<div class="fail">${err.message}</div>`; });
