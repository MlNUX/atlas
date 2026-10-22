// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import Atlas

// A line icon (24 × 24 grid, drawn as SVG in any colour). Unknown names
// draw nothing.
Item {
    id: icon
    property string name
    property color color: Theme.textMuted
    property int size: 16
    /// Line width on the 24 grid.
    property real weight: 1.8

    // Shapes on the 24 × 24 grid; strokes unless noted.
    readonly property var shapes: ({
        "home": "<path d='M3 10.5 12 3l9 7.5'/><path d='M5 9.5V20h14V9.5'/><path d='M10 20v-6h4v6'/>",
        "desktop": "<rect x='3' y='4' width='18' height='12' rx='2'/><path d='M8 20h8M12 16v4'/>",
        "document": "<path d='M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z'/><path d='M14 3v5h5'/>",
        "documents": "<path d='M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z'/><path d='M14 3v5h5'/>",
        "downloads": "<path d='M12 4v11M7 10l5 5 5-5'/><path d='M4 19h16'/>",
        "pictures": "<rect x='3' y='4' width='18' height='16' rx='2'/><circle cx='9' cy='10' r='1.8'/><path d='m21 16-5-5-9 9'/>",
        "music": "<path d='M9 18V5l11-2v13'/><circle cx='6.5' cy='18' r='2.5'/><circle cx='17.5' cy='16' r='2.5'/>",
        "videos": "<rect x='3' y='5' width='18' height='14' rx='2'/><path d='m10 9 5 3-5 3z'/>",
        "trash": "<path d='M4 7h16M10 11v6M14 11v6'/><path d='M6 7l1 13h10l1-13'/><path d='M9 7V4h6v3'/>",
        "folder": "<path d='M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z'/>",
        "folder-fill": "<path fill='COLOR' d='M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z'/>",
        "folder-open": "<path d='M3 18.5V6.5A1.5 1.5 0 0 1 4.5 5H9l2 2.5h7A1.5 1.5 0 0 1 19.5 9v1.5'/><path d='M3 18.5 5.5 11h16L19 20H4.5A1.5 1.5 0 0 1 3 18.5z'/>",
        "folder-plus": "<path d='M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z'/><path d='M12 11v6M9 14h6'/>",
        "file-plus": "<path d='M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z'/><path d='M14 3v5h5M12 11v6M9 14h6'/>",
        "file-pdf": "<path d='M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z'/><path d='M14 3v5h5'/><path d='M8.5 17c2-1.5 3.5-4.5 3.5-7 0 3 2 5 4 6-2.5-.5-5 0-7.5 1z'/>",
        "file-text": "<path d='M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z'/><path d='M14 3v5h5M9 12h6M9 15.5h6M9 9h2'/>",
        "file-code": "<path d='M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z'/><path d='M14 3v5h5M10 12l-2 2.5 2 2.5M14 12l2 2.5-2 2.5'/>",
        "file-table": "<path d='M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z'/><path d='M14 3v5h5M8 12h8v6H8zM8 15h8M12 12v6'/>",
        "file-image": "<path d='M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z'/><path d='M14 3v5h5'/><circle cx='10' cy='12' r='1.5'/><path d='m18 17-3-3-6 6'/>",
        "chevron-right": "<path d='m9 6 6 6-6 6'/>",
        "chevron-down": "<path d='m6 9 6 6 6-6'/>",
        "chevron-left": "<path d='m15 6-6 6 6 6'/>",
        "search": "<circle cx='11' cy='11' r='6.5'/><path d='m20 20-4.2-4.2'/>",
        "calendar": "<rect x='3.5' y='5' width='17' height='15' rx='2'/><path d='M3.5 10h17M8 3v4M16 3v4'/>",
        "session": "<rect x='3' y='4' width='18' height='12' rx='2'/><path d='M8 20h8M12 16v4'/>",
        "menu": "<path d='M4 7h16M4 12h16M4 17h16'/>",
        "plus": "<path d='M12 5v14M5 12h14'/>",
        "check": "<path d='m5 12.5 4.5 4.5L19 7.5'/>",
        "minus": "<path d='M5 12h14'/>",
        "close": "<path d='M6 6l12 12M18 6 6 18'/>",
        "arrow-left": "<path d='M19 12H5M11 6l-6 6 6 6'/>",
        "arrow-up": "<path d='M12 19V5M6 11l6-6 6 6'/>",
        "grid": "<rect x='4' y='4' width='6.5' height='6.5' rx='1.5'/><rect x='13.5' y='4' width='6.5' height='6.5' rx='1.5'/><rect x='4' y='13.5' width='6.5' height='6.5' rx='1.5'/><rect x='13.5' y='13.5' width='6.5' height='6.5' rx='1.5'/>",
        "arrow-right": "<path d='M5 12h14M13 6l6 6-6 6'/>",
        "terminal": "<rect x='3' y='4' width='18' height='16' rx='2'/><path d='m7 9 3 3-3 3M12.5 15H17'/>",
        "split": "<rect x='3' y='4' width='18' height='16' rx='2'/><path d='M12 4v16'/>",
        "split-vertical": "<rect x='3' y='4' width='18' height='16' rx='2'/><path d='M3 12h18'/>",
        "sidebar-right": "<rect x='3' y='4' width='18' height='16' rx='2'/><path d='M15 4v16'/>",
        "sidebar-left": "<rect x='3' y='4' width='18' height='16' rx='2'/><path d='M9 4v16'/>",
        "link": "<path d='M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1'/><path d='M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1'/>",
        "focus": "<circle cx='12' cy='12' r='3'/><path d='M12 2v4M12 18v4M2 12h4M18 12h4'/>",
        "graph": "<circle cx='12' cy='12' r='2.5'/><circle cx='5' cy='6' r='2'/><circle cx='19' cy='6' r='2'/><circle cx='6' cy='19' r='2'/><circle cx='18' cy='18' r='2'/><path d='m6.5 7.5 3.6 3M17.5 7.5l-3.6 3M7.5 17.5l2.8-3.6M16.5 16.5l-2.6-2.7'/>",
        "quote": "<path fill='COLOR' stroke='none' d='M5 17.5c2.8 0 4.5-2 4.5-4.8V7H4v5.5h3c0 1.6-.8 2.7-2 2.9zM14 17.5c2.8 0 4.5-2 4.5-4.8V7H13v5.5h3c0 1.6-.8 2.7-2 2.9z'/>",
        "play": "<path d='M7 5v14l11-7z'/>",
        "pause": "<path d='M8 5v14M16 5v14'/>",
        "undo": "<path d='M9 14 4 9l5-5'/><path d='M4 9h10.5a5.5 5.5 0 0 1 0 11H11'/>",
        "redo": "<path d='m15 14 5-5-5-5'/><path d='M20 9H9.5a5.5 5.5 0 0 0 0 11H13'/>",
        "skip": "<path d='M6 5v14l9-7z'/><path d='M18 5v14'/>",
        "coffee": "<path d='M5 9h11v5a5 5 0 0 1-5 5H10a5 5 0 0 1-5-5z'/><path d='M16 11h1.5a2.5 2.5 0 0 1 0 5H16'/><path d='M9 3v3M12 3v3'/>",
        "bookmark": "<path d='M6 4h12v17l-6-4-6 4z'/>",
        "settings": "<circle cx='12' cy='12' r='3'/><path d='M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1'/>",
        "refresh": "<path d='M20 11a8 8 0 0 0-14.5-4.5L4 8M4 4v4h4'/><path d='M4 13a8 8 0 0 0 14.5 4.5L20 16M20 20v-4h-4'/>",
        "more": "<circle fill='COLOR' stroke='none' cx='12' cy='5.5' r='1.6'/><circle fill='COLOR' stroke='none' cx='12' cy='12' r='1.6'/><circle fill='COLOR' stroke='none' cx='12' cy='18.5' r='1.6'/>",
        "more-horizontal": "<circle fill='COLOR' stroke='none' cx='5.5' cy='12' r='1.6'/><circle fill='COLOR' stroke='none' cx='12' cy='12' r='1.6'/><circle fill='COLOR' stroke='none' cx='18.5' cy='12' r='1.6'/>",
        "maximize": "<path d='M14 4h6v6M10 20H4v-6M20 4l-6.5 6.5M4 20l6.5-6.5'/>",
        "minimize": "<path d='M4 14h6v6M20 10h-6V4M10 14l-6.5 6.5M14 10l6.5-6.5'/>",
        "list": "<path d='M9 6h11M9 12h11M9 18h11'/><circle fill='COLOR' stroke='none' cx='4.5' cy='6' r='1.2'/><circle fill='COLOR' stroke='none' cx='4.5' cy='12' r='1.2'/><circle fill='COLOR' stroke='none' cx='4.5' cy='18' r='1.2'/>",
        "highlighter": "<path d='m9 11-5 5v3h3l5-5'/><path d='m9 11 6-6 4 4-6 6'/><path d='M14 20h6'/>",
        "pen": "<path d='M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z'/>",
        "eye": "<path d='M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z'/><circle cx='12' cy='12' r='3'/>",
        "backlinks": "<path d='M10 7 5 12l5 5'/><path d='M5 12h9a5 5 0 0 1 5 5v1'/>",
        "check-square": "<rect x='3.5' y='3.5' width='17' height='17' rx='2.5'/><path d='m8 12 3 3 5-6'/>",
        "info": "<circle cx='12' cy='12' r='9'/><path d='M12 11v6M12 7.5v.5'/>",
        "export": "<path d='M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z'/><path d='M14 3v5h5'/>",
        "external": "<path d='M14 4h6v6M20 4l-8 8'/><path d='M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4'/>",
        "collapse": "<path d='M4 8h16M4 16h16'/><path d='m9 4 3 3 3-3M9 20l3-3 3 3'/>",
        "hidden": "<path d='M3 3l18 18'/><path d='M10.6 6A9.7 9.7 0 0 1 12 5.5c6 0 9.5 6.5 9.5 6.5a17 17 0 0 1-2.6 3.4M6.2 7.4A17 17 0 0 0 2.5 12S6 18.5 12 18.5a9 9 0 0 0 4-.9'/>",
        "disk": "<rect x='3' y='7' width='18' height='10' rx='2'/><path d='M7 12h.01M11 12h6'/>",
        "logo": "<path fill='COLOR' stroke='none' d='M12 2.5 22 21h-4.4L12 10.6 6.4 21H2z'/><path fill='COLOR' stroke='none' opacity='.55' d='M12 13.6 15.4 21H8.6z'/>",
        "warning": "<path d='M12 4 2.5 20h19z'/><path d='M12 10v4.5M12 17.5v.5'/>",
        "key": "<circle cx='8' cy='15' r='4'/><path d='m11 12 9-9M17 6l3 3M15 8l2 2'/>",
        "moon": "<path d='M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z'/>",
    })

    readonly property string svg: shapes[name]
        ? "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='" + color
          + "' stroke-width='" + weight + "' stroke-linecap='round' stroke-linejoin='round'>"
          + shapes[name].replace(/COLOR/g, color) + "</svg>"
        : ""

    width: size
    height: size
    // Layouts go by the implicit size.
    implicitWidth: size
    implicitHeight: size

    Image {
        anchors.fill: parent
        sourceSize.width: icon.size * 2
        sourceSize.height: icon.size * 2
        fillMode: Image.PreserveAspectFit
        smooth: true
        source: icon.svg ? "data:image/svg+xml;utf8," + encodeURIComponent(icon.svg) : ""
    }
}
