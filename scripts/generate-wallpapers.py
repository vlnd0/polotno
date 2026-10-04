#!/usr/bin/env python3
"""Generate original, seamless H.264 wallpapers and static modern art. Requires FFmpeg."""
import hashlib
import json
from pathlib import Path
import random
import subprocess

OUT = Path(__file__).resolve().parent.parent / "web/public/wallpapers"
OUT.mkdir(parents=True, exist_ok=True)
PHASE = "2*PI*T/8"
DEFINITIONS = [
    ("silk", "Текучий шёлк", "Мягкие волны", "14172d", "839bc9", ".5+.5*sin(5*X/W+2*sin(3*Y/H+p))", "Спокойные"),
    ("aurora", "Северное сияние", "Световые ленты", "030b19", "5cf4b9", "exp(-pow(Y/H-(.5+.15*sin(4*X/W+p)),2)*30)*(.6+.4*sin(2*X/W-p))", "Спокойные"),
    ("lava", "Жидкая лава", "Тёплое свечение", "180609", "ef7234", ".5+.5*sin(9*X/W+cos(6*Y/H+p)+.5*sin(p))", "Абстракция"),
    ("chrome", "Жидкий хром", "Серебряные складки", "09101a", "c2ccd9", "pow(.5+.5*sin(9*X/W+4*Y/H+2*cos(3*Y/H-p)),3)", "Абстракция"),
    ("waves", "Океан света", "Тонкие световые волны", "020f19", "529ab8", "pow(.5+.5*sin(18*Y/H+2*sin(4*X/W+p)),6)", "Спокойные"),
    ("halo", "Неоновое кольцо", "Плавное дыхание света", "08051c", "bb5eff", "exp(-pow(sqrt(pow(X/W-.5,2)+pow(Y/H-.5,2))-(.24+.05*sin(p)),2)*400)", "Киберпанк"),
    ("city", "Кибергород", "Неоновый силуэт под дождём", "030b20", "7057bd", "pow(.5+.5*cos(90*X/W),30)*pow(.5+.5*sin(18*Y/H-p),10)", "Киберпанк"),
    ("grid", "Цифровой горизонт", "Сетка в бесконечность", "020912", "20b6bd", "min(1,pow(.5+.5*cos(65*(X/W-.5)/(Y/H+.15)),28)+pow(.5+.5*cos(26/(Y/H+.15)-p),32))", "Киберпанк"),
    ("rain", "Неоновый дождь", "Голубые и розовые следы", "040919", "397ede", "pow(.5+.5*cos(100*X/W),36)*pow(.5+.5*sin(24*Y/H-p),12)", "Киберпанк"),
    ("scanner", "Ночной сканер", "Свет скользит в темноте", "020d13", "41dcc2", "exp(-pow(Y/H-(.5+.38*sin(p)),2)*260)*(.5+.5*sin(10*X/W))", "Киберпанк"),
    ("hologram", "Голограмма", "Переливающиеся ткани", "110a23", "f082bc", "pow(.5+.5*sin(12*X/W+3*sin(7*Y/H-p)),3)", "Киберпанк"),
    ("dusk", "Пыльный закат", "Терракота и вечерний свет", "301a2e", "dfa16c", ".5+.5*sin(2*X/W+2*Y/H+.5*sin(p))", "Спокойные"),
]

def command(*args):
    subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", *args], check=True)

def hashed(path):
    digest = hashlib.sha256(path.read_bytes()).hexdigest()[:12]
    result = path.with_name(path.stem + "-" + digest + path.suffix)
    path.replace(result)
    return result.name

manifest = []
for index, (slug, title, description, dark, light, expression, category) in enumerate(DEFINITIONS, 1):
    dark_rgb = [int(dark[i:i+2], 16) for i in (0, 2, 4)]
    light_rgb = [int(light[i:i+2], 16) for i in (0, 2, 4)]
    # Replace only the phase symbol, not the p in exp/pow.
    import re
    expression = re.sub(r"\bp\b", "(" + PHASE + ")", DEFINITIONS[index-1][5])
    channels = [f"{lo}+({hi}-{lo})*({expression})" for lo, hi in zip(dark_rgb, light_rgb)]
    filters = "format=gbrp,geq=" + ":".join(f"{key}='{value}'" for key, value in zip(("r", "g", "b"), channels)) + ",scale=1280:720:flags=bicubic"
    if slug == "city":
        rng = random.Random(42)
        x = 0
        while x < 1280:
            width = rng.randrange(45, 105)
            top = rng.randrange(160, 490)
            filters += f",drawbox=x={x}:y={top}:w={width}:h={720-top}:color=0x090d20:t=fill"
            filters += f",drawbox=x={x}:y={top}:w={width}:h=2:color=0x6c55c0:t=fill"
            for y in range(top+20, 690, 32):
                for wx in range(x+10, min(x+width-10, 1280), 18):
                    if rng.random() < .4:
                        color = rng.choice(("17a7bb", "a748b2", "ae9362"))
                        filters += f",drawbox=x={wx}:y={y}:w=4:h=12:color=0x{color}:t=fill"
            x += width + rng.randrange(5, 16)
    source = "nullsrc=size=384x216:rate=30:duration=8"
    movie = OUT / (slug + ".mp4")
    command("-f", "lavfi", "-i", source, "-vf", filters, "-an", "-c:v", "libx264", "-preset", "fast", "-crf", "23", "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(movie))
    poster = OUT / (slug + ".jpg")
    command("-ss", "1", "-i", str(movie), "-frames:v", "1", "-q:v", "2", str(poster))
    movie_name, poster_name = hashed(movie), hashed(poster)
    manifest.append({"id": index, "title": title, "description": description, "category": category, "kind": "video", "file": movie_name, "poster": poster_name, "duration_seconds": 8})
    # The still version is independently selectable, with no decoder cost.
    manifest.append({"id": 100+index, "title": title + " · картина", "description": "Современная абстракция", "category": category, "kind": "image", "file": poster_name, "poster": poster_name, "duration_seconds": 0})
    print(f"{index}/{len(DEFINITIONS)}: {title}", flush=True)
(OUT / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
