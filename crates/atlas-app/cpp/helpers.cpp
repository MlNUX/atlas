// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

#include "helpers.h"

#include <QtCore/QMimeData>
#include <QtCore/QUrl>
#include <QtGui/QClipboard>
#include <QtGui/QFontMetricsF>
#include <QtGui/QGuiApplication>
#include <QtGui/QImage>
#include <QtGui/QImageReader>
#include <QtGui/QKeyEvent>
#include <QtGui/QMouseEvent>
#include <QtGui/QPointingDevice>
#include <QtGui/QWindow>
#include <QtCore/QCoreApplication>
#include <QtCore/QPointer>
#include <QtCore/QTimer>

// Exported by QtGui for QTest (qtestmouse.h); feeds a mouse event through
// QWindowSystemInterface.
Q_GUI_EXPORT void qt_handleMouseEvent(QWindow* window, const QPointF& local, const QPointF& global, Qt::MouseButtons state,
                                      Qt::MouseButton button, QEvent::Type type, Qt::KeyboardModifiers mods, int timestamp);
#include <QtQuick/QQuickWindow>
#include <QtCore/QFile>
#include <QtCore/QMimeDatabase>
#include <QtWebEngineCore/QWebEngineUrlRequestJob>
#include <QtWebEngineCore/QWebEngineUrlScheme>
#include <QtWebEngineCore/QWebEngineUrlSchemeHandler>
#include <QtWebEngineCore/QWebEngineCookieStore>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QtCore/QDateTime>
#include <QtNetwork/QNetworkCookie>
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


void sendWheel(QObject* window, double x, double y, int delta, int flags)
{
    auto* w = qobject_cast<QWindow*>(window);
    if (!w)
        return;
    const QPointF local(x, y);
    // flags: 1 Ctrl held, 2 touchpad-like (pixel delta only, in a scroll
    // phase), 4 the gesture's ScrollBegin, 8 wheel notches in a scroll phase,
    // 16 from a "touchpad" device with finger pointer type (as Qt on Wayland
    // reports even mouse wheels).
    static QPointingDevice touchpad(QStringLiteral("touchpad"), 0x7a71, QInputDevice::DeviceType::TouchPad,
                                    QPointingDevice::PointerType::Finger, QInputDevice::Capability::Position
                                        | QInputDevice::Capability::Scroll, 10, 0);
    const bool pixel = flags & 2;
    const Qt::ScrollPhase phase = (flags & 4) ? Qt::ScrollBegin : (flags & (2 | 8)) ? Qt::ScrollUpdate : Qt::NoScrollPhase;
    QWheelEvent event(local, w->mapToGlobal(local), pixel ? QPoint(0, delta) : QPoint(), pixel ? QPoint() : QPoint(0, delta),
                      Qt::NoButton, (flags & 1) ? Qt::ControlModifier : Qt::NoModifier, phase, false,
                      Qt::MouseEventNotSynthesized, (flags & 16) ? &touchpad : QPointingDevice::primaryPointingDevice());
    QCoreApplication::sendEvent(w, &event);
}

void sendKey(QObject* window, int key, int modifiers)
{
    auto* w = qobject_cast<QWindow*>(window);
    if (!w)
        return;
    const auto mods = Qt::KeyboardModifiers(modifiers);
    QKeyEvent press(QEvent::KeyPress, key, mods);
    QCoreApplication::sendEvent(w, &press);
    QKeyEvent release(QEvent::KeyRelease, key, mods);
    QCoreApplication::sendEvent(w, &release);
}

QStringList imageFormats()
{
    QStringList out;
    for (const QByteArray& format : QImageReader::supportedImageFormats())
        out << QString::fromLatin1(format).toLower();
    return out;
}

void setOwner(QObject* object, QObject* owner)
{
    if (object && owner)
        object->setParent(owner);
}

void postMouse(QObject* window, int kind, double x, double y, int delayMs)
{
    QPointer<QWindow> w = qobject_cast<QWindow*>(window);
    if (!w)
        return;
    // Through the window system like a real mouse (as QTest does), so the
    // application tracks the button state and drags see the events. Each
    // event has its own timer: they keep firing inside a drag's event loop.
    // kind + 10: with Ctrl held.
    const Qt::KeyboardModifiers modifiers = kind >= 10 ? Qt::ControlModifier : Qt::NoModifier;
    kind %= 10;
    QTimer::singleShot(delayMs, [w, kind, x, y, modifiers] {
        if (!w)
            return;
        const QPointF local(x, y);
        const QEvent::Type type = kind == 0 ? QEvent::MouseButtonPress : kind == 1 ? QEvent::MouseMove : QEvent::MouseButtonRelease;
        const Qt::MouseButton button = kind == 1 ? Qt::NoButton : Qt::LeftButton;
        const Qt::MouseButtons buttons = kind == 2 ? Qt::NoButton : Qt::LeftButton;
        static int timestamp = 0;
        timestamp += 50;
        qt_handleMouseEvent(w, local, w->mapToGlobal(local), buttons, button, type, modifiers, timestamp);
    });
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

namespace {
// The cookies of the watched profile, kept current by its cookie store.
QPointer<QQuickWebEngineProfile> cookieProfile;
QList<QNetworkCookie> cookies;
}

bool watchWebCookies(QObject* profile)
{
    auto* p = qobject_cast<QQuickWebEngineProfile*>(profile);
    if (!p)
        return false;
    if (cookieProfile == p)
        return true;
    if (cookieProfile)
        QObject::disconnect(cookieProfile->cookieStore(), nullptr, qApp, nullptr);
    cookieProfile = p;
    cookies.clear();
    QWebEngineCookieStore* store = p->cookieStore();
    QObject::connect(store, &QWebEngineCookieStore::cookieAdded, qApp, [](const QNetworkCookie& c) {
        cookies.removeIf([&](const QNetworkCookie& o) { return o.hasSameIdentifier(c); });
        cookies.append(c);
    });
    QObject::connect(store, &QWebEngineCookieStore::cookieRemoved, qApp, [](const QNetworkCookie& c) {
        cookies.removeIf([&](const QNetworkCookie& o) { return o.hasSameIdentifier(c); });
    });
    store->loadAllCookies();
    return true;
}

QString webCookieHeader(const QString& address)
{
    const QUrl url(address);
    const QString host = url.host().toLower();
    const QString path = url.path().isEmpty() ? QStringLiteral("/") : url.path();
    const QDateTime now = QDateTime::currentDateTimeUtc();
    QStringList parts;
    for (const QNetworkCookie& c : std::as_const(cookies)) {
        QString domain = c.domain().toLower();
        if (domain.startsWith('.'))
            domain.remove(0, 1);
        if (host != domain && !host.endsWith('.' + domain))
            continue;
        if (!c.path().isEmpty() && !path.startsWith(c.path()))
            continue;
        if (c.expirationDate().isValid() && c.expirationDate() < now)
            continue;
        if (c.isSecure() && url.scheme() != QLatin1String("https"))
            continue;
        parts << QString::fromUtf8(c.name()) + '=' + QString::fromUtf8(c.value());
    }
    return parts.join(QStringLiteral("; "));
}

QString webCookieSummary()
{
    if (!cookieProfile)
        return QStringLiteral("kein Web-Profil");
    QStringList parts;
    for (const QNetworkCookie& c : std::as_const(cookies))
        parts << QString::fromUtf8(c.name()) + '@' + c.domain() + c.path() + (c.isSecure() ? QStringLiteral(" (secure)") : QString());
    return parts.isEmpty() ? QStringLiteral("keine Cookies im Web-Profil") : parts.join(QStringLiteral(", "));
}

QString webUserAgent()
{
    return cookieProfile ? cookieProfile->httpUserAgent() : QString();
}

void clearWebCookies()
{
    if (cookieProfile)
        cookieProfile->cookieStore()->deleteAllCookies();
    cookies.clear();
}

} // namespace atlas
