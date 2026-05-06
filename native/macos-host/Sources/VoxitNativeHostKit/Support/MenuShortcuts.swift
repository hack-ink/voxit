import AppKit
import SwiftUI

extension VoxitHotkeyPresentation {
  var swiftUIKeyEquivalent: KeyEquivalent {
    if keyEquivalent == " " {
      return .space
    }

    guard let character = keyEquivalent.first else {
      return .space
    }
    return KeyEquivalent(character)
  }

  var swiftUIModifiers: EventModifiers {
    var modifiers = EventModifiers()
    if modifierMask.contains(.control) {
      modifiers.insert(.control)
    }
    if modifierMask.contains(.option) {
      modifiers.insert(.option)
    }
    if modifierMask.contains(.shift) {
      modifiers.insert(.shift)
    }
    if modifierMask.contains(.command) {
      modifiers.insert(.command)
    }
    return modifiers
  }
}
