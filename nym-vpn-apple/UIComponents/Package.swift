// swift-tools-version: 5.9
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "UIComponents",
    defaultLocalization: "en",
    platforms: [
        .iOS(.v16),
        .macOS(.v13)
    ],
    products: [
        .library(
            name: "UIComponents",
            targets: ["UIComponents"]
        )
    ],
    dependencies: [
        .package(path: "../Services"),
        .package(path: "../Theme"),
        .package(url: "https://github.com/airbnb/lottie-spm.git", from: "4.5.0")
    ],
    targets: [
        .target(
            name: "UIComponents",
            dependencies: [
                "Theme",
                .product(name: "ConnectionManager", package: "Services"),
                .product(name: "CountriesManager", package: "Services"),
                .product(name: "ConfigurationManager", package: "Services"),
                .product(name: "Modifiers", package: "Services"),
                .product(name: "Lottie", package: "lottie-spm")
            ],
            resources: [
                .process("Resources/Assets.xcassets")
            ]
        ),
        .testTarget(
            name: "UIComponentsTests",
            dependencies: ["UIComponents"]
        )
    ]
)
