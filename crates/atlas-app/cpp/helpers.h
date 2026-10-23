// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

// Small Qt helpers that cxx-qt-lib does not bind.

#pragma once

#include <QtCore/QString>
#include <QtCore/QStringList>
#include <QtCore/QObject>
#include <QtGui/QFont>

#include "rust/cxx.h"

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

// Sends a wheel event (`delta` in eighths of a degree, 120 = one notch) at
// (x, y), with Ctrl if `ctrl`; for the smoke test.
void sendWheel(QObject* window, double x, double y, int delta, int flags);
void sendKey(QObject* window, int key, int modifiers);

// Calls `onHold` when fingers come to rest on the touchpad (Wayland hold
// gesture, see touchpad_hold.cpp); false where that is not available.
bool watchTouchpadHold(rust::Fn<void()> onHold);

// File suffixes of the image formats Qt can read here (lower case).
QStringList imageFormats();

// Makes `owner` the QObject parent of `object` (its lifetime owner),
// independent of the visual parent.
void setOwner(QObject* object, QObject* owner);

// Sends a left mouse event at (x, y) in window coordinates after `delayMs`:
// kind 0 press, 1 move, 2 release. Used by the smoke test to drive real
// drag-and-drop.
void postMouse(QObject* window, int kind, double x, double y, int delayMs);

// Cookies of a web profile (QML WebEngineProfile, passed as QObject), for
// HTTP requests made from Rust with the same session (ILIAS): followed
// from now on; false if `profile` is none.
bool watchWebCookies(QObject* profile);
// The `Cookie` header the watched profile would send to `url`.
QString webCookieHeader(const QString& url);
// The user agent the watched profile's pages send ("" without one).
QString webUserAgent();
// Deletes all cookies of the watched profile (signs out everywhere).
void clearWebCookies();

} // namespace atlas
