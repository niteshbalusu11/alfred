// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "AlfredAPIClient",
    platforms: [
        .iOS(.v16),
        .macOS(.v12)
    ],
    products: [
        .library(
            name: "AlfredAPIClient",
            targets: ["AlfredAPIClient"]
        )
    ],
    targets: [
        .target(
            name: "AlfredAPIClient",
            path: "Sources"
        )
    ]
)
