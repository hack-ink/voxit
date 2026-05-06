// swift-tools-version: 6.0

import Foundation
import PackageDescription

let packageRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let repoRoot = packageRoot.deletingLastPathComponent().deletingLastPathComponent()
let defaultRustLibDir = repoRoot.appendingPathComponent("target/debug").path
let rustLibDir = ProcessInfo.processInfo.environment["VOXIT_HOST_FFI_LIB_DIR"] ?? defaultRustLibDir

let package = Package(
	name: "VoxitNativeHost",
	platforms: [
		.macOS(.v14),
	],
	products: [
		.library(name: "VoxitHostBridge", targets: ["VoxitHostBridge"]),
		.library(name: "VoxitNativeHostKit", targets: ["VoxitNativeHostKit"]),
		.executable(name: "VoxitHostBridgeProbe", targets: ["VoxitHostBridgeProbe"]),
		.executable(name: "VoxitNativeHost", targets: ["VoxitNativeHost"]),
	],
	targets: [
		.systemLibrary(
			name: "CVoxitHostFFI",
			path: "Sources/CVoxitHostFFI"
		),
		.target(
			name: "VoxitHostBridge",
			dependencies: ["CVoxitHostFFI"],
			linkerSettings: [
				.linkedFramework("AppKit"),
				.linkedFramework("AudioToolbox"),
				.linkedFramework("CoreAudio"),
				.linkedFramework("Security"),
				.unsafeFlags([
					"-L",
					rustLibDir,
					"-lvoxit_host_ffi",
				]),
			]
		),
		.target(
			name: "VoxitNativeHostKit",
			dependencies: ["VoxitHostBridge"],
			linkerSettings: [
				.linkedFramework("AppKit"),
				.linkedFramework("SwiftUI"),
			]
		),
		.executableTarget(
			name: "VoxitHostBridgeProbe",
			dependencies: ["VoxitHostBridge"]
		),
		.executableTarget(
			name: "VoxitNativeHost",
			dependencies: ["VoxitNativeHostKit"]
		),
	]
)
