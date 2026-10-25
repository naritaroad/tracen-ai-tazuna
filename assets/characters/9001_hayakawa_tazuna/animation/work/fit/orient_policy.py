import os, stage3 as S3
if os.environ.get('TZ_LATE_IDLE'):
    def phases(t):
        idle = max(S3.ramp(t, 0.0, 12.33), S3.ramp(t, 35.5, 43.0, 0.4))
        return idle, S3.ramp(t, 12.95, 30.12), S3.ramp(t, 30.88, 34.12)
    S3.phases = phases
def left_policy(W, p, t):
    idle, chest, chin = S3.phases(t)
    if chest > 0 or chin > 0:
        W = W.copy(); W[92:112] = 0.0
        if chin > 0 and p['S'][9] > 0.45:
            W[9] = W[91] = 0.5 * p['S'][9]
        elif chest > 0:
            W[9] = W[91] = 0.0
    return W
