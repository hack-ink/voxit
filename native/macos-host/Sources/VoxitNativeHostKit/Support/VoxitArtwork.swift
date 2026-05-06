import AppKit
import Foundation

enum VoxitArtwork {
  static func statusBarImage() -> NSImage {
    let image =
      image(
        named: "StatusBarIcon.png",
        fallbackPath: "assets/tray-icon/generated/tray-icon-template.png")
      ?? NSImage(systemSymbolName: "waveform", accessibilityDescription: "Voxit")
      ?? NSImage(size: NSSize(width: 18, height: 18))
    image.isTemplate = true
    image.size = NSSize(width: 18, height: 18)
    return image
  }

  @MainActor
  static func applyApplicationIcon() {
    guard
      let image = image(
        named: "AppIcon.icns",
        fallbackPath: "assets/app-icon/generated/app-icon.icns"
      )
    else {
      return
    }
    NSApp.applicationIconImage = image
  }

  private static func image(named resourceName: String, fallbackPath: String) -> NSImage? {
    let directResourceURL = Bundle.main.resourceURL?.appendingPathComponent(resourceName)
    let fallbackResourceURL = repositoryRootURL()?.appendingPathComponent(fallbackPath)

    return [directResourceURL, fallbackResourceURL]
      .compactMap { $0 }
      .first(where: { FileManager.default.fileExists(atPath: $0.path) })
      .flatMap { NSImage(contentsOf: $0) }
  }

  private static func repositoryRootURL() -> URL? {
    var url = URL(fileURLWithPath: #filePath)
    for _ in 0..<8 {
      url.deleteLastPathComponent()
      if FileManager.default.fileExists(atPath: url.appendingPathComponent("Cargo.toml").path) {
        return url
      }
    }
    return nil
  }
}
