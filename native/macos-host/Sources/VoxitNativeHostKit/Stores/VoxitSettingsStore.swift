import AppKit
import Foundation
import VoxitHostBridge

@MainActor
final class VoxitSettingsStore: ObservableObject {
  static let didChangeNotification = Notification.Name("VoxitSettingsDidChange")

  @Published private(set) var settings: VoxitSettings

  private enum DefaultsKey {
    static let dictationHotkey = "dictationHotkey"
    static let hotkeyMode = "hotkeyMode"
    static let startHidden = "startHidden"
    static let pasteAfterTranscription = "pasteAfterTranscription"
    static let rewriteAfterTranscription = "rewriteAfterTranscription"
    static let authRoute = "authRoute"
    static let audioInput = "audioInput"
    static let realtimeModel = "realtimeModel"
    static let realtimeTranscriptionModel = "realtimeTranscriptionModel"
    static let finalizeModel = "finalizeModel"
    static let rewriteModel = "rewriteModel"
  }

  private let defaults: UserDefaults
  private var syncHandler: ((VoxitSettings) -> Void)?

  init(defaults: UserDefaults = .standard) {
    self.defaults = defaults
    let baseSettings = VoxitSettings.defaults
    let settings = VoxitSettings(
      dictationHotkey: defaults.string(forKey: DefaultsKey.dictationHotkey)
        ?? baseSettings.dictationHotkey,
      hotkeyMode: VoxitHotkeyModePreference(
        rawValue: defaults.string(forKey: DefaultsKey.hotkeyMode) ?? "")
        ?? baseSettings.hotkeyMode,
      startHidden: defaults.object(forKey: DefaultsKey.startHidden) as? Bool
        ?? baseSettings.startHidden,
      pasteAfterTranscription: defaults.object(forKey: DefaultsKey.pasteAfterTranscription)
        as? Bool
        ?? baseSettings.pasteAfterTranscription,
      rewriteAfterTranscription: defaults.object(forKey: DefaultsKey.rewriteAfterTranscription)
        as? Bool
        ?? baseSettings.rewriteAfterTranscription,
      authRoute: VoxitAuthRoutePreference(
        rawValue: defaults.string(forKey: DefaultsKey.authRoute) ?? "")
        ?? baseSettings.authRoute,
      audioInput: VoxitAudioInputPreference(
        rawValue: defaults.string(forKey: DefaultsKey.audioInput) ?? "")
        ?? baseSettings.audioInput,
      realtimeModel: defaults.string(forKey: DefaultsKey.realtimeModel)
        ?? baseSettings.realtimeModel,
      realtimeTranscriptionModel: defaults.string(
        forKey: DefaultsKey.realtimeTranscriptionModel)
        ?? baseSettings.realtimeTranscriptionModel,
      finalizeModel: defaults.string(forKey: DefaultsKey.finalizeModel)
        ?? baseSettings.finalizeModel,
      rewriteModel: defaults.string(forKey: DefaultsKey.rewriteModel)
        ?? baseSettings.rewriteModel
    )
    self.settings = settings.sanitized()
    Self.persist(self.settings, into: defaults)
  }

  func update(_ mutate: (inout VoxitSettings) -> Void) {
    var next = settings
    mutate(&next)
    let sanitized = next.sanitized()
    settings = sanitized
    Self.persist(sanitized, into: defaults)
    syncHandler?(sanitized)
    NotificationCenter.default.post(name: Self.didChangeNotification, object: self)
  }

  func setSyncHandler(_ syncHandler: @escaping (VoxitSettings) -> Void) {
    self.syncHandler = syncHandler
  }

  private static func persist(_ settings: VoxitSettings, into defaults: UserDefaults) {
    defaults.set(settings.dictationHotkey, forKey: DefaultsKey.dictationHotkey)
    defaults.set(settings.hotkeyMode.rawValue, forKey: DefaultsKey.hotkeyMode)
    defaults.set(settings.startHidden, forKey: DefaultsKey.startHidden)
    defaults.set(settings.pasteAfterTranscription, forKey: DefaultsKey.pasteAfterTranscription)
    defaults.set(settings.rewriteAfterTranscription, forKey: DefaultsKey.rewriteAfterTranscription)
    defaults.set(settings.authRoute.rawValue, forKey: DefaultsKey.authRoute)
    defaults.set(settings.audioInput.rawValue, forKey: DefaultsKey.audioInput)
    defaults.set(settings.realtimeModel, forKey: DefaultsKey.realtimeModel)
    defaults.set(
      settings.realtimeTranscriptionModel,
      forKey: DefaultsKey.realtimeTranscriptionModel
    )
    defaults.set(settings.finalizeModel, forKey: DefaultsKey.finalizeModel)
    defaults.set(settings.rewriteModel, forKey: DefaultsKey.rewriteModel)
  }
}

struct VoxitSettings: Equatable {
  var dictationHotkey: String
  var hotkeyMode: VoxitHotkeyModePreference
  var startHidden: Bool
  var pasteAfterTranscription: Bool
  var rewriteAfterTranscription: Bool
  var authRoute: VoxitAuthRoutePreference
  var audioInput: VoxitAudioInputPreference
  var realtimeModel: String
  var realtimeTranscriptionModel: String
  var finalizeModel: String
  var rewriteModel: String

  static var defaults: Self {
    Self(
      dictationHotkey: "Control-Shift-Space",
      hotkeyMode: .toggle,
      startHidden: true,
      pasteAfterTranscription: true,
      rewriteAfterTranscription: true,
      authRoute: .chatGPTDeviceCode,
      audioInput: .systemDefault,
      realtimeModel: "gpt-realtime-2",
      realtimeTranscriptionModel: "gpt-4o-mini-transcribe",
      finalizeModel: "gpt-4o-transcribe",
      rewriteModel: "gpt-5.2-mini"
    )
  }

  var dictationHotkeyPresentation: VoxitHotkeyPresentation {
    Self.dictationHotkeyPresentation(for: dictationHotkey)
  }

  func sanitized() -> Self {
    var copy = self
    copy.dictationHotkey =
      Self.dictationHotkeyPresentation(for: copy.dictationHotkey)
      .displayTitle
    copy.realtimeModel = Self.sanitizedModelID(
      copy.realtimeModel,
      fallback: Self.defaults.realtimeModel
    )
    copy.realtimeTranscriptionModel = Self.sanitizedModelID(
      copy.realtimeTranscriptionModel,
      fallback: Self.defaults.realtimeTranscriptionModel
    )
    copy.finalizeModel = Self.sanitizedModelID(
      copy.finalizeModel,
      fallback: Self.defaults.finalizeModel
    )
    copy.rewriteModel = Self.sanitizedModelID(
      copy.rewriteModel,
      fallback: Self.defaults.rewriteModel
    )
    return copy
  }

  static func dictationHotkeyPresentation(for raw: String) -> VoxitHotkeyPresentation {
    parseHotkeyPresentation(raw)
      ?? parseHotkeyPresentation(defaults.dictationHotkey)
      ?? VoxitHotkeyPresentation(
        displayTitle: "Control-Shift-Space",
        keyEquivalent: " ",
        modifierMask: [.control, .shift]
      )
  }

  private static func parseHotkeyPresentation(_ raw: String) -> VoxitHotkeyPresentation? {
    let tokens = hotkeyTokens(from: raw)
    guard tokens.isEmpty == false else {
      return nil
    }

    var modifiers = NSEvent.ModifierFlags()
    var keyEquivalent: String?
    for token in tokens {
      switch token.lowercased() {
      case "alt", "option":
        modifiers.insert(.option)
      case "ctrl", "control":
        modifiers.insert(.control)
      case "shift":
        modifiers.insert(.shift)
      case "cmd", "command", "super", "meta", "win":
        modifiers.insert(.command)
      default:
        keyEquivalent = normalizedMenuKeyEquivalent(for: token)
      }
    }

    guard let keyEquivalent else {
      return nil
    }

    var titleParts: [String] = []
    if modifiers.contains(.control) {
      titleParts.append("Control")
    }
    if modifiers.contains(.option) {
      titleParts.append("Option")
    }
    if modifiers.contains(.shift) {
      titleParts.append("Shift")
    }
    if modifiers.contains(.command) {
      titleParts.append("Command")
    }
    titleParts.append(displayTitle(for: keyEquivalent))

    return VoxitHotkeyPresentation(
      displayTitle: titleParts.joined(separator: "-"),
      keyEquivalent: keyEquivalent,
      modifierMask: modifiers
    )
  }

  private static func normalizedMenuKeyEquivalent(for token: String) -> String? {
    let normalized = token.lowercased()
    let key = normalized.hasPrefix("key") ? String(normalized.dropFirst(3)) : normalized
    switch key {
    case "space":
      return " "
    default:
      guard key.count == 1, key.unicodeScalars.allSatisfy(\.isASCII) else {
        return nil
      }
      return key
    }
  }

  private static func displayTitle(for keyEquivalent: String) -> String {
    keyEquivalent == " " ? "Space" : keyEquivalent.uppercased()
  }

  private static func hotkeyTokens(from raw: String) -> [String] {
    raw
      .split { character in
        character == "+" || character == "-"
      }
      .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
      .filter { $0.isEmpty == false }
  }

  private static func sanitizedModelID(_ raw: String, fallback: String) -> String {
    let modelID = raw.trimmingCharacters(in: .whitespacesAndNewlines)

    return modelID.isEmpty ? fallback : modelID
  }
}

struct VoxitHotkeyPresentation: Equatable {
  let displayTitle: String
  let keyEquivalent: String
  let modifierMask: NSEvent.ModifierFlags
}

enum VoxitHotkeyModePreference: String, CaseIterable, Identifiable {
  case toggle
  case hold

  var id: Self { self }

  var title: String {
    switch self {
    case .toggle:
      return "Toggle"
    case .hold:
      return "Hold"
    }
  }

  var hostBridgeValue: HotkeyMode {
    switch self {
    case .toggle:
      return .toggle
    case .hold:
      return .hold
    }
  }
}

enum VoxitAuthRoutePreference: String, CaseIterable, Identifiable {
  case chatGPTDeviceCode

  var id: Self { self }

  var title: String {
    switch self {
    case .chatGPTDeviceCode:
      return "ChatGPT Device Code"
    }
  }
}

enum VoxitAudioInputPreference: String, CaseIterable, Identifiable {
  case systemDefault

  var id: Self { self }

  var title: String {
    switch self {
    case .systemDefault:
      return "System Default"
    }
  }
}
