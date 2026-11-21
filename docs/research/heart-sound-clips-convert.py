#!/usr/bin/env python3
"""One-off conversion of an approved reference clip (brief B28 Part C, item 10).

Usage: heart-sound-clips-convert.py IN.ogg OUT.wav START_S END_S
Needs GStreamer (oggdemux, vorbisdec, audioresample, wavenc) and the Python standard library only.
Steps: decode and downmix to mono, resample to 4 kHz (GStreamer audioresample), cut at START_S..END_S
(END_S = 0 keeps the end), snapping each cut to the quietest 10 ms within 80 ms of the request,
5 ms linear fades at both cuts, peak-normalise to -6 dBFS, write 16-bit PCM.
"""
import array, math, os, subprocess, sys, tempfile, wave

RATE = 4000

def main(src, dst, start_s, end_s):
    with tempfile.TemporaryDirectory() as tmp:
        mid = os.path.join(tmp, "r.wav")
        subprocess.run(["gst-launch-1.0", "-q", "filesrc", f"location={src}", "!", "oggdemux", "!", "vorbisdec",
                        "!", "audioconvert", "!", "audioresample", "!",
                        f"audio/x-raw,format=S16LE,rate={RATE},channels=1", "!", "wavenc", "!",
                        "filesink", f"location={mid}"], check=True)
        with wave.open(mid) as w:
            x = array.array("h", w.readframes(w.getnframes()))
    n = len(x)
    win, reach = RATE // 100, RATE * 8 // 100

    def snap(t):
        c = int(t * RATE)
        best, at = None, c
        for i in range(max(0, c - reach), min(n - win, c + reach) + 1, 8):
            e = sum(v * v for v in x[i:i + win])
            if best is None or e < best:
                best, at = e, i + win // 2
        return at

    a = snap(start_s) if start_s > 0 else 0
    b = snap(end_s) if end_s > 0 else n
    y = [float(v) for v in x[a:b]]
    f = RATE * 5 // 1000
    for i in range(f):
        g = i / f
        y[i] *= g
        y[-1 - i] *= g
    peak = max(abs(v) for v in y)
    k = (10 ** (-6 / 20) * 32767) / peak
    out = array.array("h", [int(round(v * k)) for v in y])
    with wave.open(dst, "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(RATE)
        w.writeframes(out.tobytes())
    print(f"{dst}: {len(out) / RATE:.3f} s, cut {a / RATE:.3f}-{b / RATE:.3f} s of {n / RATE:.3f}, gain {k:.3f}")

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], float(sys.argv[3]), float(sys.argv[4]))
