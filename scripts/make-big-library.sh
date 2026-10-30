#!/usr/bin/env bash
# Generate a large synthetic audiobook library for the performance pass: by
# default 1000 books in the messy layouts the scanner handles (single m4b
# files with tags and chapters, folders of numbered mp3 parts, CD1/CD2
# subfolders, author/series/book trees, loose files split by name, sidecar
# metadata.json and cover images). The audio is short generated tones; the
# output is git-ignored and contains no copyrighted material.
#
# Usage: scripts/make-big-library.sh [output-dir] [book-count]
#        (default: build/perf-library, 1000)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:-$root/build/perf-library}"
count="${2:-1000}"

command -v ffmpeg >/dev/null || { echo "ffmpeg not found" >&2; exit 1; }

# Only ever delete our own generated directory.
case "$out" in
    */perf-library) rm -rf -- "$out" ;;
    *) if [[ -e "$out" ]]; then echo "refusing to overwrite $out" >&2; exit 1; fi ;;
esac
mkdir -p "$out"
tpl="$out/.templates"
mkdir -p "$tpl"

ff() { ffmpeg -hide_banner -loglevel error -nostdin -y "$@"; }

# Templates: a short mp3 part, a short m4a, three cover images.
ff -f lavfi -i "sine=frequency=440:sample_rate=22050:duration=3" -ac 1 -c:a libmp3lame -b:a 32k "$tpl/part.mp3"
ff -f lavfi -i "sine=frequency=330:sample_rate=22050:duration=6" -ac 1 -c:a aac -b:a 32k "$tpl/book.m4a"
for c in steelblue darkorange seagreen; do
    ff -f lavfi -i "color=c=$c:s=600x600:d=1" -frames:v 1 "$tpl/$c.jpg"
done
covers=(steelblue darkorange seagreen)
cat > "$tpl/chapters.ffmeta" <<'META'
;FFMETADATA1

[CHAPTER]
TIMEBASE=1/1000
START=0
END=2000
title=Chapter One

[CHAPTER]
TIMEBASE=1/1000
START=2000
END=4000
title=Chapter Two

[CHAPTER]
TIMEBASE=1/1000
START=4000
END=6000
title=Chapter Three
META

first=(Ada Bea Cy Dee Eli Fay Gus Hal Ida Jon Kai Lou Max Nia Oto Pia Quin Rex Sol Tia Uma Vic Wen Xia Yan Zoe)
last=(Abbott Brennan Castillo Dvořák Eriksen Fontaine García Holm Ivanova Jensen Kowalski Lindqvist Moreau Novák Okafor Petrov Quist Rossi Santos Tanaka Ueda Vargas Weber Yilmaz Zeller)
adj=(Silent Hidden Last Broken Golden Distant Quiet Burning Lost Forgotten Winter Hollow Crimson Endless Paper Iron)
noun=(River Garden Empire Library Mountain Signal Harbor Kingdom Lantern Orchard Voyage Machine Archive Shadow Island Promise)

author_of() { local i=$1; echo "${first[$((i % 26))]} ${last[$(((i / 26) % 25))]}"; }
title_of() { local i=$1; echo "The ${adj[$((i % 16))]} ${noun[$(((i / 16) % 16))]} $((i / 256 + 1))"; }

# m4b jobs run in parallel at the end (one ffmpeg each).
jobs="$tpl/m4b-jobs"
: > "$jobs"

for ((i = 0; i < count; i++)); do
    # 150 authors, a few dozen series of up to 6 books.
    a=$((i % 150))
    author="$(author_of "$a")"
    title="$(title_of "$i")"
    case $((i % 10)) in
    0|1|2|3)
        # One tagged m4b per book, in an author folder; some with chapters.
        dir="$out/Audiobooks/$author"
        mkdir -p "$dir"
        printf '%s\t%s\t%s\t%s\n' "$dir/$title.m4b" "$title" "$author" "$((i % 3 == 0 ? 1 : 0))" >> "$jobs"
        ;;
    4|5|6)
        # A folder of numbered mp3 parts, untagged: the folder names tell
        # author and title. Half carry a metadata.json, a third a cover.
        dir="$out/Audiobooks/$author/$title"
        mkdir -p "$dir"
        parts=$((2 + i % 11))
        for ((p = 1; p <= parts; p++)); do
            cp "$tpl/part.mp3" "$dir/$(printf '%02d' "$p") - $title.mp3"
        done
        if ((i % 2 == 0)); then
            printf '{"title": "%s", "authors": ["%s"], "narrators": ["%s"], "description": "Book %d of the synthetic library."}\n' \
                "$title" "$author" "$(author_of $((a + 7)))" "$i" > "$dir/metadata.json"
        fi
        if ((i % 3 == 0)); then
            cp "$tpl/${covers[$((i % 3))]}.jpg" "$dir/cover.jpg"
        fi
        ;;
    7|8)
        # Author / Series / Book N with CD1 and CD2 subfolders.
        series="${noun[$((a % 16))]} Saga"
        dir="$out/Series/$author/$series/$title"
        for cd in 1 2; do
            mkdir -p "$dir/CD$cd"
            for ((p = 1; p <= 3; p++)); do
                cp "$tpl/part.mp3" "$dir/CD$cd/Track $(printf '%02d' "$p").mp3"
            done
        done
        cp "$tpl/${covers[$((i % 3))]}.jpg" "$dir/folder.jpg"
        ;;
    9)
        # Loose files in an author folder, split by their name prefix.
        dir="$out/Loose/$author"
        mkdir -p "$dir"
        for p in 1 2 3; do
            cp "$tpl/part.mp3" "$dir/$title - Part $p.mp3"
        done
        ;;
    esac
done

make_m4b() {
    local dst=$1 title=$2 author=$3 chapters=$4 tpl=$5
    if [[ $chapters == 1 ]]; then
        ffmpeg -hide_banner -loglevel error -nostdin -y -i "$tpl/book.m4a" -i "$tpl/chapters.ffmeta" \
            -map 0:a -map_chapters 1 -c copy -f mp4 \
            -metadata title="$title" -metadata album="$title" -metadata artist="$author" -metadata genre=Audiobook "$dst"
    else
        ffmpeg -hide_banner -loglevel error -nostdin -y -i "$tpl/book.m4a" -c copy -f mp4 \
            -metadata title="$title" -metadata album="$title" -metadata artist="$author" -metadata genre=Audiobook "$dst"
    fi
}
export -f make_m4b
export tpl
while IFS=$'\t' read -r dst title author chapters; do
    printf '%s\0%s\0%s\0%s\0' "$dst" "$title" "$author" "$chapters"
done < "$jobs" | xargs -0 -n 4 -P "$(nproc)" bash -c 'make_m4b "$1" "$2" "$3" "$4" "$tpl"' _

rm -rf -- "$tpl"
books=$count
files=$(find "$out" -type f | wc -l)
echo "Generated $books books ($files files, $(du -sh "$out" | cut -f1)) in $out"
