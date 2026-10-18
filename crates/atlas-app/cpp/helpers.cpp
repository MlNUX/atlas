// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

#include "helpers.h"

#include <QtGui/QClipboard>
#include <QtGui/QFontMetricsF>
#include <QtGui/QGuiApplication>
#include <QtGui/QImage>
#include <QtQuick/QQuickWindow>

namespace atlas {

double fontCellWidth(const QFont& font)
{
    return QFontMetricsF(font).horizontalAdvance(QLatin1Char('M'));
}

double fontLineHeight(const QFont& font)
{
    return QFontMetricsF(font).height();
}

double fontAscent(const QFont& font)
{
    return QFontMetricsF(font).ascent();
}

void clipboardSetText(const QString& text)
{
    QGuiApplication::clipboard()->setText(text);
}

QString clipboardText()
{
    return QGuiApplication::clipboard()->text();
}

bool saveWindowImage(QObject* window, const QString& path)
{
    auto* quickWindow = qobject_cast<QQuickWindow*>(window);
    if (!quickWindow)
        return false;
    return quickWindow->grabWindow().save(path, "PNG");
}

} // namespace atlas
