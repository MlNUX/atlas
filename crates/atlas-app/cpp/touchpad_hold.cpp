// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

// Fingers resting on the touchpad, for stopping momentum scrolling.
//
// Qt reports touchpad scrolling only while the fingers move; putting them
// down again sends nothing, so momentum could not be stopped by a touch.
// libinput recognises that as a "hold" gesture, which compositors (e.g.
// Hyprland, KWin, Mutter) pass on through zwp_pointer_gestures_v1 version 3
// – Qt (6.11) only binds swipes and pinches. Atlas asks for holds itself on
// Qt's wl_pointer. The two protocol interfaces are written out here so the
// build needs no wayland-scanner.

#include "helpers.h"

#include <QtCore/QDebug>
#include <QtCore/QTimer>
#include <QtGui/QGuiApplication>

#if __has_include(<wayland-client.h>)
#include <QtGui/qguiapplication_platform.h>
#include <wayland-client.h>
#include <cstring>
#define ATLAS_WAYLAND 1
#endif

namespace atlas {

#ifdef ATLAS_WAYLAND
namespace {

const wl_interface* kNoTypes[] = { nullptr, nullptr, nullptr, nullptr };

// zwp_pointer_gesture_hold_v1
const wl_interface* kHoldBeginTypes[] = { nullptr, nullptr, &wl_surface_interface, nullptr };
const wl_message kHoldRequests[] = { { "destroy", "3", kNoTypes } };
const wl_message kHoldEvents[] = {
    { "begin", "3uuou", kHoldBeginTypes },
    { "end", "3uui", kNoTypes },
};
const wl_interface kHoldInterface = { "zwp_pointer_gesture_hold_v1", 3, 1, kHoldRequests, 2, kHoldEvents };

// zwp_pointer_gestures_v1 (only get_hold_gesture is used)
const wl_interface* kGetHoldTypes[] = { &kHoldInterface, &wl_pointer_interface };
const wl_interface* kPointerTypes[] = { nullptr, &wl_pointer_interface };
const wl_message kGesturesRequests[] = {
    { "get_swipe_gesture", "no", kPointerTypes },
    { "get_pinch_gesture", "no", kPointerTypes },
    { "release", "2", kNoTypes },
    { "get_hold_gesture", "3no", kGetHoldTypes },
};
const wl_interface kGesturesInterface = { "zwp_pointer_gestures_v1", 3, 4, kGesturesRequests, 0, nullptr };

struct HoldListener {
    void (*begin)(void*, wl_proxy*, uint32_t serial, uint32_t time, wl_surface* surface, uint32_t fingers);
    void (*end)(void*, wl_proxy*, uint32_t serial, uint32_t time, int32_t cancelled);
};

rust::Fn<void()>* callback = nullptr;
wl_proxy* gestures = nullptr;
wl_proxy* hold = nullptr;

void onBegin(void*, wl_proxy*, uint32_t, uint32_t, wl_surface*, uint32_t)
{
    if (callback)
        (*callback)();
}
void onEnd(void*, wl_proxy*, uint32_t, uint32_t, int32_t) {}
const HoldListener kHoldListener = { onBegin, onEnd };

struct Found {
    uint32_t name = 0;
    uint32_t version = 0;
};
void onGlobal(void* data, wl_registry*, uint32_t name, const char* interface, uint32_t version)
{
    if (std::strcmp(interface, "zwp_pointer_gestures_v1") == 0) {
        auto* found = static_cast<Found*>(data);
        found->name = name;
        found->version = version;
    }
}
void onGlobalRemove(void*, wl_registry*, uint32_t) {}
const wl_registry_listener kRegistryListener = { onGlobal, onGlobalRemove };

// Binds the gestures global on a private queue (not to disturb Qt's), then
// moves the objects to the default queue that Qt dispatches.
bool bindHold()
{
    auto* app = qGuiApp ? qGuiApp->nativeInterface<QNativeInterface::QWaylandApplication>() : nullptr;
    if (!app || !app->display() || !app->pointer())
        return false;
    wl_display* display = app->display();
    wl_event_queue* queue = wl_display_create_queue(display);
    auto* wrapper = static_cast<wl_display*>(wl_proxy_create_wrapper(display));
    wl_proxy_set_queue(reinterpret_cast<wl_proxy*>(wrapper), queue);
    wl_registry* registry = wl_display_get_registry(wrapper);
    Found found;
    wl_registry_add_listener(registry, &kRegistryListener, &found);
    wl_display_roundtrip_queue(display, queue);
    if (found.version >= 3) {
        gestures = static_cast<wl_proxy*>(
            wl_registry_bind(registry, found.name, &kGesturesInterface, 3));
        wl_proxy_set_queue(gestures, nullptr);
        hold = wl_proxy_marshal_flags(gestures, 3, &kHoldInterface, 3, 0, nullptr, app->pointer());
        wl_proxy_add_listener(hold, reinterpret_cast<void (**)(void)>(const_cast<HoldListener*>(&kHoldListener)), nullptr);
        wl_display_flush(display);
        qInfo("touchpad: hold gesture enabled (stops momentum scrolling)");
    }
    wl_registry_destroy(registry);
    wl_proxy_wrapper_destroy(wrapper);
    wl_event_queue_destroy(queue);
    return true;
}

} // namespace
#endif

bool watchTouchpadHold(rust::Fn<void()> onHold)
{
#ifdef ATLAS_WAYLAND
    if (callback || QGuiApplication::platformName() != QLatin1String("wayland"))
        return false;
    callback = new rust::Fn<void()>(onHold);
    // The pointer exists once the seat reported it: try a few times.
    auto attempt = std::make_shared<std::function<void(int)>>();
    *attempt = [attempt](int left) {
        if (bindHold() || left <= 0)
            return;
        QTimer::singleShot(500, [attempt, left] { (*attempt)(left - 1); });
    };
    (*attempt)(20);
    return true;
#else
    Q_UNUSED(onHold);
    return false;
#endif
}

} // namespace atlas
