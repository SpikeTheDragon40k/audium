#!/usr/bin/env bash
# Generate a small demo library for screenshots (scripts/make-screenshots.sh):
# 16 invented books by invented authors, with drawn covers, chapters and
# silent audio of realistic length. Nothing in it is copyrighted material.
# The output is git-ignored.
#
# Usage: scripts/make-demo-library.sh [output-dir]   (default: build/demo-library)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:-$root/build/demo-library}"

command -v ffmpeg >/dev/null || { echo "ffmpeg not found" >&2; exit 1; }
serif=/usr/share/fonts/noto/NotoSerif-Bold.ttf
sans=/usr/share/fonts/noto/NotoSans-Regular.ttf
for f in "$serif" "$sans"; do
    [[ -f "$f" ]] || { echo "font not found: $f (install noto-fonts)" >&2; exit 1; }
done

# Only ever delete our own generated directory.
case "$out" in
    */demo-library) rm -rf -- "$out" ;;
    *) if [[ -e "$out" ]]; then echo "refusing to overwrite $out" >&2; exit 1; fi ;;
esac
mkdir -p "$out"

ff() { ffmpeg -hide_banner -loglevel error -nostdin -y "$@"; }

# Cover text goes to drawtext through files: no quoting or escaping of
# apostrophes and the like in filter arguments.
text="$(mktemp -d)"
trap 'rm -rf -- "$text"' EXIT

# title line 1 | title line 2 (or empty) | author | narrator | series (or empty) | minutes | two cover colours
books=(
    "The Quiet|Harbour|Elena Marsh|Tom Ridley||312|0x1d3557|0x457b9d"
    "Salt and|Starlight|Elena Marsh|Tom Ridley||268|0x264653|0x2a9d8f"
    "The Lantern|Keepers|Owen Clarke|Ada Fenwick|The Lantern Keepers #1|545|0x3d0e61|0xc1121f"
    "The Road to|Kessaly|Owen Clarke|Ada Fenwick|The Lantern Keepers #2|598|0x3d0e61|0xf77f00"
    "Crown of the|Ashen Vale|Owen Clarke|Ada Fenwick|The Lantern Keepers #3|634|0x3d0e61|0xfcbf49"
    "Small Hours at|Hollin Bay|Mira Okafor|Jonah Pike||241|0x0b132b|0x5bc0be"
    "The Cartographer's|Daughter|Lucia Ferrante|Clara Moss||402|0x6d2e46|0xd5b9b2"
    "Nine Winters|North|Henrik Dahl|Peter Lund||377|0x1b263b|0xa9d6e5"
    "The Glass|Orchard|Ines Varga|Clara Moss||289|0x386641|0xa7c957"
    "Letters from the|Lighthouse|Ruth Abernathy|Tom Ridley||223|0x003049|0xeae2b7"
    "Paper Moons|over Vell|Sam Whitlock|Jonah Pike||196|0x22223b|0x9a8c98"
    "The Long Way|to Linden|Mira Okafor|Ada Fenwick||331|0x7f5539|0xddb892"
    "Under Copper|Skies|Henrik Dahl|Peter Lund||458|0x9c6644|0xe6ccb2"
    "The Clockmaker's|Apprentice|Lucia Ferrante|Clara Moss||365|0x14213d|0xfca311"
    "The Salt|Choir|Ines Varga|Jonah Pike||274|0x023e8a|0x90e0ef"
    "A Field Guide to|Forgetting|Ruth Abernathy|Tom Ridley||188|0x5f0f40|0xfb8b24"
)
chapter_names=(Prologue Arrival "The Letter" Crossings "The Storm" "Old Friends" "Night Watch" Departure Returning "The Archive" Embers Thaw "The Bridge" Reckoning Homecoming Epilogue)

# One second of silence, repeated: encoding hours of silence takes moments.
for entry in "${books[@]}"; do
    IFS='|' read -r line1 line2 author narrator series minutes c1 c2 <<<"$entry"
    title="$line1${line2:+ $line2}"
    dir="$out/$author/$title"
    mkdir -p "$dir"

    # Cover: a two-colour gradient, a light band, the title and the author.
    printf '%s' "$line1" > "$text/line1"
    printf '%s' "$line2" > "$text/line2"
    printf '%s' "${author^^}" > "$text/author"
    ff -f lavfi -i "gradients=s=600x600:c0=$c1:c1=$c2:x0=0:y0=0:x1=600:y1=600:d=1" -frames:v 1 \
       -vf "drawbox=y=330:w=iw:h=4:color=white@0.7:t=fill,\
drawtext=fontfile=$serif:textfile=$text/line1:expansion=none:fontcolor=white:fontsize=50:x=(w-text_w)/2:y=180,\
drawtext=fontfile=$serif:textfile=$text/line2:expansion=none:fontcolor=white:fontsize=50:x=(w-text_w)/2:y=250,\
drawtext=fontfile=$sans:textfile=$text/author:expansion=none:fontcolor=white@0.9:fontsize=26:x=(w-text_w)/2:y=480" \
       "$dir/cover.jpg"

    # Chapters every ~25 minutes.
    meta="$dir/.chapters.ffmeta"
    printf ';FFMETADATA1\ntitle=%s\nartist=%s\nalbum=%s\ncomposer=%s\ngenre=Audiobook\n' \
        "$title" "$author" "$title" "$narrator" > "$meta"
    count=$(( minutes / 25 < 2 ? 2 : minutes / 25 ))
    for ((c = 0; c < count; c++)); do
        start=$(( c * minutes * 60000 / count ))
        end=$(( (c + 1) * minutes * 60000 / count ))
        name="${chapter_names[$(( c % ${#chapter_names[@]} ))]}"
        printf '\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=%d\nEND=%d\ntitle=%s\n' "$start" "$end" "$name" >> "$meta"
    done
    # -t limits the output (the silence source never ends by itself).
    ff -f lavfi -i "anullsrc=r=8000:cl=mono" -i "$meta" \
       -map 0:a -map_metadata 1 -map_chapters 1 -c:a libmp3lame -b:a 8k -id3v2_version 4 \
       -t $(( minutes * 60 )) "$dir/$title.mp3"
    rm -f "$meta"

    # Sidecar with the series and a description.
    series_json=""
    if [[ -n "$series" ]]; then series_json=", \"series\": [\"$series\"]"; fi
    printf '{"title": "%s", "authors": ["%s"], "narrators": ["%s"]%s, "description": "A demo book for Audium'\''s screenshots. The title, author and cover are invented, and the audio is silence."}\n' \
        "$title" "$author" "$narrator" "$series_json" > "$dir/metadata.json"
done

echo "Demo library: ${#books[@]} books, $(du -sh "$out" | cut -f1), in $out"
