import AppKit

@MainActor
final class GlobalHotkeyMonitor: ObservableObject {
  private enum Phase: Sendable {
    case down
    case up
  }

  private struct EventPayload: Sendable {
    let characters: String
    let modifierRawValue: UInt
    let phase: Phase
  }

  private static let relevantModifiers: NSEvent.ModifierFlags = [
    .command, .control, .option, .shift,
  ]

  private var globalKeyDownMonitor: Any?
  private var globalKeyUpMonitor: Any?
  private var localKeyDownMonitor: Any?
  private var localKeyUpMonitor: Any?
  private var presentation = VoxitSettings.defaults.dictationHotkeyPresentation
  private var hotkeyMode = VoxitHotkeyModePreference.toggle
  private var isPressed = false
  private var keyDownHandler: (() -> Void)?
  private var keyUpHandler: (() -> Void)?

  init() {
    installMonitors()
  }

  func configure(
    settings: VoxitSettings,
    keyDown: @escaping () -> Void,
    keyUp: @escaping () -> Void
  ) {
    presentation = settings.dictationHotkeyPresentation
    hotkeyMode = settings.hotkeyMode
    keyDownHandler = keyDown
    keyUpHandler = keyUp
  }

  private func installMonitors() {
    globalKeyDownMonitor = NSEvent.addGlobalMonitorForEvents(matching: .keyDown) {
      [weak self] event in
      Self.enqueue(event: event, phase: .down, target: self)
    }
    globalKeyUpMonitor = NSEvent.addGlobalMonitorForEvents(matching: .keyUp) { [weak self] event in
      Self.enqueue(event: event, phase: .up, target: self)
    }
    localKeyDownMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) {
      [weak self] event in
      Self.enqueue(event: event, phase: .down, target: self)
      return event
    }
    localKeyUpMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyUp) { [weak self] event in
      Self.enqueue(event: event, phase: .up, target: self)
      return event
    }
  }

  private func handle(_ payload: EventPayload) {
    guard matchesHotkey(payload) else {
      return
    }

    switch payload.phase {
    case .down:
      guard isPressed == false else {
        return
      }
      isPressed = true
      keyDownHandler?()
    case .up:
      guard isPressed else {
        return
      }
      isPressed = false
      if hotkeyMode == .hold {
        keyUpHandler?()
      }
    }
  }

  private func matchesHotkey(_ payload: EventPayload) -> Bool {
    let modifiers = NSEvent.ModifierFlags(rawValue: payload.modifierRawValue)
      .intersection(Self.relevantModifiers)
    let expectedModifiers = presentation.modifierMask.intersection(Self.relevantModifiers)

    guard modifiers == expectedModifiers else {
      return false
    }

    return normalizedKey(payload.characters) == normalizedKey(presentation.keyEquivalent)
  }

  private func normalizedKey(_ value: String) -> String {
    if value == " " {
      return "space"
    }

    return value.lowercased()
  }

  private static func enqueue(event: NSEvent, phase: Phase, target: GlobalHotkeyMonitor?) {
    let payload = EventPayload(
      characters: event.charactersIgnoringModifiers ?? "",
      modifierRawValue: event.modifierFlags.rawValue,
      phase: phase
    )

    Task { @MainActor in
      target?.handle(payload)
    }
  }
}
