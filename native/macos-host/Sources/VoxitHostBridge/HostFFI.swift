import CVoxitHostFFI
import Foundation

public enum HostPlatform: Equatable, Sendable {
  case macOS
  case unsupported
}

public enum AuthMethod: Equatable, Sendable {
  case chatGPTDeviceCode
}

public enum AuthState: Equatable, Sendable {
  case checking
  case signedOut
  case signedIn
  case busy
}

public enum DictationState: Equatable, Sendable {
  case idle
  case listening
  case finalizing
  case rewriting
  case done
}

public enum HotkeyMode: Equatable, Sendable {
  case toggle
  case hold
}

public enum PromptProfileKind: Equatable, Sendable {
  case fastDictation
  case messaging
  case mail
  case codeEditor
  case terminal
  case workTracker
}

public enum VoiceInteractionTier: Equatable, Sendable {
  case fastDictation
  case contextRewrite
  case voiceIntent
}

public enum VoiceReasoningEffort: Equatable, Sendable {
  case minimal
  case low
  case medium
  case high
}

public enum VoiceOutputPolicy: Equatable, Sendable {
  case insertText
  case previewBeforeInsert
  case confirmBeforeAction
}

public struct HostSnapshot: Equatable, Sendable {
  public var platform: HostPlatform
  public var authMethod: AuthMethod
  public var authState: AuthState
  public var dictationState: DictationState
  public var hotkeyMode: HotkeyMode
  public var panelWidth: Int
  public var panelHeight: Int
  public var rewriteEnabled: Bool
  public var hasFocusedContext: Bool
  public var selectedTextPresent: Bool
  public var hasRawTranscript: Bool
  public var hasFinalOutput: Bool
  public var hasError: Bool
  public var recordingDurationMS: UInt64
  public var focusedBundleID: String?
  public var focusedAppName: String?
  public var focusedWindowTitle: String?
  public var focusedURLDomain: String?
  public var focusedElementRole: String?
  public var promptProfileID: String?
  public var promptDirective: String?
  public var rawTranscript: String?
  public var finalOutput: String?
  public var lastError: String?
  public var promptProfileKind: PromptProfileKind
  public var voiceTier: VoiceInteractionTier
  public var reasoningEffort: VoiceReasoningEffort
  public var outputPolicy: VoiceOutputPolicy

  public init(
    platform: HostPlatform,
    authMethod: AuthMethod,
    authState: AuthState,
    dictationState: DictationState,
    hotkeyMode: HotkeyMode,
    panelWidth: Int,
    panelHeight: Int,
    rewriteEnabled: Bool,
    hasFocusedContext: Bool,
    selectedTextPresent: Bool,
    hasRawTranscript: Bool,
    hasFinalOutput: Bool,
    hasError: Bool,
    recordingDurationMS: UInt64,
    focusedBundleID: String?,
    focusedAppName: String?,
    focusedWindowTitle: String?,
    focusedURLDomain: String?,
    focusedElementRole: String?,
    promptProfileID: String?,
    promptDirective: String?,
    rawTranscript: String?,
    finalOutput: String?,
    lastError: String?,
    promptProfileKind: PromptProfileKind,
    voiceTier: VoiceInteractionTier,
    reasoningEffort: VoiceReasoningEffort,
    outputPolicy: VoiceOutputPolicy
  ) {
    self.platform = platform
    self.authMethod = authMethod
    self.authState = authState
    self.dictationState = dictationState
    self.hotkeyMode = hotkeyMode
    self.panelWidth = panelWidth
    self.panelHeight = panelHeight
    self.rewriteEnabled = rewriteEnabled
    self.hasFocusedContext = hasFocusedContext
    self.selectedTextPresent = selectedTextPresent
    self.hasRawTranscript = hasRawTranscript
    self.hasFinalOutput = hasFinalOutput
    self.hasError = hasError
    self.recordingDurationMS = recordingDurationMS
    self.focusedBundleID = focusedBundleID
    self.focusedAppName = focusedAppName
    self.focusedWindowTitle = focusedWindowTitle
    self.focusedURLDomain = focusedURLDomain
    self.focusedElementRole = focusedElementRole
    self.promptProfileID = promptProfileID
    self.promptDirective = promptDirective
    self.rawTranscript = rawTranscript
    self.finalOutput = finalOutput
    self.lastError = lastError
    self.promptProfileKind = promptProfileKind
    self.voiceTier = voiceTier
    self.reasoningEffort = reasoningEffort
    self.outputPolicy = outputPolicy
  }
}

public enum HostBridgeError: Error, Equatable, CustomStringConvertible {
  case abiVersionMismatch(expected: UInt32, actual: UInt32)
  case sessionCreationFailed
  case ffiStatus(context: String, code: UInt32)
  case invalidPlatform(UInt32)
  case invalidAuthMethod(UInt32)
  case invalidAuthState(UInt32)
  case invalidDictationState(UInt32)
  case invalidHotkeyMode(UInt32)
  case invalidPromptProfileKind(UInt32)
  case invalidVoiceInteractionTier(UInt32)
  case invalidVoiceReasoningEffort(UInt32)
  case invalidVoiceOutputPolicy(UInt32)

  public var description: String {
    switch self {
    case .abiVersionMismatch(let expected, let actual):
      return "FFI ABI mismatch: expected \(expected), got \(actual)"
    case .sessionCreationFailed:
      return "Failed to create Voxit host session"
    case .ffiStatus(let context, let code):
      return "FFI status \(code) while \(context)"
    case .invalidPlatform(let rawValue):
      return "Unknown platform \(rawValue)"
    case .invalidAuthMethod(let rawValue):
      return "Unknown auth method \(rawValue)"
    case .invalidAuthState(let rawValue):
      return "Unknown auth state \(rawValue)"
    case .invalidDictationState(let rawValue):
      return "Unknown dictation state \(rawValue)"
    case .invalidHotkeyMode(let rawValue):
      return "Unknown hotkey mode \(rawValue)"
    case .invalidPromptProfileKind(let rawValue):
      return "Unknown prompt profile kind \(rawValue)"
    case .invalidVoiceInteractionTier(let rawValue):
      return "Unknown voice interaction tier \(rawValue)"
    case .invalidVoiceReasoningEffort(let rawValue):
      return "Unknown voice reasoning effort \(rawValue)"
    case .invalidVoiceOutputPolicy(let rawValue):
      return "Unknown voice output policy \(rawValue)"
    }
  }
}

public final class VoxitHostSession {
  private let handle: OpaquePointer

  public init() throws {
    let actualAbi = voxit_host_ffi_abi_version()
    if actualAbi != VOXIT_HOST_FFI_ABI_VERSION {
      throw HostBridgeError.abiVersionMismatch(
        expected: VOXIT_HOST_FFI_ABI_VERSION,
        actual: actualAbi
      )
    }

    let config = VoxitHostConfig(platform: VOXIT_PLATFORM_MACOS)
    guard let handle = voxit_host_session_create(config) else {
      throw HostBridgeError.sessionCreationFailed
    }

    self.handle = handle
  }

  deinit {
    voxit_host_session_destroy(handle)
  }

  public func currentSnapshot() throws -> HostSnapshot {
    var outSnapshot = VoxitHostSnapshot()
    try requireOk(
      voxit_host_session_copy_snapshot(handle, &outSnapshot),
      context: "copying host snapshot"
    )

    return try decode(snapshot: outSnapshot)
  }

  public func refreshFocusedContext() throws -> HostSnapshot {
    try requireOk(
      voxit_host_session_refresh_focused_context(handle),
      context: "refreshing focused context"
    )

    return try currentSnapshot()
  }

  public func startDictation() throws -> HostSnapshot {
    try requireOk(voxit_host_session_start_dictation(handle), context: "starting dictation")

    return try currentSnapshot()
  }

  public func stopDictation() throws -> HostSnapshot {
    try requireOk(voxit_host_session_stop_dictation(handle), context: "stopping dictation")

    return try currentSnapshot()
  }

  public func pasteFinalOutput() throws -> HostSnapshot {
    try requireOk(voxit_host_session_paste_final_output(handle), context: "pasting final output")

    return try currentSnapshot()
  }

  private func requireOk(_ status: VoxitStatus, context: String) throws {
    let code = voxit_status_code(status)
    if code != 0 {
      throw HostBridgeError.ffiStatus(context: context, code: code)
    }
  }

  private func decode(snapshot: VoxitHostSnapshot) throws -> HostSnapshot {
    HostSnapshot(
      platform: try decode(platform: snapshot.platform),
      authMethod: try decode(authMethod: snapshot.auth_method),
      authState: try decode(authState: snapshot.auth_state),
      dictationState: try decode(dictationState: snapshot.dictation_state),
      hotkeyMode: try decode(hotkeyMode: snapshot.hotkey_mode),
      panelWidth: Int(snapshot.panel_width_px),
      panelHeight: Int(snapshot.panel_height_px),
      rewriteEnabled: snapshot.rewrite_enabled != 0,
      hasFocusedContext: snapshot.has_focused_context != 0,
      selectedTextPresent: snapshot.selected_text_present != 0,
      hasRawTranscript: snapshot.has_raw_transcript != 0,
      hasFinalOutput: snapshot.has_final_output != 0,
      hasError: snapshot.has_error != 0,
      recordingDurationMS: snapshot.recording_duration_ms,
      focusedBundleID: try copyString(field: VOXIT_HOST_STRING_FOCUSED_BUNDLE_ID),
      focusedAppName: try copyString(field: VOXIT_HOST_STRING_FOCUSED_APP_NAME),
      focusedWindowTitle: try copyString(field: VOXIT_HOST_STRING_FOCUSED_WINDOW_TITLE),
      focusedURLDomain: try copyString(field: VOXIT_HOST_STRING_FOCUSED_URL_DOMAIN),
      focusedElementRole: try copyString(field: VOXIT_HOST_STRING_FOCUSED_ELEMENT_ROLE),
      promptProfileID: try copyString(field: VOXIT_HOST_STRING_PROMPT_PROFILE_ID),
      promptDirective: try copyString(field: VOXIT_HOST_STRING_PROMPT_DIRECTIVE),
      rawTranscript: try copyString(field: VOXIT_HOST_STRING_RAW_TRANSCRIPT),
      finalOutput: try copyString(field: VOXIT_HOST_STRING_FINAL_OUTPUT),
      lastError: try copyString(field: VOXIT_HOST_STRING_LAST_ERROR),
      promptProfileKind: try decode(promptProfileKind: snapshot.prompt_profile_kind),
      voiceTier: try decode(voiceTier: snapshot.voice_tier),
      reasoningEffort: try decode(reasoningEffort: snapshot.reasoning_effort),
      outputPolicy: try decode(outputPolicy: snapshot.output_policy)
    )
  }

  private func copyString(field: VoxitHostStringField) throws -> String? {
    var buffer = [CChar](repeating: 0, count: 65_536)
    let bufferCount = buffer.count
    try buffer.withUnsafeMutableBufferPointer { pointer in
      try requireOk(
        voxit_host_session_copy_string(handle, field, pointer.baseAddress, UInt(bufferCount)),
        context: "copying host string"
      )
    }
    let endIndex = buffer.firstIndex(of: 0) ?? buffer.endIndex
    let bytes = buffer[..<endIndex].map { UInt8(bitPattern: $0) }
    let value = String(decoding: bytes, as: UTF8.self)
    return value.isEmpty ? nil : value
  }

  private func decode(platform: VoxitPlatformTag) throws -> HostPlatform {
    switch platform.rawValue {
    case VOXIT_PLATFORM_MACOS.rawValue:
      return .macOS
    case VOXIT_PLATFORM_UNSUPPORTED.rawValue:
      return .unsupported
    default:
      throw HostBridgeError.invalidPlatform(platform.rawValue)
    }
  }

  private func decode(authMethod: VoxitAuthMethod) throws -> AuthMethod {
    switch authMethod.rawValue {
    case VOXIT_AUTH_METHOD_CHATGPT_DEVICE_CODE.rawValue:
      return .chatGPTDeviceCode
    default:
      throw HostBridgeError.invalidAuthMethod(authMethod.rawValue)
    }
  }

  private func decode(authState: VoxitAuthState) throws -> AuthState {
    switch authState.rawValue {
    case VOXIT_AUTH_STATE_CHECKING.rawValue:
      return .checking
    case VOXIT_AUTH_STATE_SIGNED_OUT.rawValue:
      return .signedOut
    case VOXIT_AUTH_STATE_SIGNED_IN.rawValue:
      return .signedIn
    case VOXIT_AUTH_STATE_BUSY.rawValue:
      return .busy
    default:
      throw HostBridgeError.invalidAuthState(authState.rawValue)
    }
  }

  private func decode(dictationState: VoxitDictationState) throws -> DictationState {
    switch dictationState.rawValue {
    case VOXIT_DICTATION_STATE_IDLE.rawValue:
      return .idle
    case VOXIT_DICTATION_STATE_LISTENING.rawValue:
      return .listening
    case VOXIT_DICTATION_STATE_FINALIZING.rawValue:
      return .finalizing
    case VOXIT_DICTATION_STATE_REWRITING.rawValue:
      return .rewriting
    case VOXIT_DICTATION_STATE_DONE.rawValue:
      return .done
    default:
      throw HostBridgeError.invalidDictationState(dictationState.rawValue)
    }
  }

  private func decode(hotkeyMode: VoxitHotkeyMode) throws -> HotkeyMode {
    switch hotkeyMode.rawValue {
    case VOXIT_HOTKEY_MODE_TOGGLE.rawValue:
      return .toggle
    case VOXIT_HOTKEY_MODE_HOLD.rawValue:
      return .hold
    default:
      throw HostBridgeError.invalidHotkeyMode(hotkeyMode.rawValue)
    }
  }

  private func decode(promptProfileKind: VoxitPromptProfileKind) throws -> PromptProfileKind {
    switch promptProfileKind.rawValue {
    case VOXIT_PROMPT_PROFILE_FAST_DICTATION.rawValue:
      return .fastDictation
    case VOXIT_PROMPT_PROFILE_MESSAGING.rawValue:
      return .messaging
    case VOXIT_PROMPT_PROFILE_MAIL.rawValue:
      return .mail
    case VOXIT_PROMPT_PROFILE_CODE_EDITOR.rawValue:
      return .codeEditor
    case VOXIT_PROMPT_PROFILE_TERMINAL.rawValue:
      return .terminal
    case VOXIT_PROMPT_PROFILE_WORK_TRACKER.rawValue:
      return .workTracker
    default:
      throw HostBridgeError.invalidPromptProfileKind(promptProfileKind.rawValue)
    }
  }

  private func decode(voiceTier: VoxitVoiceInteractionTier) throws -> VoiceInteractionTier {
    switch voiceTier.rawValue {
    case VOXIT_VOICE_TIER_FAST_DICTATION.rawValue:
      return .fastDictation
    case VOXIT_VOICE_TIER_CONTEXT_REWRITE.rawValue:
      return .contextRewrite
    case VOXIT_VOICE_TIER_VOICE_INTENT.rawValue:
      return .voiceIntent
    default:
      throw HostBridgeError.invalidVoiceInteractionTier(voiceTier.rawValue)
    }
  }

  private func decode(reasoningEffort: VoxitVoiceReasoningEffort) throws -> VoiceReasoningEffort {
    switch reasoningEffort.rawValue {
    case VOXIT_REASONING_EFFORT_MINIMAL.rawValue:
      return .minimal
    case VOXIT_REASONING_EFFORT_LOW.rawValue:
      return .low
    case VOXIT_REASONING_EFFORT_MEDIUM.rawValue:
      return .medium
    case VOXIT_REASONING_EFFORT_HIGH.rawValue:
      return .high
    default:
      throw HostBridgeError.invalidVoiceReasoningEffort(reasoningEffort.rawValue)
    }
  }

  private func decode(outputPolicy: VoxitVoiceOutputPolicy) throws -> VoiceOutputPolicy {
    switch outputPolicy.rawValue {
    case VOXIT_OUTPUT_POLICY_INSERT_TEXT.rawValue:
      return .insertText
    case VOXIT_OUTPUT_POLICY_PREVIEW_BEFORE_INSERT.rawValue:
      return .previewBeforeInsert
    case VOXIT_OUTPUT_POLICY_CONFIRM_BEFORE_ACTION.rawValue:
      return .confirmBeforeAction
    default:
      throw HostBridgeError.invalidVoiceOutputPolicy(outputPolicy.rawValue)
    }
  }
}
