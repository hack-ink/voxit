import Foundation
import VoxitHostBridge

@main
enum VoxitHostBridgeProbe {
  static func main() throws {
    let session = try VoxitHostSession()
    let snapshot = try session.currentSnapshot()

    guard snapshot.platform == .macOS else {
      fatalError("unexpected platform: \(snapshot)")
    }
    guard snapshot.authMethod == .chatGPTDeviceCode else {
      fatalError("unexpected auth method: \(snapshot)")
    }
    guard snapshot.dictationState == .idle else {
      fatalError("unexpected dictation state: \(snapshot)")
    }
    guard snapshot.panelWidth > 0, snapshot.panelHeight > 0 else {
      fatalError("unexpected panel dimensions: \(snapshot)")
    }
  }
}
