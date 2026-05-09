import Foundation
import VoxitHostBridge

@MainActor
public final class HostStore: ObservableObject {
  @Published public private(set) var snapshot: HostSnapshot?
  @Published public private(set) var errorMessage: String?

  private var session: VoxitHostSession?

  public init() {}

  public func reload() async {
    do {
      let session = try currentSession()
      snapshot = try session.currentSnapshot()
      errorMessage = nil
    } catch {
      errorMessage = String(describing: error)
    }
  }

  public func refreshFocusedContext() async {
    do {
      let session = try currentSession()
      snapshot = try session.refreshFocusedContext()
      errorMessage = nil
    } catch {
      errorMessage = String(describing: error)
    }
  }

  public func startDictation() async {
    do {
      let session = try currentSession()
      snapshot = try session.startDictation()
      errorMessage = snapshot?.lastError
    } catch {
      errorMessage = String(describing: error)
    }
  }

  public func stopDictation() async {
    do {
      let session = try currentSession()
      snapshot = try session.stopDictation()
      errorMessage = snapshot?.lastError
    } catch {
      errorMessage = String(describing: error)
    }
  }

  public func pasteFinalOutput() async {
    do {
      let session = try currentSession()
      snapshot = try session.pasteFinalOutput()
      errorMessage = snapshot?.lastError
    } catch {
      errorMessage = String(describing: error)
    }
  }

  func savePreferences(_ settings: VoxitSettings) async {
    do {
      let session = try currentSession()
      snapshot = try session.savePreferences(
        hotkeyChord: settings.dictationHotkey,
        hotkeyMode: settings.hotkeyMode.hostBridgeValue,
        startHidden: settings.startHidden,
        pasteAfterTranscription: settings.pasteAfterTranscription,
        rewriteAfterTranscription: settings.rewriteAfterTranscription
      )
      errorMessage = snapshot?.lastError
    } catch {
      errorMessage = String(describing: error)
    }
  }

  private func currentSession() throws -> VoxitHostSession {
    if let session {
      return session
    }

    let session = try VoxitHostSession()
    self.session = session

    return session
  }
}
