#!/usr/bin/env python3
"""Record real Semaprax commands and render the handbook terminal tour.

Requires Pillow. Pass a CLI built from this checkout with --cli; generated web
files stay under target/ and the committed media goes under handbook/assets/.
"""

import argparse
import shutil
import subprocess
import textwrap
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "handbook/assets/demo"
WEB = "target/handbook-demo-web"
WIDTH, HEIGHT = 1200, 620

STEPS = [
    ("01", "Check", "A readable file becomes a checked program.",
     ["check", "examples/meaning.spx"]),
    ("02", "Run", "The checked entry point returns 42.",
     ["run", "examples/meaning.spx"]),
    ("03", "Inspect", "Stable IDs make declarations easy to find.",
     ["query", "examples/meaning.spx"]),
    ("04", "Test", "A multi-file project runs its declared tests.",
     ["test", "examples/calculator-project/semaprax.toml"]),
    ("05", "Build", "The project produces a WebAssembly package.",
     ["build", "examples/calculator-project/semaprax.toml", "--target", "web", "-o", WEB]),
    ("06", "Verify", "Node checks the generated scalar exports.",
     ["node", "scripts/verify-wasm-scalar-exports.mjs", WEB]),
]


def font(size, bold=False):
    path = (
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf" if bold else
        "/System/Library/Fonts/Menlo.ttc"
    )
    try:
        return ImageFont.truetype(path, size)
    except OSError:
        return ImageFont.truetype("DejaVuSansMono.ttf", size)


BRAND = font(24, bold=True)
TITLE = font(55, bold=True)
BODY = font(23)
SMALL = font(18)
MONO = font(21)


def run(args, cli):
    executable = ["node", *args[1:]] if args[0] == "node" else [str(cli), *args]
    completed = subprocess.run(executable, cwd=ROOT, capture_output=True, text=True)
    if completed.returncode:
        raise RuntimeError(
            f"{' '.join(executable)} failed ({completed.returncode}):\n"
            f"{completed.stdout}{completed.stderr}"
        )
    return completed.stdout + completed.stderr


def split_for_terminal(value, width=55):
    lines = []
    for raw in value.expandtabs(4).splitlines():
        lines.extend(textwrap.wrap(raw, width, break_long_words=True,
                                   break_on_hyphens=False) or [""])
    return lines


def frame(number, title, description, commands):
    im = Image.new("RGB", (WIDTH, HEIGHT), "#091522")
    d = ImageDraw.Draw(im)
    d.rounded_rectangle((24, 24, WIDTH - 24, HEIGHT - 24), radius=30,
                        fill="#102237", outline="#314D64", width=2)
    d.rounded_rectangle((24, 24, WIDTH - 24, 94), radius=30, fill="#172D43")
    d.rectangle((24, 66, WIDTH - 24, 94), fill="#172D43")
    d.text((53, 46), "SEMAPRAX", font=BRAND, fill="#F2F6F8")
    d.text((225, 49), "REAL CLI  /  SOURCE CHECKOUT", font=SMALL, fill="#9FC1C9")
    d.rounded_rectangle((1054, 42, 1144, 78), radius=17, fill="#245767")
    d.text((1068, 48), f"{number} / 06", font=SMALL, fill="#D8F5EC")

    d.rounded_rectangle((48, 126, 355, 532), radius=24, fill="#173047")
    d.text((75, 165), f"STEP {number}", font=SMALL, fill="#64DDB1")
    d.text((72, 210), title, font=TITLE, fill="#F7F2E9")
    for i, line in enumerate(textwrap.wrap(description, 17)):
        d.text((75, 295 + i * 33), line, font=BODY, fill="#C9D8DE")
    d.line((75, 463, 328, 463), fill="#436071", width=2)
    d.text((75, 482), "Meaning in. Code out.", font=SMALL, fill="#8FB8B4")

    d.rounded_rectangle((381, 126, 1152, 532), radius=19,
                        fill="#08121F", outline="#416072", width=2)
    d.rounded_rectangle((381, 126, 1152, 178), radius=19, fill="#23394D")
    d.rectangle((381, 154, 1152, 178), fill="#23394D")
    for x, color in [(408, "#FF817C"), (429, "#F9C66D"), (450, "#75D8AD")]:
        d.ellipse((x, 146, x + 11, 157), fill=color)
    d.text((488, 143), "terminal  ·  repository root", font=SMALL,
           fill="#D5E5EB")

    y = 201
    for command, output in commands:
        for i, line in enumerate(split_for_terminal("$ " + command)):
            d.text((410, y), line, font=MONO,
                   fill="#78E4B5" if i == 0 else "#C1EAD9")
            y += 31
        y += 9
        for line in split_for_terminal(output.rstrip("\n")):
            d.text((410, y), line, font=MONO, fill="#E5EDF1")
            y += 30
        y += 22
    if y > 516:
        raise ValueError(f"terminal content exceeds frame: {title}")

    for i in range(6):
        d.rounded_rectangle((497 + i * 34, 566, 516 + i * 34, 576), radius=5,
                            fill="#62DDB0" if i + 1 == int(number) else "#476579")
    d.text((842, 558), "Recorded from Semaprax v0.6.0", font=SMALL,
           fill="#90A9B4")
    return im


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path,
                        help="path to the source-built semaprax executable")
    args = parser.parse_args()
    cli = args.cli.resolve()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    version = run(["--version"], cli).strip()
    web_output = ROOT / WEB
    if web_output.is_symlink():
        raise RuntimeError(f"refusing symlink at {web_output}")
    if web_output.exists():
        shutil.rmtree(web_output)
    records = []
    frames = []
    for number, title, description, cmd in STEPS:
        output = run(cmd, cli)
        command = " ".join(["semaprax", *cmd]) if cmd[0] != "node" else " ".join(cmd)
        records.append((command, output))
        frames.append(frame(number, title, description, [(command, output)]))

    still = frame("01", "First run", "A checked source file returns 42.",
                  records[:2])
    still.save(OUTPUT / "check-and-run.png", optimize=True)
    frames[0].save(OUTPUT / "first-steps.gif", save_all=True,
                   append_images=frames[1:], duration=2400, loop=0,
                   optimize=True, disposal=2)
    transcript = [f"Recorded with {version}", "Repository root"]
    for command, output in records:
        transcript.append(f"$ {command}\n{output.rstrip()}")
    (OUTPUT / "transcript.txt").write_text(
        "\n\n".join(transcript) + "\n", encoding="utf-8"
    )
    print(f"Wrote {OUTPUT / 'first-steps.gif'}")
    print(f"Wrote {OUTPUT / 'check-and-run.png'}")
    print(f"Wrote {OUTPUT / 'transcript.txt'}")


if __name__ == "__main__":
    main()
