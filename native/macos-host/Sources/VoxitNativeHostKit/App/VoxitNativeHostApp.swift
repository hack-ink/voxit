import AppKit
import SwiftUI

public struct VoxitNativeHostApp: App {
  @Environment(\.openWindow) private var openWindow
  @StateObject private var store = HostStore()
  @StateObject private var settingsStore = VoxitSettingsStore()
  @StateObject private var hotkeyMonitor = GlobalHotkeyMonitor()
  @State private var settingsWindowController: VoxitSettingsWindowController?
  @State private var recordingHUDWindowController: RecordingHUDWindowController?

  public init() {}

  public var body: some Scene {
    Window("Voxit", id: "main") {
      ContentView(store: store)
        .frame(minWidth: 720, minHeight: 460)
        .task {
          VoxitArtwork.applyApplicationIcon()
          configureSettingsSync()
          configureHotkeyMonitor()
          await store.reload()
          await store.savePreferences(settingsStore.settings)
          await store.setGlossary(UserDefaults.standard.string(forKey: "glossaryTerms") ?? "")
          let profileOverrideRaw =
            UserDefaults.standard.string(forKey: "profileOverride") ?? ProfileOverride.auto.rawValue
          await store.setProfileOverride(ProfileOverride(rawValue: profileOverrideRaw)?.profileKind)
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
        Button("Start Dictation") {
          startDictation()
        }

        Button("Stop Dictation") {
          Task {
            await store.stopDictation()
          }
        }
        .keyboardShortcut(".", modifiers: [.command])
        .disabled(store.snapshot?.dictationState != .listening)

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

      Button("Start Dictation") {
        startDictation()
      }

      Button("Stop Dictation") {
        Task {
          await store.stopDictation()
        }
      }
      .disabled(store.snapshot?.dictationState != .listening)

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
  private func configureSettingsSync() {
    settingsStore.setSyncHandler { settings in
      Task { @MainActor in
        await store.savePreferences(settings)
        configureHotkeyMonitor()
      }
    }
  }

  @MainActor
  private func configureHotkeyMonitor() {
    hotkeyMonitor.configure(
      settings: settingsStore.settings,
      keyDown: {
        handleHotkeyDown()
      },
      keyUp: {
        handleHotkeyUp()
      }
    )
  }

  @MainActor
  private func startDictation() {
    presentRecordingHUD()
    Task {
      await store.startDictation()
    }
  }

  @MainActor
  private func handleHotkeyDown() {
    presentRecordingHUD()

    if settingsStore.settings.hotkeyMode == .hold {
      guard store.snapshot?.dictationState != .listening else {
        return
      }

      Task {
        await store.startDictation()
      }
    } else if store.snapshot?.dictationState == .listening {
      Task {
        await store.stopDictation()
      }
    } else {
      Task {
        await store.startDictation()
      }
    }
  }

  @MainActor
  private func handleHotkeyUp() {
    guard settingsStore.settings.hotkeyMode == .hold,
      store.snapshot?.dictationState == .listening
    else {
      return
    }

    Task {
      await store.stopDictation()
    }
  }

  @MainActor
  private func presentRecordingHUD() {
    if recordingHUDWindowController == nil {
      recordingHUDWindowController = RecordingHUDWindowController(store: store)
    }

    recordingHUDWindowController?.present()
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
