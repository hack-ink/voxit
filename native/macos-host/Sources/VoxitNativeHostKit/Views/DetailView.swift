import SwiftUI
import VoxitHostBridge

struct DetailView: View {
  var selection: NavigationItem
  var snapshot: HostSnapshot?
  var errorMessage: String?
  var refreshFocusedContext: () -> Void
  var startDictation: () -> Void
  var stopDictation: () -> Void
  var pasteFinalOutput: () -> Void
  var setProfileOverride: (PromptProfileKind?) -> Void
  var setGlossary: (String) -> Void

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
          ActivityDetail(
            snapshot: snapshot,
            refreshFocusedContext: refreshFocusedContext,
            startDictation: startDictation,
            stopDictation: stopDictation,
            pasteFinalOutput: pasteFinalOutput
          )
        case .appRules:
          AppRulesDetail()
        case .profiles:
          ProfilesDetail(snapshot: snapshot, setProfileOverride: setProfileOverride)
        case .glossary:
          GlossaryDetail(setGlossary: setGlossary)
        case .promptLab:
          PromptLabDetail(snapshot: snapshot)
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
  var refreshFocusedContext: () -> Void
  var startDictation: () -> Void
  var stopDictation: () -> Void
  var pasteFinalOutput: () -> Void

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
        value: snapshot?.promptProfileKind.label ?? "Loading",
        systemImage: "person.text.rectangle"
      )
      StatusCard(
        title: "Tier",
        value: snapshot?.voiceTier.label ?? "Loading",
        systemImage: "square.stack.3d.up"
      )
      StatusCard(
        title: "Reasoning",
        value: snapshot?.reasoningEffort.label ?? "Loading",
        systemImage: "brain"
      )
      StatusCard(
        title: "Output",
        value: snapshot?.outputPolicy.label ?? "Loading",
        systemImage: "text.cursor"
      )
      StatusCard(
        title: "Last Run",
        value: snapshot?.recordingSummary ?? "No Runs",
        systemImage: "timer"
      )
      StatusCard(
        title: "Focused App",
        value: snapshot?.focusedAppLabel ?? "No Context",
        systemImage: "app.connected.to.app.below.fill"
      )
      StatusCard(
        title: "Window",
        value: snapshot?.focusedWindowTitle ?? snapshot?.focusedURLDomain ?? "No Context",
        systemImage: "macwindow"
      )
    }

    HStack(spacing: 10) {
      Button("Refresh Focus", systemImage: "scope") {
        refreshFocusedContext()
      }
      Button("Start Recording", systemImage: "record.circle") {
        startDictation()
      }
      .buttonStyle(.borderedProminent)
      .disabled(snapshot?.dictationState == .listening)
      Button("Stop", systemImage: "stop.circle") {
        stopDictation()
      }
      .disabled(snapshot?.dictationState != .listening)
      Button("Paste Final", systemImage: "text.badge.checkmark") {
        pasteFinalOutput()
      }
      .disabled(snapshot?.hasFinalOutput != true)
    }

    if let finalOutput = snapshot?.finalOutput {
      TranscriptPreview(title: "Final Output", text: finalOutput)
    }
    if let rawTranscript = snapshot?.rawTranscript {
      TranscriptPreview(title: "Raw Transcript", text: rawTranscript)
    }
    if let pass1Transcript = snapshot?.pass1TranscriptPreview {
      TranscriptPreview(title: "Realtime Draft", text: pass1Transcript)
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
  var snapshot: HostSnapshot?
  var setProfileOverride: (PromptProfileKind?) -> Void
  @AppStorage("profileOverride") private var profileOverride = ProfileOverride.auto.rawValue

  var body: some View {
    Picker("Profile", selection: profileOverrideBinding) {
      ForEach(ProfileOverride.allCases) { profile in
        Text(profile.title).tag(profile.rawValue)
      }
    }
    .pickerStyle(.menu)

    LabeledContentGrid {
      StatusCard(
        title: "Current",
        value: snapshot?.promptProfileKind.label ?? "Loading",
        systemImage: "scope"
      )
      StatusCard(
        title: "Reasoning",
        value: snapshot?.reasoningEffort.label ?? "Loading",
        systemImage: "brain"
      )
      StatusCard(
        title: "Directive",
        value: snapshot?.promptDirective ?? "Profile Default",
        systemImage: "wand.and.stars"
      )
      StatusCard(
        title: "Output", value: snapshot?.outputPolicy.label ?? "Loading",
        systemImage: "arrow.triangle.branch")
    }
  }

  private var profileOverrideBinding: Binding<String> {
    Binding(
      get: { profileOverride },
      set: { value in
        profileOverride = value
        setProfileOverride(ProfileOverride(rawValue: value)?.profileKind)
      }
    )
  }
}

private struct GlossaryDetail: View {
  var setGlossary: (String) -> Void
  @AppStorage("glossaryTerms") private var glossaryTerms = ""

  var body: some View {
    TextEditor(text: glossaryBinding)
      .font(.body)
      .frame(minHeight: 140)
      .padding(10)
      .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))

    LabeledContentGrid {
      StatusCard(title: "Custom Terms", value: glossarySummary, systemImage: "text.book.closed")
      StatusCard(title: "Entity Guard", value: "Numbers, dates, money", systemImage: "number")
    }
  }

  private var glossaryBinding: Binding<String> {
    Binding(
      get: { glossaryTerms },
      set: { value in
        glossaryTerms = value
        setGlossary(value)
      }
    )
  }

  private var glossarySummary: String {
    let count = glossaryTerms.split(whereSeparator: \.isNewline).filter { !$0.isEmpty }.count
    return count == 0 ? "None" : "\(count) Terms"
  }
}

private struct PromptLabDetail: View {
  var snapshot: HostSnapshot?
  @AppStorage("promptLabSample") private var sample = "Summarize what I just said for this app."

  var body: some View {
    TextEditor(text: $sample)
      .font(.body)
      .frame(minHeight: 96)
      .padding(10)
      .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))

    LabeledContentGrid {
      StatusCard(
        title: "Profile", value: snapshot?.promptProfileKind.label ?? "Loading",
        systemImage: "rectangle.split.2x1")
      StatusCard(
        title: "Reasoning", value: snapshot?.reasoningEffort.label ?? "Profile Default",
        systemImage: "brain")
      StatusCard(
        title: "Directive", value: snapshot?.promptDirective ?? "Profile Default",
        systemImage: "text.alignleft")
    }
  }
}

enum ProfileOverride: String, CaseIterable, Identifiable {
  case auto
  case fastDictation
  case messaging
  case mail
  case codeEditor
  case terminal
  case workTracker

  var id: Self { self }

  var title: String {
    switch self {
    case .auto:
      return "Auto"
    case .fastDictation:
      return "Fast Dictation"
    case .messaging:
      return "Messaging"
    case .mail:
      return "Mail"
    case .codeEditor:
      return "Code Editor"
    case .terminal:
      return "Terminal"
    case .workTracker:
      return "Work Tracker"
    }
  }

  var profileKind: PromptProfileKind? {
    switch self {
    case .auto:
      return nil
    case .fastDictation:
      return .fastDictation
    case .messaging:
      return .messaging
    case .mail:
      return .mail
    case .codeEditor:
      return .codeEditor
    case .terminal:
      return .terminal
    case .workTracker:
      return .workTracker
    }
  }
}

private struct TranscriptPreview: View {
  var title: String
  var text: String

  var body: some View {
    VStack(alignment: .leading, spacing: 6) {
      Text(title)
        .font(.caption)
        .foregroundStyle(.secondary)
      Text(text)
        .font(.body)
        .textSelection(.enabled)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
    .padding(14)
    .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
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
