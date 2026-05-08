import Foundation

public enum NavigationItem: String, CaseIterable, Identifiable, Sendable {
  case activity
  case appRules
  case profiles
  case glossary
  case promptLab

  public var id: String {
    rawValue
  }

  var title: String {
    switch self {
    case .activity:
      return "Activity"
    case .appRules:
      return "App Rules"
    case .profiles:
      return "Profiles"
    case .glossary:
      return "Glossary"
    case .promptLab:
      return "Prompt Lab"
    }
  }

  var systemImage: String {
    switch self {
    case .activity:
      return "waveform"
    case .appRules:
      return "rectangle.3.group"
    case .profiles:
      return "person.text.rectangle"
    case .glossary:
      return "text.book.closed"
    case .promptLab:
      return "slider.horizontal.3"
    }
  }
}
