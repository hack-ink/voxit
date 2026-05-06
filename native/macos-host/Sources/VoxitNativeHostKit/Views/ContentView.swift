import SwiftUI
import VoxitHostBridge

public struct ContentView: View {
  @ObservedObject var store: HostStore
  @SceneStorage("selection") private var selection: NavigationItem = .dictation

  public init(store: HostStore) {
    self.store = store
  }

  public var body: some View {
    NavigationSplitView {
      SidebarView(selection: $selection, snapshot: store.snapshot)
    } detail: {
      DetailView(selection: selection, snapshot: store.snapshot, errorMessage: store.errorMessage)
    }
  }
}
