.pragma library

// Formats milliseconds as h:mm:ss, or m:ss below one hour.
function format(ms) {
    const total = Math.max(0, Math.floor(ms / 1000));
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    const ss = s < 10 ? "0" + s : "" + s;
    if (h > 0) {
        return h + ":" + (m < 10 ? "0" + m : m) + ":" + ss;
    }
    return m + ":" + ss;
}
