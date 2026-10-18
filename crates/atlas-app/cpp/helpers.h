// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

// Small Qt helpers that cxx-qt-lib does not bind.

#pragma once

#include <QtCore/QString>
#include <QtCore/QStringList>
#include <QtCore/QObject>
#include <QtGui/QFont>

namespace atlas {

// Advance of one monospace cell, line height and ascent in logical pixels.
double fontCellWidth(const QFont& font);
double fontLineHeight(const QFont& font);
double fontAscent(const QFont& font);

void clipboardSetText(const QString& text);
QString clipboardText();

// File clipboard compatible with other file managers: text/uri-list plus the
// GNOME (x-special/gnome-copied-files) and KDE cut markers.
void clipboardSetFiles(const QStringList& paths, bool cut);
// Local files on the clipboard (empty if there are none).
QStringList clipboardFiles();
bool clipboardFilesCut();

// Sets up QtWebEngine; must run before the QGuiApplication is created.
void initWebEngine();
// Serves `atlas://` URLs for the Markdown preview; needs the application.
void installWebSchemeHandler();

// Renders a QQuickWindow (passed from QML as QObject) into a PNG file.
bool saveWindowImage(QObject* window, const QString& path);

} // namespace atlas
