#!/bin/bash
# The gallery (ROADMAP Stage 1): the standard sites of a world rendered side by side, so a
# change in the region or the embark is judged on many places at once.
#
#   scripts/gallery.sh [OUT_DIR] [WORLD_ARGS...]
#
# Sites come from $SITES ("X,Y X,Y ...") or, by default, the generation sites written in
# CLAUDE.md for seed 42 at 512x256 under the Sun. For each site it runs --local-snapshot
# (the region in ink at 4 px, the embark's surface, the level below, the section) and --zoom
# (the landform classes), then composes one contact sheet per world: rows are sites, columns
# are the five views. The sheet is made with Pillow if python3 has it, else ImageMagick's
# montage, else an index.html that lays the PNGs out in the same grid.
#
#   scripts/gallery.sh out/gallery --load-world worlds/s42_sol.world
#   SITES="42,32 33,32 36,23" scripts/gallery.sh out/dev --dev
#
# A saved world (--save-world, then --load-world) spares the ~4 s of world generation per site.
set -euo pipefail
cd "$(dirname "$0")/.."

OUT=${1:-out/gallery}
shift || true
WORLD_ARGS=("$@")
if [ ${#WORLD_ARGS[@]} -eq 0 ]; then
    if [ -f worlds/s42_sol.world ]; then WORLD_ARGS=(--load-world worlds/s42_sol.world); else WORLD_ARGS=(--seed 42 --star sol --width 512 --height 256); fi
fi
# The generation sites of seed 42 (CLAUDE.md, "Standard seeds"), when none are given.
SITES=${SITES:-"$(grep -o 'generation sites (seed 42[^:]*): [^.]*' CLAUDE.md | head -1 | grep -oE '[0-9]+,[0-9]+' | tr '\n' ' ')"}
if [ -z "$SITES" ]; then SITES="392,97 177,133 124,153 382,62 272,48 18,106"; fi

BIN=target/release/planet_generator
[ -x "$BIN" ] || cargo build --release
mkdir -p "$OUT"
export PLANET_CHUNK_CACHE=${PLANET_CHUNK_CACHE:-$PWD/.chunk_cache_main}

rows=()
for site in $SITES; do
    tag=${site/,/_}
    echo "== $site"
    "$BIN" "${WORLD_ARGS[@]}" --headless --no-history --local-snapshot "$OUT/s$tag" --tiles-center "$site" 2>&1 | grep -E "^Site|^Relief|Region in ink:" || true
    "$BIN" "${WORLD_ARGS[@]}" --headless --no-history --zoom "$site" --map-output-dir "$OUT" 2>&1 | grep -E "landforms:" || true
    zoom=$(ls "$OUT"/zoom_*_"${site/,/_}"_landforms.png 2>/dev/null | head -1 || true)
    below=$(ls "$OUT"/s${tag}_z*.png 2>/dev/null | sort | sed -n 2p || true)
    rows+=("$site|$OUT/s${tag}_region_4px.png|${zoom:-}|$OUT/s${tag}_surface.png|${below:-}|$OUT/s${tag}_section.png")
done

SHEET="$OUT/gallery.png"
if python3 -c "import PIL" 2>/dev/null; then
    python3 - "$SHEET" "${rows[@]}" <<'EOF'
import sys
from PIL import Image, ImageDraw
sheet, rows = sys.argv[1], sys.argv[2:]
cell = 320
imgs = []
for r in rows:
    parts = r.split("|")
    row = []
    for p in parts[1:]:
        try:
            im = Image.open(p).convert("RGB")
            im.thumbnail((cell, cell))
        except Exception:
            im = Image.new("RGB", (cell, cell), (236, 226, 202))
        row.append(im)
    imgs.append((parts[0], row))
W = 5 * cell + 6 * 8 + 90
H = len(imgs) * (cell + 8) + 8
out = Image.new("RGB", (W, H), (236, 226, 202))
d = ImageDraw.Draw(out)
for i, (site, row) in enumerate(imgs):
    y = 8 + i * (cell + 8)
    d.text((8, y + 4), site, fill=(51, 38, 28))
    for j, im in enumerate(row):
        out.paste(im, (90 + 8 + j * (cell + 8), y))
out.save(sheet)
print("saved", sheet)
EOF
elif command -v montage >/dev/null 2>&1; then
    files=()
    for r in "${rows[@]}"; do IFS='|' read -r _ a b c d e <<<"$r"; files+=("${a:-null:}" "${b:-null:}" "${c:-null:}" "${d:-null:}" "${e:-null:}"); done
    montage "${files[@]}" -tile 5x -geometry 320x320+4+4 -background '#ece2ca' "$SHEET" && echo "saved $SHEET"
else
    html="$OUT/index.html"
    {
        echo '<!doctype html><meta charset=utf-8><title>Gallery</title><style>body{background:#ece2ca;font:14px sans-serif}td{padding:4px}img{width:320px}</style><table>'
        echo '<tr><th>site<th>region (ink)<th>landforms<th>surface<th>below<th>section'
        for r in "${rows[@]}"; do
            IFS='|' read -r site a b c d e <<<"$r"
            echo "<tr><td>$site"
            for f in "$a" "$b" "$c" "$d" "$e"; do echo "<td><img src=\"$(basename "$f")\">"; done
        done
        echo '</table>'
    } >"$html"
    echo "saved $html (no Pillow or montage: an HTML grid)"
fi
