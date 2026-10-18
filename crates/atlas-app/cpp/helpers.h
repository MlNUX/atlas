// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

// Small Qt helpers that cxx-qt-lib does not bind.

#pragma once

#include <QtCore/QString>
#include <QtCore/QObject>
#include <QtGui/QFont>

namespace atlas {

// Advance of one monospace cell, line height and ascent in logical pixels.
double fontCellWidth(const QFont& font);
double fontLineHeight(const QFont& font);
double fontAscent(const QFont& font);

void clipboardSetText(const QString& text);
QString clipboardText();

// Renders a QQuickWindow (passed from QML as QObject) into a PNG file.
bool saveWindowImage(QObject* window, const QString& path);

} // namespace atlas
