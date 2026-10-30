#pragma once

#include <QObject>
#include <QString>

class PlaybackSession;

// Applies the colour theme chosen in Settings (PlaybackSession::theme) to
// the whole application: System follows the desktop (Breeze light or dark);
// Dracula and Nord are KDE colour schemes shipped with Audium
// (app/themes/*.colors). The palette is set application-wide, the way
// KColorSchemeManager does it, so Qt Quick controls and Kirigami colours
// all follow.
class Theme : public QObject
{
    Q_OBJECT

public:
    enum Scheme { System = 0, Dracula = 1, Nord = 2 };
    Q_ENUM(Scheme)

    explicit Theme(PlaybackSession *session, QObject *parent = nullptr);

    // The .colors file of `scheme`, or an empty string for System (or when
    // the file is missing, which then also falls back to System).
    static QString schemeFile(int scheme);

private:
    void apply();

    PlaybackSession *m_session;
    int m_applied = System;
};
