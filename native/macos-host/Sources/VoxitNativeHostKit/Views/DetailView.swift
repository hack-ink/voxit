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
        case .activity:
          ActivityDetail(snapshot: snapshot)
        case .appRules:
          AppRulesDetail()
        case .profiles:
          ProfilesDetail()
        case .glossary:
          GlossaryDetail()
        case .promptLab:
          PromptLabDetail()
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
    }
  }
}

private struct ActivityDetail: View {
  var snapshot: HostSnapshot?

  var body: some View {
    LabeledContentGrid {
      StatusCard(
        title: "State",
        value: snapshot?.dictationState.label ?? "Loading",
        systemImage: "waveform"
      )
      StatusCard(
        title: "Auth",
        value: snapshot?.authState.label ?? "Loading",
        systemImage: "person.crop.circle.badge.checkmark"
      )
      StatusCard(
        title: "Profile",
        value: "Fast Dictation",
        systemImage: "person.text.rectangle"
      )
      StatusCard(
        title: "Policy",
        value: "Insert Text",
        systemImage: "text.cursor"
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

private struct AppRulesDetail: View {
  var body: some View {
    LabeledContentGrid {
      StatusCard(
        title: "Work Tracker",
        value: "Linear, GitHub",
        systemImage: "checklist"
      )
      StatusCard(
        title: "Messaging",
        value: "Slack, Discord",
        systemImage: "bubble.left.and.bubble.right"
      )
      StatusCard(
        title: "Code Editor",
        value: "Cursor, VS Code, Xcode",
        systemImage: "curlybraces"
      )
      StatusCard(
        title: "Terminal",
        value: "Confirm",
        systemImage: "terminal"
      )
    }
  }
}

private struct ProfilesDetail: View {
  var body: some View {
    LabeledContentGrid {
      StatusCard(title: "Fast Dictation", value: "Minimal", systemImage: "bolt")
      StatusCard(title: "Context Rewrite", value: "Low", systemImage: "wand.and.stars")
      StatusCard(title: "Voice Intent", value: "Medium", systemImage: "arrow.triangle.branch")
    }
  }
}

private struct GlossaryDetail: View {
  var body: some View {
    LabeledContentGrid {
      StatusCard(title: "Custom Terms", value: "None", systemImage: "text.book.closed")
      StatusCard(title: "Entity Guard", value: "Numbers, dates, money", systemImage: "number")
    }
  }
}

private struct PromptLabDetail: View {
  var body: some View {
    LabeledContentGrid {
      StatusCard(title: "Comparison", value: "No Runs", systemImage: "rectangle.split.2x1")
      StatusCard(title: "Reasoning", value: "Profile Default", systemImage: "brain")
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
