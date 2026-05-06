import SwiftUI
import VoxitHostBridge

struct SidebarView: View {
  @Binding var selection: NavigationItem
  var snapshot: HostSnapshot?

  var body: some View {
    List(selection: $selection) {
      ForEach(NavigationItem.allCases) { item in
        Label(item.title, systemImage: item.systemImage)
          .tag(item)
      }
    }
    .listStyle(.sidebar)
    .navigationTitle("Voxit")
    .safeAreaInset(edge: .bottom) {
      if let snapshot {
        SidebarStatus(snapshot: snapshot)
      }
    }
  }
}

private struct SidebarStatus: View {
  var snapshot: HostSnapshot

  var body: some View {
    VStack(alignment: .leading, spacing: 4) {
      Text(snapshot.dictationState.label)
        .font(.caption)
        .foregroundStyle(.primary)
      Text(snapshot.authState.label)
        .font(.caption2)
        .foregroundStyle(.secondary)
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.horizontal, 12)
    .padding(.vertical, 10)
  }
}
