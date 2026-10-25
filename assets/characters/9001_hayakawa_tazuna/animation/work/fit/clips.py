"""Cut the 30 fps performance into game-ready clips: loops, transitions (source + reversed), pose-matched joins."""
import json, numpy as np

FPS = 30
d = json.load(open('body30.json'))
t30 = np.array(d['t']); Q = {b: np.array(v) for b, v in d['quats'].items()}   # x,y,z,w
BONES = sorted(Q)


def idx(t):
    return int(round((t - t30[0]) * FPS))


def seg(a, b):
    return {k: Q[k][idx(a):idx(b) + 1].copy() for k in BONES}


def slerp(q0, q1, w):
    q0 = q0 / np.linalg.norm(q0); q1 = q1 / np.linalg.norm(q1)
    if np.dot(q0, q1) < 0:
        q1 = -q1
    d = np.clip(np.dot(q0, q1), -1, 1)
    if d > 0.9995:
        q = q0 + w * (q1 - q0); return q / np.linalg.norm(q)
    th = np.arccos(d)
    return (np.sin((1 - w) * th) * q0 + np.sin(w * th) * q1) / np.sin(th)


def smooth(x):
    return x * x * (3 - 2 * x)


def length(c):
    return len(c[BONES[0]])


def make_loop(a, L_sec, N_sec):
    """seamless loop: first N frames blend from the continuation (tail) into the head"""
    L = int(round(L_sec * FPS)); N = int(round(N_sec * FPS))
    S = {k: Q[k][idx(a):idx(a) + L + N] for k in BONES}
    out = {}
    for k in BONES:
        o = S[k][:L].copy()
        for i in range(N):
            w = smooth((i + 1) / (N + 1))
            o[i] = slerp(S[k][L + i], S[k][i], w)
        out[k] = o
    return out


def reverse(c, stretch=1.0):
    out = {}
    for k in BONES:
        r = c[k][::-1]
        n = len(r); m = int(round(n * stretch))
        src = np.linspace(0, n - 1, m)
        res = []
        for s in src:
            i0 = int(np.floor(s)); i1 = min(i0 + 1, n - 1); res.append(slerp(r[i0], r[i1], s - i0))
        out[k] = np.array(res)
    return out


def match_start(c, pose, N_sec):
    N = int(round(N_sec * FPS)); out = {}
    for k in BONES:
        o = c[k].copy(); delta0 = pose[k]
        for i in range(min(N, len(o))):
            w = smooth(1 - i / N)       # 1 at start -> 0
            o[i] = slerp(o[i], delta0, w) if w > 0 else o[i]
        out[k] = o
    return out


def match_end(c, pose, N_sec):
    N = int(round(N_sec * FPS)); out = {}
    for k in BONES:
        o = c[k].copy(); n = len(o)
        for j in range(min(N, n)):
            i = n - 1 - j; w = smooth(1 - j / N)
            o[i] = slerp(o[i], pose[k], w)
        out[k] = o
    return out


def first(c):
    return {k: c[k][0] for k in BONES}


def last(c):
    return {k: c[k][-1] for k in BONES}


# ---- head pitch retarget: the source camera sat ~25 deg above her eye line; web cameras sit at eye level ----
from scipy.spatial.transform import Rotation as Rot
def mean_pitch(b, a, z):
    q = Q[b][idx(a):idx(z)]
    return float(np.mean(Rot.from_quat(q).as_euler('XYZ')[:, 0]))
net = mean_pitch('neck', 1.0, 12.2) + mean_pitch('head', 1.0, 12.2)
COMP = -0.7 * net                       # positive = tilt down (forward)
print('idle net head+neck pitch %.1f deg -> compensation %.1f deg' % (np.degrees(net), np.degrees(COMP)))
CORR = {'neck': Rot.from_rotvec([0.4 * COMP, 0, 0]), 'head': Rot.from_rotvec([0.6 * COMP, 0, 0])}
def retarget_head(c):
    out = dict(c)
    for b, R in CORR.items():
        out[b] = (R * Rot.from_quat(c[b])).as_quat()
    return out

clips = {}
clips['idle_polite'] = make_loop(2.5, 7.0, 0.8)
clips['chest_hold'] = make_loop(13.9, 5.2, 0.6)
clips['chin_hold'] = make_loop(31.2, 2.8, 0.9)
IDLE0 = first(clips['idle_polite']); CHEST0 = first(clips['chest_hold']); CHIN0 = first(clips['chin_hold'])

clips['idle_to_chest'] = match_end(match_start(seg(12.2, 13.5), IDLE0, 0.25), CHEST0, 0.35)
clips['chest_to_idle'] = match_end(match_start(reverse(seg(12.2, 13.5), 1.15), CHEST0, 0.3), IDLE0, 0.35)
clips['chest_to_chin'] = match_end(match_start(seg(30.05, 31.0), CHEST0, 0.25), CHIN0, 0.3)
clips['chin_to_idle'] = match_end(match_start(seg(34.05, 35.3), CHIN0, 0.25), IDLE0, 0.35)
clips['idle_to_chin'] = match_end(match_start(reverse(seg(34.05, 35.3), 1.0), IDLE0, 0.3), CHIN0, 0.35)
for k in list(clips):
    clips[k] = retarget_head(clips[k])
clips['talk_full_qQ40'] = seg(0.3, 42.0)          # keeps the original head angles (for side-by-side with the source)

META = {
    'idle_polite': dict(label='禮貌待機', loop=True, emotion='平靜、親切', usage='待命、聆聽使用者輸入', source=dict(video='qQ40c6kgV8M', start=2.5),
                        description='雙手輕疊於下腹前、手肘微張，頭部隨呼吸細微擺動；7.0 秒無縫循環。', interrupt='任意時點'),
    'idle_to_chest': dict(label='抬手至胸前', loop=False, next='chest_hold', emotion='真誠、自我表白', usage='「我也能理解」「請相信我」等表達心意的回答開頭',
                          source=dict(video='qQ40c6kgV8M', start=12.2), description='右手由交握處抬到胸口，左手同時放下垂於身側，頭向右微傾。',
                          interrupt='結束後自動進入胸前停留'),
    'chest_hold': dict(label='胸前停留', loop=True, emotion='真誠、溫柔', usage='說明、安撫、表達同理時的說話姿勢', source=dict(video='qQ40c6kgV8M', start=13.9),
                       description='右手輕放胸口、頭部微傾並有點頭變化；5.2 秒循環。', interrupt='任意時點（接 chest_to_idle 或 chest_to_chin）'),
    'chest_to_idle': dict(label='胸前→待機', loop=False, next='idle_polite', emotion='收尾', usage='胸前說話結束回到待機',
                          source=dict(video='qQ40c6kgV8M', start=12.2, reverse=True, rate=1 / 1.15),
                          description='由抬手動作反向補製（原片無此段），速度放慢 15%。', interrupt='建議播放完畢'),
    'chest_to_chin': dict(label='胸前→托腮', loop=False, next='chin_hold', emotion='俏皮、莞爾', usage='開玩笑、自嘲（「なんて……ふふっ」）',
                          source=dict(video='qQ40c6kgV8M', start=30.05), description='右手由胸口移到下巴，左手移到左髖旁。', interrupt='結束後自動進入托腮停留'),
    'chin_hold': dict(label='托腮微笑', loop=True, emotion='俏皮、愉快', usage='輕鬆話題、玩笑後的停頓', source=dict(video='qQ40c6kgV8M', start=31.2),
                      description='彎曲的食指托住下巴、頭部微傾；2.8 秒循環。', interrupt='任意時點'),
    'chin_to_idle': dict(label='托腮→待機', loop=False, next='idle_polite', emotion='收尾', usage='玩笑後回到正題',
                         source=dict(video='qQ40c6kgV8M', start=34.05), description='放下右手回到雙手交疊待機。', interrupt='建議播放完畢'),
    'idle_to_chin': dict(label='待機→托腮', loop=False, next='chin_hold', emotion='俏皮', usage='直接進入玩笑語氣',
                         source=dict(video='qQ40c6kgV8M', start=34.05, reverse=True), description='由「托腮→待機」反向補製（原片無此段）。', interrupt='建議播放完畢'),
    'talk_full_qQ40': dict(label='完整台詞表演', loop=False, emotion='親切→真誠→俏皮→親切', usage='與原片同步比對、展示用', face=True,
                           source=dict(video='qQ40c6kgV8M', start=0.3), description='原片 0.3–42.0 秒完整還原：身體、眨眼、閉眼、嘴型與微笑（含表情軌道）。',
                           interrupt='展示用'),
}
out = {}
for k, c in clips.items():
    out[k] = dict(frames=length(c), quats={b: np.round(c[b], 6).tolist() for b in BONES}, **META[k])
    print(f'{k:16s} {length(c):4d} frames ({length(c) / FPS:.2f}s) loop={META[k]["loop"]}')
json.dump(dict(fps=FPS, clips=out), open('clips.json', 'w'), ensure_ascii=False)

# seam check: angular jump at loop boundary and at transition joins (deg, worst bone)
def ang(q0, q1):
    return np.degrees(2 * np.arccos(min(1, abs(np.dot(q0 / np.linalg.norm(q0), q1 / np.linalg.norm(q1))))))
for k in ('idle_polite', 'chest_hold', 'chin_hold'):
    c = clips[k]; print('loop seam', k, round(max(ang(c[b][-1], c[b][0]) for b in BONES), 2), 'deg; per-frame typical', round(np.median([ang(c[b][i], c[b][i + 1]) for b in BONES for i in range(0, length(c) - 1, 7)]), 3))
for a, b2 in (('idle_to_chest', 'chest_hold'), ('chest_to_idle', 'idle_polite'), ('chest_to_chin', 'chin_hold'), ('chin_to_idle', 'idle_polite'), ('idle_to_chin', 'chin_hold')):
    print('join', a, '->', b2, round(max(ang(clips[a][b][-1], clips[b2][b][0]) for b in BONES), 2), 'deg')
# seam detail: worst bone vs its own largest in-clip step
for k in ('idle_polite', 'chest_hold', 'chin_hold'):
    c = clips[k]; worst = max(BONES, key=lambda b: ang(c[b][-1], c[b][0]))
    steps = [ang(c[worst][i], c[worst][i + 1]) for i in range(length(c) - 1)]
    print('  seam detail', k, worst, round(ang(c[worst][-1], c[worst][0]), 2), 'max in-clip step', round(max(steps), 2), 'p90', round(np.percentile(steps, 90), 2))
