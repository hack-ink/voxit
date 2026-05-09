import SwiftUI
import VoxitHostBridge

public struct ContentView: View {
  @ObservedObject var store: HostStore
  @SceneStorage("selection") private var selection: NavigationItem = .activity

  public init(store: HostStore) {
    self.store = store
  }

  public var body: some View {
    NavigationSplitView {
      SidebarView(selection: $selection, snapshot: store.snapshot)
    } detail: {
      DetailView(
        selection: selection,
        snapshot: store.snapshot,
        errorMessage: store.errorMessage,
        refreshFocusedContext: {
          Task {
            await store.refreshFocusedContext()
          }
        },
        startDictation: {
          Task {
            await store.startDictation()
          }
        },
        stopDictation: {
          Task {
            await store.stopDictation()
          }
        },
        pasteFinalOutput: {
          Task {
            await store.pasteFinalOutput()
          }
        }
      )
    }
  }
}
