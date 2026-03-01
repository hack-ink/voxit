use std::{
	sync::{
		Arc,
		atomic::{AtomicU8, Ordering},
		mpsc::Sender,
	},
	thread,
};

use global_hotkey::{
	GlobalHotKeyEvent, GlobalHotKeyEventReceiver, GlobalHotKeyManager, HotKeyState,
	hotkey::{Code, HotKey, Modifiers},
};

use crate::{AppCommand, HotkeyMode, prelude::Result};

pub(crate) fn spawn_global_hotkey_listener(
	command_tx: Sender<AppCommand>,
	mode: Arc<AtomicU8>,
) -> Result<GlobalHotKeyManager> {
	let manager = GlobalHotKeyManager::new().map_err(|err| {
		crate::prelude::eyre!("Failed to initialize global hotkey manager: {err}")
	})?;
	let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);

	manager.register(hotkey).map_err(|err| {
		crate::prelude::eyre!("Failed to register global hotkey (Ctrl+Shift+Space): {err}")
	})?;

	let hotkey_id = hotkey.id();
	let receiver: GlobalHotKeyEventReceiver = GlobalHotKeyEvent::receiver().clone();

	thread::spawn(move || {
		let mut is_holding = false;

		while let Ok(event) = receiver.recv() {
			if event.id() != hotkey_id {
				continue;
			}

			match event.state() {
				HotKeyState::Pressed => match HotkeyMode::from_u8(mode.load(Ordering::Acquire)) {
					HotkeyMode::Toggle => {
						let _ = command_tx.send(AppCommand::ToggleRecording);
					},
					HotkeyMode::Hold =>
						if !is_holding {
							is_holding = true;

							let _ = command_tx.send(AppCommand::StartRecording);
						},
				},
				HotKeyState::Released =>
					if is_holding {
						is_holding = false;

						let _ = command_tx.send(AppCommand::StopRecording);
					},
			}
		}
	});

	Ok(manager)
}
