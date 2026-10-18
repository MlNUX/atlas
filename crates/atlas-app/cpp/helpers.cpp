// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

#include "helpers.h"

#include <QtCore/QMimeData>
#include <QtCore/QUrl>
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


void clipboardSetFiles(const QStringList& paths, bool cut)
{
    auto* mime = new QMimeData;
    QList<QUrl> urls;
    QByteArray gnome = cut ? "cut" : "copy";
    for (const QString& path : paths) {
        const QUrl url = QUrl::fromLocalFile(path);
        urls << url;
        gnome += '\n' + url.toEncoded();
    }
    mime->setUrls(urls);
    mime->setData(QStringLiteral("x-special/gnome-copied-files"), gnome);
    mime->setData(QStringLiteral("application/x-kde-cutselection"), cut ? "1" : "0");
    mime->setText(paths.join(QLatin1Char('\n')));
    QGuiApplication::clipboard()->setMimeData(mime);
}

QStringList clipboardFiles()
{
    QStringList paths;
    const QMimeData* mime = QGuiApplication::clipboard()->mimeData();
    if (!mime || !mime->hasUrls())
        return paths;
    for (const QUrl& url : mime->urls()) {
        if (url.isLocalFile())
            paths << url.toLocalFile();
    }
    return paths;
}

bool clipboardFilesCut()
{
    const QMimeData* mime = QGuiApplication::clipboard()->mimeData();
    if (!mime)
        return false;
    if (mime->data(QStringLiteral("application/x-kde-cutselection")) == "1")
        return true;
    return mime->data(QStringLiteral("x-special/gnome-copied-files")).startsWith("cut");
}

} // namespace atlas
