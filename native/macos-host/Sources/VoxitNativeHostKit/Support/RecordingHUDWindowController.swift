import AppKit
import SwiftUI

@MainActor
final class RecordingHUDWindowController: NSWindowController, NSWindowDelegate {
  private let store: HostStore

  init(store: HostStore) {
    self.store = store

    let contentRect = NSRect(x: 0, y: 0, width: 380, height: 220)
    let panel = NSPanel(
      contentRect: contentRect,
      styleMask: [.titled, .closable, .hudWindow, .nonactivatingPanel, .fullSizeContentView],
      backing: .buffered,
      defer: false
    )
    panel.title = "Voxit Recording"
    panel.titleVisibility = .hidden
    panel.titlebarAppearsTransparent = true
    panel.isReleasedWhenClosed = false
    panel.hidesOnDeactivate = false
    panel.level = .floating
    panel.collectionBehavior = [.canJoinAllSpaces, .moveToActiveSpace, .transient]

    super.init(window: panel)

    panel.delegate = self
    panel.contentViewController = NSHostingController(rootView: RecordingHUDView(store: store))
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) {
    fatalError("init(coder:) has not been implemented")
  }

  func present() {
    guard let window else {
      return
    }

    positionNearTopTrailing(window)
    showWindow(nil)
    window.orderFrontRegardless()
  }

  private func positionNearTopTrailing(_ window: NSWindow) {
    let visibleFrame = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1_280, height: 720)
    let frame = window.frame
    let origin = NSPoint(
      x: visibleFrame.maxX - frame.width - 24,
      y: visibleFrame.maxY - frame.height - 24
    )

    window.setFrameOrigin(origin)
  }
}
