import Foundation
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

extension PromptProfileKind {
  var label: String {
    switch self {
    case .fastDictation:
      return "Fast Dictation"
    case .messaging:
      return "Messaging"
    case .mail:
      return "Mail"
    case .codeEditor:
      return "Code Editor"
    case .terminal:
      return "Terminal"
    case .workTracker:
      return "Work Tracker"
    }
  }
}

extension VoiceInteractionTier {
  var label: String {
    switch self {
    case .fastDictation:
      return "Fast Dictation"
    case .contextRewrite:
      return "Context Rewrite"
    case .voiceIntent:
      return "Voice Intent"
    }
  }
}

extension VoiceReasoningEffort {
  var label: String {
    switch self {
    case .minimal:
      return "Minimal"
    case .low:
      return "Low"
    case .medium:
      return "Medium"
    case .high:
      return "High"
    }
  }
}

extension VoiceOutputPolicy {
  var label: String {
    switch self {
    case .insertText:
      return "Insert Text"
    case .previewBeforeInsert:
      return "Preview"
    case .confirmBeforeAction:
      return "Confirm"
    }
  }
}

extension HostSnapshot {
  var focusedAppLabel: String {
    if let focusedAppName {
      return focusedAppName
    }
    if let focusedBundleID {
      return focusedBundleID
    }
    if let focusedURLDomain {
      return focusedURLDomain
    }
    return "No Context"
  }

  var recordingSummary: String {
    if recordingDurationMS > 0 {
      return "\(recordingDurationMS) ms"
    }
    if hasRawTranscript || hasFinalOutput {
      return "Completed"
    }
    return "No Runs"
  }

  var pass1TranscriptPreview: String? {
    let committed = pass1CommittedTranscript?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    let draft = pass1DraftTranscript?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

    switch (committed.isEmpty, draft.isEmpty) {
    case (false, false):
      return "\(committed) \(draft)"
    case (false, true):
      return committed
    case (true, false):
      return draft
    case (true, true):
      return nil
    }
  }
}
