import VoxitHostBridge

extension AuthMethod {
  var label: String {
    switch self {
    case .chatGPTDeviceCode:
      return "Device Code"
    }
  }
}

extension AuthState {
  var label: String {
    switch self {
    case .checking:
      return "Checking"
    case .signedOut:
      return "Signed Out"
    case .signedIn:
      return "Signed In"
    case .busy:
      return "Busy"
    }
  }
}

extension DictationState {
  var label: String {
    switch self {
    case .idle:
      return "Idle"
    case .listening:
      return "Listening"
    case .finalizing:
      return "Finalizing"
    case .rewriting:
      return "Rewriting"
    case .done:
      return "Done"
    }
  }
}

extension HotkeyMode {
  var label: String {
    switch self {
    case .toggle:
      return "Toggle"
    case .hold:
      return "Hold"
    }
  }
}
