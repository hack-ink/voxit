import AppKit
import SwiftUI

enum VoxitSettingsWindowMetrics {
  static let width: CGFloat = 620
  static let minHeight: CGFloat = 336
  static let idealHeight: CGFloat = 396
  static let cornerRadius: CGFloat = 18
}

@MainActor
final class VoxitSettingsViewModel: ObservableObject {
  @Published private(set) var settings: VoxitSettings

  private let settingsStore: VoxitSettingsStore

  init(settingsStore: VoxitSettingsStore) {
    self.settingsStore = settingsStore
    self.settings = settingsStore.settings
  }

  func refresh() {
    settings = settingsStore.settings
  }

  func update(_ mutate: (inout VoxitSettings) -> Void) {
    settingsStore.update(mutate)
    settings = settingsStore.settings
  }

  func restoreDefaults() {
    update { $0 = VoxitSettings.defaults }
  }

  func openMicrophoneSettings() {
    openPrivacySettings(query: "Privacy_Microphone")
  }

  func openAccessibilitySettings() {
    openPrivacySettings(query: "Privacy_Accessibility")
  }

  private func openPrivacySettings(query: String) {
    let modernURLString =
      "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?\(query)"
    if let modernURL = URL(string: modernURLString), NSWorkspace.shared.open(modernURL) {
      return
    }

    let fallbackURLString = "x-apple.systempreferences:com.apple.preference.security?\(query)"
    if let fallbackURL = URL(string: fallbackURLString) {
      NSWorkspace.shared.open(fallbackURL)
    }
  }
}

struct VoxitSettingsView: View {
  @ObservedObject var model: VoxitSettingsViewModel
  @State private var selectedSection: VoxitSettingsSection = .general

  var body: some View {
    HStack(alignment: .top, spacing: 12) {
      SettingsRail(selectedSection: $selectedSection)
        .frame(width: 150)
        .padding(.top, 24)

      SettingsDashboard(
        model: model,
        section: selectedSection,
        restoreDefaults: model.restoreDefaults
      )
      .frame(maxWidth: .infinity, alignment: .topLeading)
    }
    .padding(.top, 12)
    .padding(.horizontal, 14)
    .padding(.bottom, 12)
    .controlSize(.small)
    .frame(
      minWidth: VoxitSettingsWindowMetrics.width,
      idealWidth: VoxitSettingsWindowMetrics.width,
      minHeight: VoxitSettingsWindowMetrics.minHeight,
      idealHeight: VoxitSettingsWindowMetrics.idealHeight
    )
    .background(.regularMaterial)
  }
}

private enum VoxitSettingsSection: String, CaseIterable, Identifiable {
  case general
  case dictation
  case audio
  case permissions
  case about

  var id: Self { self }

  var title: String {
    switch self {
    case .general:
      return "General"
    case .dictation:
      return "Dictation"
    case .audio:
      return "Audio"
    case .permissions:
      return "Permissions"
    case .about:
      return "About"
    }
  }

  var subtitle: String {
    switch self {
    case .general:
      return "Startup"
    case .dictation:
      return "Shortcut"
    case .audio:
      return "Input"
    case .permissions:
      return "Access"
    case .about:
      return "Project"
    }
  }

  var symbolName: String {
    switch self {
    case .general:
      return "switch.2"
    case .dictation:
      return "waveform"
    case .audio:
      return "mic"
    case .permissions:
      return "lock.shield"
    case .about:
      return "info.circle"
    }
  }

  var allowsRestoreDefaults: Bool {
    switch self {
    case .general, .dictation, .audio:
      return true
    case .permissions, .about:
      return false
    }
  }
}

private struct SettingsRail: View {
  @Binding var selectedSection: VoxitSettingsSection

  var body: some View {
    VStack(alignment: .leading, spacing: 14) {
      HStack(spacing: 8) {
        Image(nsImage: NSApp.applicationIconImage)
          .resizable()
          .scaledToFit()
          .frame(width: 28, height: 28)
          .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
        Text("Voxit")
          .font(.system(size: 17, weight: .semibold, design: .rounded))
      }
      .padding(.horizontal, 2)

      VStack(spacing: 5) {
        ForEach(VoxitSettingsSection.allCases) { section in
          SettingsRailButton(
            section: section,
            isSelected: selectedSection == section
          ) {
            selectedSection = section
          }
        }
      }
    }
  }
}

private struct SettingsRailButton: View {
  let section: VoxitSettingsSection
  let isSelected: Bool
  let action: () -> Void
  @State private var isHovered = false

  var body: some View {
    Button(action: action) {
      HStack(spacing: 8) {
        Image(systemName: section.symbolName)
          .symbolRenderingMode(.hierarchical)
          .font(.system(size: 12.5, weight: .semibold))
          .foregroundStyle(isSelected ? Color.accentColor : Color.secondary)
          .frame(width: 23, height: 23)

        VStack(alignment: .leading, spacing: 2) {
          Text(section.title)
            .font(.system(size: 12.5, weight: .semibold))
            .lineLimit(1)
          Text(section.subtitle)
            .font(.system(size: 10, weight: .medium))
            .foregroundStyle(.secondary)
            .lineLimit(1)
        }
        Spacer(minLength: 0)
      }
      .padding(.horizontal, 8)
      .padding(.vertical, 5)
      .frame(maxWidth: .infinity)
      .contentShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
      .background {
        if isSelected {
          RoundedRectangle(cornerRadius: 9, style: .continuous)
            .fill(.primary.opacity(0.06))
          HStack {
            Capsule()
              .fill(Color.accentColor)
              .frame(width: 2, height: 16)
            Spacer()
          }
          .padding(.leading, 1)
        } else if isHovered {
          RoundedRectangle(cornerRadius: 9, style: .continuous)
            .fill(.primary.opacity(0.035))
        }
      }
    }
    .buttonStyle(.plain)
    .onHover { isHovered = $0 }
  }
}

private struct SettingsDashboard: View {
  @ObservedObject var model: VoxitSettingsViewModel
  let section: VoxitSettingsSection
  let restoreDefaults: () -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      HStack(alignment: .firstTextBaseline) {
        VStack(alignment: .leading, spacing: 2) {
          Text(section.title)
            .font(.system(size: 20, weight: .semibold))
          Text(section.subtitle)
            .font(.caption)
            .foregroundStyle(.secondary)
        }
        Spacer()
        if section.allowsRestoreDefaults {
          Button("Defaults", action: restoreDefaults)
        }
      }
      .padding(.top, 10)

      Group {
        switch section {
        case .general:
          GeneralSettingsPane(model: model)
        case .dictation:
          DictationSettingsPane(model: model)
        case .audio:
          AudioSettingsPane(model: model)
        case .permissions:
          PermissionsSettingsPane(model: model)
        case .about:
          AboutSettingsPane()
        }
      }
      .frame(maxWidth: .infinity, alignment: .topLeading)
    }
    .padding(.top, 10)
  }
}

private struct GeneralSettingsPane: View {
  @ObservedObject var model: VoxitSettingsViewModel

  var body: some View {
    SettingsPanel {
      Toggle(
        "Start as menu bar app",
        isOn: settingBinding(\.startHidden)
      )
      Toggle(
        "Paste after transcription",
        isOn: settingBinding(\.pasteAfterTranscription)
      )
      Toggle(
        "Rewrite after transcription",
        isOn: settingBinding(\.rewriteAfterTranscription)
      )
    }
  }

  private func settingBinding(_ keyPath: WritableKeyPath<VoxitSettings, Bool>) -> Binding<Bool> {
    Binding(
      get: { model.settings[keyPath: keyPath] },
      set: { value in
        model.update { $0[keyPath: keyPath] = value }
      }
    )
  }
}

private struct DictationSettingsPane: View {
  @ObservedObject var model: VoxitSettingsViewModel

  var body: some View {
    SettingsPanel {
      Picker("Shortcut", selection: hotkeyBinding) {
        Text("Control-Shift-Space").tag("Control-Shift-Space")
        Text("Option-Space").tag("Option-Space")
        Text("Command-Shift-Space").tag("Command-Shift-Space")
        Text("Control-Option-V").tag("Control-Option-V")
      }
      .pickerStyle(.menu)

      Picker("Mode", selection: hotkeyModeBinding) {
        ForEach(VoxitHotkeyModePreference.allCases) { mode in
          Text(mode.title).tag(mode)
        }
      }
      .pickerStyle(.segmented)

      Picker("Auth", selection: authRouteBinding) {
        ForEach(VoxitAuthRoutePreference.allCases) { route in
          Text(route.title).tag(route)
        }
      }
      .pickerStyle(.menu)
    }
  }

  private var hotkeyBinding: Binding<String> {
    Binding(
      get: { model.settings.dictationHotkey },
      set: { value in
        model.update { $0.dictationHotkey = value }
      }
    )
  }

  private var hotkeyModeBinding: Binding<VoxitHotkeyModePreference> {
    Binding(
      get: { model.settings.hotkeyMode },
      set: { value in
        model.update { $0.hotkeyMode = value }
      }
    )
  }

  private var authRouteBinding: Binding<VoxitAuthRoutePreference> {
    Binding(
      get: { model.settings.authRoute },
      set: { value in
        model.update { $0.authRoute = value }
      }
    )
  }
}

private struct AudioSettingsPane: View {
  @ObservedObject var model: VoxitSettingsViewModel

  var body: some View {
    SettingsPanel {
      Picker("Input", selection: audioInputBinding) {
        ForEach(VoxitAudioInputPreference.allCases) { input in
          Text(input.title).tag(input)
        }
      }
      .pickerStyle(.menu)

      LabeledContent("Target rate", value: "24 kHz")
      LabeledContent("Channels", value: "Mono")
    }
  }

  private var audioInputBinding: Binding<VoxitAudioInputPreference> {
    Binding(
      get: { model.settings.audioInput },
      set: { value in
        model.update { $0.audioInput = value }
      }
    )
  }
}

private struct PermissionsSettingsPane: View {
  @ObservedObject var model: VoxitSettingsViewModel

  var body: some View {
    SettingsPanel {
      HStack {
        LabeledContent("Microphone", value: "Required")
        Button("Open") {
          model.openMicrophoneSettings()
        }
      }

      HStack {
        LabeledContent("Accessibility", value: "Optional")
        Button("Open") {
          model.openAccessibilitySettings()
        }
      }
    }
  }
}

private struct AboutSettingsPane: View {
  var body: some View {
    SettingsPanel {
      LabeledContent("App", value: "Voxit")
      LabeledContent("Host", value: "Swift + Rust Core FFI")
      LabeledContent("Auth", value: "ChatGPT Device Code")
    }
  }
}

private struct SettingsPanel<Content: View>: View {
  @ViewBuilder var content: Content

  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      content
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(14)
    .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
  }
}
