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

// Listening time for statistics: "2 h 05 min", "45 min", "< 1 min", "0 min".
// `i18nc` is passed in from a QML context.
function duration(ms, i18nc) {
    const minutes = Math.floor(Math.max(0, ms) / 60000);
    if (ms > 0 && minutes === 0) {
        return i18nc("@info duration under a minute", "< 1 min");
    }
    const h = Math.floor(minutes / 60);
    const m = minutes % 60;
    if (h === 0) {
        return i18nc("@info duration in minutes", "%1 min", m);
    }
    return i18nc("@info duration in hours and minutes", "%1 h %2 min", h, m < 10 ? "0" + m : m);
}
