#!/usr/bin/env bash
# Generate synthetic audio fixtures with ffmpeg. Output is tiny, contains no
# copyrighted audio and is git-ignored. Re-running replaces the output dir.
#
# Usage: scripts/make-fixtures.sh [output-dir]   (default: tests/fixtures/generated)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:-$root/tests/fixtures/generated}"

command -v ffmpeg >/dev/null || { echo "ffmpeg not found" >&2; exit 1; }

# Only ever delete our own generated directory.
case "$out" in
    */tests/fixtures/generated|*/fixtures-*) rm -rf -- "$out" ;;
    *) if [[ -e "$out" ]]; then echo "refusing to overwrite $out" >&2; exit 1; fi ;;
esac
mkdir -p "$out"

ff() { ffmpeg -hide_banner -loglevel error -nostdin -y "$@"; }

# tone <seconds> <frequency> <out> [extra ffmpeg args...]
tone() {
    local secs=$1 freq=$2 dst=$3; shift 3
    ff -f lavfi -i "sine=frequency=${freq}:sample_rate=22050:duration=${secs}" -ac 1 "$@" "$dst"
}

# Chapter metadata: three 2-second chapters.
meta="$out/.chapters.ffmeta"
cat > "$meta" <<'META'
;FFMETADATA1
title=Fixture Book With Chapters
artist=Ada Example
album=Fixture Book With Chapters
composer=Nora Narrator
genre=Audiobook

[CHAPTER]
TIMEBASE=1/1000
START=0
END=2000
title=Opening

[CHAPTER]
TIMEBASE=1/1000
START=2000
END=4000
title=Middle

[CHAPTER]
TIMEBASE=1/1000
START=4000
END=6000
title=Ending
META

# 1. Single m4b with embedded chapters and a cover.
ff -f lavfi -i "color=c=steelblue:s=300x300:d=1" -frames:v 1 "$out/.cover.png"
tone 6 440 "$out/.tmp.m4a" -c:a aac -b:a 32k
mkdir -p "$out/single-m4b"
ff -i "$out/.tmp.m4a" -i "$meta" -i "$out/.cover.png" \
   -map 0:a -map 2:v -map_metadata 1 -map_chapters 1 \
   -c:a copy -c:v png -disposition:v attached_pic \
   -f mp4 "$out/single-m4b/Ada Example - Fixture Book With Chapters.m4b"

# 2. Single mp3 with ID3v2 CHAP frames.
mkdir -p "$out/single-mp3-chapters"
tone 6 523 "$out/.tmp.wav"
ff -i "$out/.tmp.wav" -i "$meta" -map 0:a -map_metadata 1 -map_chapters 1 \
   -c:a libmp3lame -b:a 32k -id3v2_version 4 "$out/single-mp3-chapters/book.mp3"

# 3. Folder of tracks = one book, with names that need natural sort (2 < 10).
d="$out/folder-book/Bea Writer/The Folder Book"
mkdir -p "$d"
for n in 1 2 10; do
    tone 2 $((300 + n * 20)) "$d/Track $n.mp3" -c:a libmp3lame -b:a 32k \
        -metadata title="Track $n" -metadata album="The Folder Book" \
        -metadata artist="Bea Writer" -metadata track="$n"
done
printf 'A synthetic description.\n' > "$d/desc.txt"
printf 'Nora Narrator\n' > "$d/reader.txt"
cp "$out/.cover.png" "$d/cover.png"

# 4. Multi-disc book: CD1/CD2 subfolders merge into the parent.
d="$out/multi-disc/Cy Author/Disc Book"
for disc in 1 2; do
    mkdir -p "$d/CD$disc"
    for n in 1 2; do
        tone 2 $((400 + disc * 50 + n * 10)) "$d/CD$disc/0$n.mp3" -c:a libmp3lame -b:a 32k \
            -metadata album="Disc Book" -metadata artist="Cy Author" \
            -metadata disc="$disc/2" -metadata track="$n/2"
    done
done

# 5. Author folder with loose single files for two different albums.
d="$out/loose-author/Dee Novelist"
mkdir -p "$d"
for book in "First Story" "Second Story"; do
    for n in 1 2; do
        tone 2 600 "$d/$book - Part $n.mp3" -c:a libmp3lame -b:a 32k \
            -metadata album="$book" -metadata artist="Dee Novelist" -metadata track="$n"
    done
done

# 6. Unicode names and no tags at all.
d="$out/unicode/Ünïcødé Àuthor/Livre d’été 本"
mkdir -p "$d"
tone 2 700 "$d/Chapitre 01 – début.mp3" -c:a libmp3lame -b:a 32k -map_metadata -1
tone 2 710 "$d/Chapitre 02 – fin.mp3" -c:a libmp3lame -b:a 32k -map_metadata -1

# 7. Corrupt files: truncated mp3 and random bytes with an audio extension.
d="$out/corrupt"
mkdir -p "$d"
tone 4 800 "$out/.tmp-corrupt.mp3" -c:a libmp3lame -b:a 32k
head -c 2048 "$out/.tmp-corrupt.mp3" > "$d/truncated.mp3"
head -c 4096 /dev/urandom > "$d/garbage.m4b"

rm -f "$out"/.tmp* "$out/.cover.png" "$meta"
echo "Fixtures written to $out"
find "$out" -type f | sort
