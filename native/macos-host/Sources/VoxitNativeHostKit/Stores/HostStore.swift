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

  private func currentSession() throws -> VoxitHostSession {
    if let session {
      return session
    }

    let session = try VoxitHostSession()
    self.session = session

    return session
  }
}
