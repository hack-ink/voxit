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

public struct HostSnapshot: Equatable, Sendable {
  public var platform: HostPlatform
  public var authMethod: AuthMethod
  public var authState: AuthState
  public var dictationState: DictationState
  public var hotkeyMode: HotkeyMode
  public var panelWidth: Int
  public var panelHeight: Int
  public var rewriteEnabled: Bool

  public init(
    platform: HostPlatform,
    authMethod: AuthMethod,
    authState: AuthState,
    dictationState: DictationState,
    hotkeyMode: HotkeyMode,
    panelWidth: Int,
    panelHeight: Int,
    rewriteEnabled: Bool
  ) {
    self.platform = platform
    self.authMethod = authMethod
    self.authState = authState
    self.dictationState = dictationState
    self.hotkeyMode = hotkeyMode
    self.panelWidth = panelWidth
    self.panelHeight = panelHeight
    self.rewriteEnabled = rewriteEnabled
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
      rewriteEnabled: snapshot.rewrite_enabled != 0
    )
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
}
