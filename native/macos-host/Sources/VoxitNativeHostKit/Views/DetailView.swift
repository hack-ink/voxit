import SwiftUI
import VoxitHostBridge

struct DetailView: View {
  var selection: NavigationItem
  var snapshot: HostSnapshot?
  var errorMessage: String?

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 18) {
        header

        if let errorMessage {
          StatusCard(
            title: "Host Bridge", value: errorMessage, systemImage: "exclamationmark.triangle")
        }

        switch selection {
        case .dictation:
          DictationDetail(snapshot: snapshot)
        case .auth:
          AuthDetail(snapshot: snapshot)
        case .audio:
          AudioDetail()
        }
      }
      .padding(24)
      .frame(maxWidth: 760, alignment: .leading)
    }
    .navigationTitle(selection.title)
  }

  private var header: some View {
    VStack(alignment: .leading, spacing: 5) {
      Text(selection.title)
        .font(.largeTitle.weight(.semibold))
      Text(subtitle)
        .foregroundStyle(.secondary)
    }
  }

  private var subtitle: String {
    switch selection {
    case .dictation:
      return "Record, finalize, rewrite, and paste."
    case .auth:
      return "ChatGPT device-code authorization."
    case .audio:
      return "Microphone input and permission state."
    }
  }
}

private struct DictationDetail: View {
  var snapshot: HostSnapshot?

  var body: some View {
    LabeledContentGrid {
      StatusCard(
        title: "State",
        value: snapshot?.dictationState.label ?? "Loading",
        systemImage: "waveform"
      )
      StatusCard(
        title: "Rewrite",
        value: snapshot?.rewriteEnabled == true ? "Enabled" : "Disabled",
        systemImage: "wand.and.stars"
      )
    }

    HStack(spacing: 10) {
      Button("Start Recording", systemImage: "record.circle") {}
        .buttonStyle(.borderedProminent)
        .disabled(true)
      Button("Paste Raw", systemImage: "text.badge.checkmark") {}
        .disabled(true)
    }
  }
}

private struct AuthDetail: View {
  var snapshot: HostSnapshot?

  var body: some View {
    LabeledContentGrid {
      StatusCard(
        title: "Status",
        value: snapshot?.authState.label ?? "Loading",
        systemImage: "person.crop.circle.badge.checkmark"
      )
      StatusCard(
        title: "Method",
        value: snapshot?.authMethod.label ?? "Device Code",
        systemImage: "rectangle.and.pencil.and.ellipsis"
      )
    }

    Button("Sign In", systemImage: "arrow.right.circle") {}
      .buttonStyle(.borderedProminent)
      .disabled(true)
  }
}

private struct AudioDetail: View {
  var body: some View {
    LabeledContentGrid {
      StatusCard(title: "Input", value: "System Default", systemImage: "mic")
      StatusCard(title: "Permission", value: "Unknown", systemImage: "checkmark.shield")
    }
  }
}

private struct LabeledContentGrid<Content: View>: View {
  @ViewBuilder var content: Content

  var body: some View {
    LazyVGrid(columns: [GridItem(.adaptive(minimum: 220), spacing: 12)], spacing: 12) {
      content
    }
  }
}

private struct StatusCard: View {
  var title: String
  var value: String
  var systemImage: String

  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      Image(systemName: systemImage)
        .foregroundStyle(.secondary)
      VStack(alignment: .leading, spacing: 2) {
        Text(title)
          .font(.caption)
          .foregroundStyle(.secondary)
        Text(value)
          .font(.title3.weight(.medium))
          .lineLimit(2)
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(14)
    .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
  }
}
