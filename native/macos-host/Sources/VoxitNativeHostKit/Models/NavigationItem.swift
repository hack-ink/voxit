import Foundation

public enum NavigationItem: String, CaseIterable, Identifiable, Sendable {
  case dictation
  case auth
  case audio

  public var id: String {
    rawValue
  }

  var title: String {
    switch self {
    case .dictation:
      return "Dictation"
    case .auth:
      return "ChatGPT"
    case .audio:
      return "Audio"
    }
  }

  var systemImage: String {
    switch self {
    case .dictation:
      return "waveform"
    case .auth:
      return "person.crop.circle.badge.checkmark"
    case .audio:
      return "mic"
    }
  }
}
