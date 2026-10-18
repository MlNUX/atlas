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
#include <QtCore/QFile>
#include <QtCore/QMimeDatabase>
#include <QtWebEngineCore/QWebEngineUrlRequestJob>
#include <QtWebEngineCore/QWebEngineUrlScheme>
#include <QtWebEngineCore/QWebEngineUrlSchemeHandler>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>

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

namespace atlas {

void initWebEngine()
{
    QWebEngineUrlScheme scheme("atlas");
    scheme.setSyntax(QWebEngineUrlScheme::Syntax::Host);
    scheme.setFlags(QWebEngineUrlScheme::SecureScheme | QWebEngineUrlScheme::LocalScheme
                    | QWebEngineUrlScheme::LocalAccessAllowed);
    QWebEngineUrlScheme::registerScheme(scheme);
    QtWebEngineQuick::initialize();
}

} // namespace atlas

namespace {

// `atlas://app/<path>` serves the bundled preview page from the resources,
// `atlas://file/<absolute path>` local files (images in notes). Both share
// one origin, so the preview can show images next to the Markdown file.
class AtlasSchemeHandler : public QWebEngineUrlSchemeHandler {
public:
    using QWebEngineUrlSchemeHandler::QWebEngineUrlSchemeHandler;

    void requestStarted(QWebEngineUrlRequestJob* job) override
    {
        const QUrl url = job->requestUrl();
        QString path;
        if (url.host() == QLatin1String("app"))
            path = QStringLiteral(":/atlas") + url.path();
        else if (url.host() == QLatin1String("file"))
            path = url.path();
        auto* file = new QFile(path, job);
        if (path.isEmpty() || !file->open(QIODevice::ReadOnly)) {
            job->fail(QWebEngineUrlRequestJob::UrlNotFound);
            return;
        }
        static const QMimeDatabase mimes;
        job->reply(mimes.mimeTypeForFile(path).name().toUtf8(), file);
    }
};

} // namespace

namespace atlas {

void installWebSchemeHandler()
{
    auto* handler = new AtlasSchemeHandler(qApp);
    QQuickWebEngineProfile::defaultProfile()->installUrlSchemeHandler("atlas", handler);
}

} // namespace atlas
