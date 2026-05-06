import AppKit
import SwiftUI

private final class VoxitSettingsWindow: NSWindow {
  override var canBecomeKey: Bool {
    true
  }

  override var canBecomeMain: Bool {
    true
  }

  override func performKeyEquivalent(with event: NSEvent) -> Bool {
    if handleCommandShortcut(event) {
      return true
    }
    return super.performKeyEquivalent(with: event)
  }

  override func keyDown(with event: NSEvent) {
    if handleCommandShortcut(event) {
      return
    }
    super.keyDown(with: event)
  }

  private func handleCommandShortcut(_ event: NSEvent) -> Bool {
    let commandModifiers = event.modifierFlags.intersection([
      .command, .option, .control, .shift,
    ])
    guard commandModifiers == .command,
      let character = event.charactersIgnoringModifiers?.lowercased()
    else {
      return false
    }

    switch character {
    case "w":
      performClose(nil)
      return true
    case "q":
      NSApp.terminate(nil)
      return true
    default:
      return false
    }
  }
}

@MainActor
final class VoxitSettingsWindowController: NSWindowController, NSWindowDelegate {
  private let viewModel: VoxitSettingsViewModel
  private let onClose: () -> Void

  init(settingsStore: VoxitSettingsStore, onClose: @escaping () -> Void = {}) {
    self.viewModel = VoxitSettingsViewModel(settingsStore: settingsStore)
    self.onClose = onClose

    let contentRect = NSRect(
      x: 0,
      y: 0,
      width: VoxitSettingsWindowMetrics.width,
      height: VoxitSettingsWindowMetrics.idealHeight
    )
    let window = VoxitSettingsWindow(
      contentRect: contentRect,
      styleMask: [.titled, .closable, .miniaturizable, .fullSizeContentView],
      backing: .buffered,
      defer: false
    )
    window.title = "Settings"
    window.titleVisibility = .hidden
    window.titlebarAppearsTransparent = true
    window.backgroundColor = .clear
    window.isOpaque = false
    if #available(macOS 11.0, *) {
      window.titlebarSeparatorStyle = .none
    }
    window.isReleasedWhenClosed = false
    window.contentMinSize = NSSize(
      width: VoxitSettingsWindowMetrics.width,
      height: VoxitSettingsWindowMetrics.minHeight
    )
    window.collectionBehavior.insert(.moveToActiveSpace)

    super.init(window: window)

    window.delegate = self
    let hostingController = NSHostingController(rootView: VoxitSettingsView(model: viewModel))
    hostingController.view.wantsLayer = true
    hostingController.view.layer?.backgroundColor = NSColor.clear.cgColor
    window.contentViewController = hostingController
    window.center()
    viewModel.refresh()
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) {
    fatalError("init(coder:) has not been implemented")
  }

  func present() {
    NSApp.setActivationPolicy(.regular)
    viewModel.refresh()
    showWindow(nil)
    NSRunningApplication.current.activate(options: [.activateAllWindows])
    window?.makeKeyAndOrderFront(nil)
    window?.invalidateShadow()
    NSApp.activate(ignoringOtherApps: true)
  }

  func windowWillClose(_: Notification) {
    onClose()
  }
}
