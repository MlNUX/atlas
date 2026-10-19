// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

T.ComboBox {
    id: control
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, implicitIndicatorHeight + topPadding + bottomPadding)
    leftPadding: 10
    rightPadding: 10 + (indicator ? indicator.width + spacing : 0)
    spacing: 6
    font.pixelSize: Colors.fontSize
    hoverEnabled: true

    delegate: ItemDelegate {
        required property var model
        required property int index
        width: ListView.view ? ListView.view.width : implicitWidth
        // Plain lists (strings) and lists of objects with `textRole`.
        text: control.textRole ? (Array.isArray(control.model) ? model.modelData[control.textRole] : model[control.textRole]) : model.modelData
        highlighted: control.highlightedIndex === index
        hoverEnabled: control.hoverEnabled
        font: control.font
    }

    indicator: Image {
        x: control.mirrored ? control.padding : control.width - width - 10
        y: control.topPadding + (control.availableHeight - height) / 2
        width: 14
        height: 14
        sourceSize: Qt.size(28, 28)
        source: Colors.icon("<path d='m6 9 6 6 6-6'/>", control.enabled ? Colors.textMuted : Colors.textFaint)
        rotation: control.popup.visible ? 180 : 0
    }

    contentItem: T.TextField {
        topPadding: 6
        bottomPadding: 6
        text: control.editable ? control.editText : control.displayText
        enabled: control.editable
        autoScroll: control.editable
        readOnly: control.down
        inputMethodHints: control.inputMethodHints
        validator: control.validator
        selectByMouse: control.selectTextByMouse
        font: control.font
        color: control.enabled ? Colors.textStrong : Colors.textFaint
        selectionColor: Colors.activeSelection
        selectedTextColor: "white"
        verticalAlignment: Text.AlignVCenter
    }

    background: Rectangle {
        implicitWidth: 140
        implicitHeight: 32
        radius: Colors.radius
        color: control.down || control.popup.visible ? Colors.selection : control.hovered ? Colors.hover : Colors.field
        border.width: control.activeFocus && control.editable ? 1.5 : 1
        border.color: control.visualFocus || (control.activeFocus && control.editable) ? Colors.accent : Colors.border
        opacity: control.enabled ? 1 : 0.6
    }

    popup: T.Popup {
        y: control.height + 4
        width: control.width
        implicitHeight: Math.min(contentItem.implicitHeight + topPadding + bottomPadding, 340)
        padding: 4

        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: control.delegateModel
            currentIndex: control.highlightedIndex
            highlightMoveDuration: 0
            boundsBehavior: Flickable.StopAtBounds
            T.ScrollBar.vertical: ScrollBar {}
        }
        background: Rectangle {
            radius: 10
            color: Colors.popup
            border.color: Colors.borderStrong
        }
    }
}
