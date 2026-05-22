import SwiftUI
import VoxitHostBridge

struct RecordingHUDView: View {
  @ObservedObject var store: HostStore

  var body: some View {
    VStack(alignment: .leading, spacing: 14) {
      HStack(alignment: .center) {
        VStack(alignment: .leading, spacing: 2) {
          Text(store.snapshot?.dictationState.label ?? "Loading")
            .font(.headline)
          Text(store.snapshot?.promptProfileKind.label ?? "Fast Dictation")
            .font(.caption)
            .foregroundStyle(.secondary)
        }
        Spacer()
        Circle()
          .fill(statusColor)
          .frame(width: 10, height: 10)
      }

      Text(previewText)
        .font(.body)
        .textSelection(.enabled)
        .frame(maxWidth: .infinity, minHeight: 72, alignment: .topLeading)
        .padding(10)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 8))

      HStack(spacing: 10) {
        Button("Stop", systemImage: "stop.circle") {
          Task {
            await store.stopDictation()
          }
        }
        .buttonStyle(.borderedProminent)
        .disabled(store.snapshot?.dictationState != .listening)

        Button("Paste", systemImage: "text.badge.checkmark") {
          Task {
            await store.pasteFinalOutput()
          }
        }
        .disabled(store.snapshot?.hasFinalOutput != true)
      }
    }
    .padding(16)
    .frame(width: 380)
  }

  private var previewText: String {
    if let finalOutput = store.snapshot?.finalOutput {
      return finalOutput
    }
    if let rawTranscript = store.snapshot?.rawTranscript {
      return rawTranscript
    }
    if let pass1Transcript = store.snapshot?.pass1TranscriptPreview {
      return pass1Transcript
    }
    if let error = store.snapshot?.lastError {
      return error
    }
    return store.snapshot?.focusedAppLabel ?? "No focused app context"
  }

  private var statusColor: Color {
    switch store.snapshot?.dictationState {
    case .listening:
      return .red
    case .finalizing, .rewriting:
      return .orange
    case .done:
      return .green
    default:
      return .secondary
    }
  }
}
