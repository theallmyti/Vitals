"""Draws the Vitals icon: the installer's beige pixel computer with a green pulse line on its
screen (the same sprite as drawPC() in installer/ui/index.html). Writes vitals.ico next to this file
(setup) and agent/icons/vitals.ico + vitals-paused.ico (the agent: Start menu, tray; paused = the
pulse goes grey).

    python installer/setup/make_icon.py
"""

from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
AGENT_ICONS = HERE.parent.parent / "agent" / "icons"

OUTLINE, BODY, HI, SHADE, SCREEN, GREEN, GREEN_DARK = (
    "#3E2F1C", "#F5ECD0", "#FDF6E3", "#D9CBA6", "#0F1B2E", "#4CAF50", "#2E6B31",
)
GREY, GREY_DARK = "#8A8F98", "#4A4F57"
BEAT = [0, 0, 0, 0, 0, 0, 0, -1, 0, -4, 3, -1, 0, 0, 0, 0, 0, 0, 0, 0]


def sprite(paused: bool = False) -> Image.Image:
    """The 32x32 pixel computer (the sprite is 32x26, centred vertically). Paused: grey pulse."""
    green, green_dark = (GREY, GREY_DARK) if paused else (GREEN, GREEN_DARK)
    im = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    top = 3

    def rect(x0, y0, x1, y1, c):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                im.putpixel((x, y + top), Image.new("RGB", (1, 1), c).getpixel((0, 0)) + (255,))

    rect(2, 0, 29, 18, OUTLINE); rect(3, 1, 28, 17, BODY); rect(3, 1, 28, 1, HI); rect(3, 17, 28, 17, SHADE)
    rect(5, 3, 26, 14, OUTLINE); rect(6, 4, 25, 13, SCREEN)
    prev = None
    for c in range(20):  # the pulse line, as on the installer's idle screen
        y = 9 + BEAT[(c + 4) % 20]
        if prev is not None and abs(y - prev) > 1:
            rect(6 + c, min(y, prev), 6 + c, max(y, prev), green_dark)
        rect(6 + c, y, 6 + c, y, green)
        prev = y
    rect(24, 16, 24, 16, green)  # power LED
    rect(13, 19, 18, 20, OUTLINE); rect(14, 19, 17, 20, SHADE)
    rect(9, 21, 22, 23, OUTLINE); rect(10, 21, 21, 22, BODY); rect(10, 22, 21, 22, SHADE)
    return im


def save(base: Image.Image, out: Path):
    # Nearest-neighbour scaling keeps the pixels crisp at every size Windows asks for.
    sizes = [16, 24, 32, 48, 64, 128, 256]
    frames = [base.resize((s, s), Image.NEAREST) for s in sizes]
    out.parent.mkdir(parents=True, exist_ok=True)
    frames[-1].save(out, format="ICO", sizes=[(s, s) for s in sizes], append_images=frames[:-1])
    print(f"wrote {out}")


def main():
    save(sprite(), HERE / "vitals.ico")
    save(sprite(), AGENT_ICONS / "vitals.ico")
    save(sprite(paused=True), AGENT_ICONS / "vitals-paused.ico")


if __name__ == "__main__":
    main()
