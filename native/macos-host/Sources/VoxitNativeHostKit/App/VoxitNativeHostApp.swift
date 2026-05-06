import AppKit
import SwiftUI

public struct VoxitNativeHostApp: App {
  @Environment(\.openWindow) private var openWindow
  @StateObject private var store = HostStore()
  @StateObject private var settingsStore = VoxitSettingsStore()
  @State private var settingsWindowController: VoxitSettingsWindowController?

  public init() {}

  public var body: some Scene {
    Window("Voxit", id: "main") {
      ContentView(store: store)
        .frame(minWidth: 720, minHeight: 460)
        .task {
          VoxitArtwork.applyApplicationIcon()
          await store.reload()
        }
    }
    .commands {
      CommandGroup(replacing: .appSettings) {
        Button("Settings...") {
          presentSettings()
        }
        .keyboardShortcut(",", modifiers: [.command])
      }

      CommandGroup(after: .appInfo) {
        Button("Start Dictation") {}
          .keyboardShortcut(
            settingsStore.settings.dictationHotkeyPresentation.swiftUIKeyEquivalent,
            modifiers: settingsStore.settings.dictationHotkeyPresentation.swiftUIModifiers
          )
          .disabled(true)

        Divider()

        Button("Refresh Status") {
          Task {
            await store.reload()
          }
        }
        .keyboardShortcut("r", modifiers: [.command])
      }
    }

    MenuBarExtra {
      Button("Open Voxit") {
        openWindow(id: "main")
        NSApp.activate(ignoringOtherApps: true)
      }
      .keyboardShortcut("o", modifiers: [.command])

      Button("Start Dictation") {}
        .keyboardShortcut(
          settingsStore.settings.dictationHotkeyPresentation.swiftUIKeyEquivalent,
          modifiers: settingsStore.settings.dictationHotkeyPresentation.swiftUIModifiers
        )
        .disabled(true)

      Divider()

      Button("Settings...") {
        presentSettings()
      }
      .keyboardShortcut(",", modifiers: [.command])

      Button("Refresh Status") {
        Task {
          await store.reload()
        }
      }
      .keyboardShortcut("r", modifiers: [.command])

      Divider()

      Button("Quit Voxit") {
        NSApp.terminate(nil)
      }
      .keyboardShortcut("q", modifiers: [.command])
    } label: {
      Image(nsImage: VoxitArtwork.statusBarImage())
        .renderingMode(.template)
        .foregroundStyle(.primary)
    }
  }

  @MainActor
  private func presentSettings() {
    if settingsWindowController == nil {
      settingsWindowController = VoxitSettingsWindowController(settingsStore: settingsStore) {
        NSApp.setActivationPolicy(.accessory)
      }
    }

    settingsWindowController?.present()
  }
}
