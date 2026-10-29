#pragma once

#include <QObject>
#include <QtQmlIntegration/qqmlintegration.h>

#include "audium_bridge/src/book_details.cxxqt.h"

class PlaybackController;

// Loads a library book into the playback controller. (Milestone 3 moves this,
// with resume and progress saving, into the Rust PlaybackSession.)
class LibraryPlayback : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

public:
    explicit LibraryPlayback(PlaybackController *player, QObject *parent = nullptr);

    // Plays the book from the start. Returns false if it has no playable track.
    Q_INVOKABLE bool play(BookDetails *book);

private:
    PlaybackController *m_player;
};
