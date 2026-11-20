"""Makes the Vitals installer's sound effects in code (chiptune style), so they are original and free of
any licence. Run: python installer/ui/sounds/make_sounds.py  -> writes the .wav files next to this script.

Each voice is a band-limited pulse wave (summed harmonics below Nyquist, so no aliasing fizz), shaped by a
short attack and an exponential release (no clicks at the edges). The melodies are original note patterns.
"""

from pathlib import Path

import numpy as np
from scipy.io import wavfile

RATE = 44100
OUT = Path(__file__).resolve().parent


def note_hz(name: str) -> float:
    """'C5', 'F#4', 'Bb3' -> Hz (A4 = 440)."""
    names = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
    semis = names[name[0]]
    rest = name[1:]
    if rest[0] == "#":
        semis, rest = semis + 1, rest[1:]
    elif rest[0] == "b":
        semis, rest = semis - 1, rest[1:]
    midi = 12 * (int(rest) + 1) + semis
    return 440.0 * 2 ** ((midi - 69) / 12)


# Overtones above this are faded out gently (a 2nd-order roll-off), so notes sound round ("boop"), not
# needle-thin ("beep"). The user found the first, unfiltered set "like a sharp pin in the ear".
SOFTEN_HZ = 2500.0


def pulse(freq, dur, duty=0.5, glide_to=None, vibrato=0.0):
    """A band-limited, softened pulse wave: Fourier series of a pulse of the given duty cycle, with each
    harmonic scaled down by a low-pass curve at SOFTEN_HZ."""
    t = np.arange(int(dur * RATE)) / RATE
    f = np.full_like(t, freq) if glide_to is None else freq * (glide_to / freq) ** (t / dur)
    if vibrato:
        f = f * (1 + vibrato * np.sin(2 * np.pi * 6 * t))
    phase = 2 * np.pi * np.cumsum(f) / RATE
    out = np.zeros_like(t)
    top = max(f.max(), 1.0)
    k = 1
    while k * top < RATE / 2 * 0.9:
        soften = 1.0 / (1.0 + (k * f / SOFTEN_HZ) ** 2)
        out += soften * (np.sin(np.pi * k * duty) / k) * np.cos(k * phase)
        k += 1
    return out / (np.abs(out).max() or 1)


def envelope(n, attack=0.006, release=0.08):
    t = np.arange(n) / RATE
    env = np.minimum(1.0, t / attack) * np.exp(-t / release)
    fade = min(n, int(0.004 * RATE))  # always end at exactly zero
    env[-fade:] *= np.linspace(1, 0, fade)
    return env


def tone(freq, dur, duty=0.5, release=0.08, glide_to=None, vibrato=0.0, gain=1.0):
    w = pulse(freq, dur, duty, glide_to, vibrato)
    return gain * w * envelope(len(w), release=release)


def sequence(notes, step, **kw):
    """Notes placed every `step` seconds; each rings for its own length. notes: (name, length) or None."""
    total = step * len(notes) + max(n[1] for n in notes if n) + 0.05
    out = np.zeros(int(total * RATE))
    for i, n in enumerate(notes):
        if n:
            w = tone(note_hz(n[0]), n[1], **kw)
            s = int(i * step * RATE)
            out[s : s + len(w)] += w
    return out


def echo(x, delay=0.09, decay=0.3, taps=3):
    d = int(delay * RATE)
    out = np.concatenate([x, np.zeros(d * taps)])
    for k in range(1, taps + 1):
        out[k * d : k * d + len(x)] += x * decay**k
    return out


def save(name, x, peak_db=-3.0, rms_db=None):
    """Scale to a peak level, or for the longer event sounds to one RMS level so they sound equally loud
    (a held square wave is much louder than its peak suggests). The peak never goes above -1 dBFS."""
    x = x / np.abs(x).max() * 10 ** (peak_db / 20)
    if rms_db is not None:
        x = x * 10 ** (rms_db / 20) / np.sqrt(np.mean(x**2))
        x = x / max(1.0, np.abs(x).max() / 10 ** (-1 / 20))
    wavfile.write(OUT / f"{name}.wav", RATE, (x * 32767).astype(np.int16))
    print(f"{name}.wav  {len(x) / RATE:.2f} s")


EVENT_RMS = -22.0  # dBFS: paired and complete; uninstalled 2 dB softer


def main():
    # All notes sit an octave below the first version, and thin duty cycles (12.5%, 25%) are replaced by
    # rounder ones (40-50%): both made the first set sound piercing.

    # Button press: a short soft blip that drops a fifth.
    save("click", tone(660, 0.07, duty=0.5, release=0.03, glide_to=440), peak_db=-9)

    # Checkbox / option toggle: two quick rising notes.
    save("toggle", sequence([("E5", 0.05), ("A5", 0.06)], 0.04, duty=0.5, release=0.025), peak_db=-10)

    # A checklist step finished during install: one rounded tick.
    save("tick", tone(note_hz("B5"), 0.08, duty=0.4, release=0.035), peak_db=-11)

    # Install starts: a rising sweep ("power on").
    save("start", tone(110, 0.38, duty=0.5, release=0.22, glide_to=440, vibrato=0.01), peak_db=-6)

    # Paired: a happy rising arpeggio that lands on a held, slightly wobbly top note.
    paired = sequence([("G4", 0.09), ("B4", 0.09), ("D5", 0.09), ("G5", 0.45)], 0.075, duty=0.4, release=0.18, vibrato=0.004)
    save("paired", echo(paired, delay=0.11, decay=0.28), rms_db=EVENT_RMS - 1)

    # Setup complete: a short four-note fanfare, lower voice underneath for weight.
    lead = sequence([("C4", 0.1), ("E4", 0.1), ("G4", 0.1), None, ("C5", 0.4)], 0.11, duty=0.5, release=0.18)
    bass = sequence([("C3", 0.3), None, None, None, ("C3", 0.4)], 0.11, duty=0.5, release=0.2, gain=0.3)
    n = max(len(lead), len(bass))
    done = np.pad(lead, (0, n - len(lead))) + np.pad(bass, (0, n - len(bass)))
    save("complete", echo(done, delay=0.12, decay=0.25, taps=2), rms_db=EVENT_RMS)

    # Uninstalled: sad, not polite. A slow falling C-minor line (G, Eb, D) whose last note, C, sags a
    # semitone and wobbles as it fades, like a machine powering down. (The first version fell through a
    # major chord, which the user heard as "appreciation".)
    # Bright separate blips sound cheerful in any key, so every note here sags in pitch while it plays
    # and runs into the next: two drooping half-steps down, then a long slide down a fourth that
    # wobbles and fades.
    def sag(name, dur, semis, vib=0.0, rel=0.18):
        f = note_hz(name)
        return tone(f, dur, duty=0.5, release=rel, glide_to=f * 2 ** (-semis / 12), vibrato=vib)

    step = 0.24
    parts = [sag("Eb4", 0.27, 0.6), sag("D4", 0.27, 0.6), sag("Db4", 0.75, 5, vib=0.015, rel=0.4)]
    out = np.zeros(int((2 * step + 0.8) * RATE))
    for i, p in enumerate(parts):
        s = int(i * step * RATE)
        out[s : s + len(p)] += p[: len(out) - s]
    save("uninstalled", echo(out, delay=0.14, decay=0.15, taps=1), rms_db=EVENT_RMS - 2)

    # Error: two low notes falling a tritone. The buzz comes from two voices 3% apart beating against
    # each other (roughness in the low-mids), not from treble, so it stays soft on the ear.
    low = [("E3", 0.13), ("A#2", 0.24)]
    a = sequence(low, 0.14, duty=0.2, release=0.13)
    b = sequence([(n, d) for n, d in low], 0.14, duty=0.2, release=0.13)
    b = np.interp(np.arange(len(b)) * 1.03, np.arange(len(b)), b, right=0.0)  # same line, 3% sharp
    save("error", a + 0.8 * b, rms_db=EVENT_RMS - 1)


if __name__ == "__main__":
    main()
