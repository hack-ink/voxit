import Foundation
import VoxitHostBridge

@MainActor
public final class HostStore: ObservableObject {
  @Published public private(set) var snapshot: HostSnapshot?
  @Published public private(set) var errorMessage: String?

  private var session: VoxitHostSession?
  private var pollingTask: Task<Void, Never>?

  public init() {}

  deinit {
    pollingTask?.cancel()
  }

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
      startRealtimePolling()
    } catch {
      errorMessage = String(describing: error)
    }
  }

  public func stopDictation() async {
    pollingTask?.cancel()
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
      snapshot = try session.saveModelPreferences(
        realtimeModel: settings.realtimeModel,
        realtimeTranscriptionModel: settings.realtimeTranscriptionModel,
        finalizeModel: settings.finalizeModel,
        rewriteModel: settings.rewriteModel
      )
      errorMessage = snapshot?.lastError
    } catch {
      errorMessage = String(describing: error)
    }
  }

  func setProfileOverride(_ profileKind: PromptProfileKind?) async {
    do {
      let session = try currentSession()
      if let profileKind {
        snapshot = try session.setProfileOverride(profileKind)
      } else {
        snapshot = try session.clearProfileOverride()
      }
      errorMessage = snapshot?.lastError
    } catch {
      errorMessage = String(describing: error)
    }
  }

  func setGlossary(_ glossaryTerms: String) async {
    do {
      let session = try currentSession()
      snapshot = try session.setGlossary(glossaryTerms)
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

  private func startRealtimePolling() {
    pollingTask?.cancel()
    pollingTask = Task { [weak self] in
      while Task.isCancelled == false {
        try? await Task.sleep(nanoseconds: 250_000_000)
        await self?.reload()

        let state = self?.snapshot?.dictationState
        if state != .listening {
          break
        }
      }
    }
  }
}
