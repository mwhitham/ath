// swift-tools-version: 6.0
// An example Swift client for ath-host, the optional Athletic Standard host program.
// See README.md in this folder.

import PackageDescription

let package = Package(
    name: "AthleticStandardHost",
    platforms: [.macOS(.v14), .iOS(.v17)],
    products: [
        .library(name: "AthleticStandardHost", targets: ["AthleticStandardHost"]),
        .executable(name: "host-example", targets: ["host-example"]),
    ],
    targets: [
        .target(name: "AthleticStandardHost"),
        .executableTarget(name: "host-example", dependencies: ["AthleticStandardHost"]),
        .testTarget(name: "AthleticStandardHostTests", dependencies: ["AthleticStandardHost"]),
    ]
)
