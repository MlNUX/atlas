// swift-tools-version: 6.2
// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import PackageDescription

// The Rust core for the macOS app: UniFFI bindings of atlas-api (module
// `atlas`, generated) plus Swift helpers. AtlasFFI.xcframework and
// Sources/AtlasKit/atlas.swift come from apps/macos/scripts/build-core.sh
// and are not checked in.
let package = Package(
    name: "AtlasKit",
    platforms: [.macOS(.v26)],
    products: [
        .library(name: "AtlasKit", targets: ["AtlasKit"])
    ],
    targets: [
        .binaryTarget(name: "AtlasFFI", path: "AtlasFFI.xcframework"),
        .target(
            name: "AtlasKit",
            dependencies: ["AtlasFFI"],
            linkerSettings: [
                // What the Rust dependencies need (TLS, file events, keychain).
                .linkedFramework("Security"),
                .linkedFramework("CoreFoundation"),
                .linkedFramework("CoreServices"),
                .linkedFramework("SystemConfiguration"),
                .linkedLibrary("iconv"),
            ]
        ),
        .testTarget(name: "AtlasKitTests", dependencies: ["AtlasKit"]),
    ]
)
