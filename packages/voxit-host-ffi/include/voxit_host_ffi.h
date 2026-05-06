#ifndef VOXIT_HOST_FFI_H
#define VOXIT_HOST_FFI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define VOXIT_HOST_FFI_ABI_VERSION 1u

typedef struct VoxitHostSessionHandle VoxitHostSessionHandle;

typedef enum VoxitStatus {
	VOXIT_STATUS_OK = 0,
	VOXIT_STATUS_NULL_HANDLE = 1,
	VOXIT_STATUS_NULL_OUTPUT = 2,
	VOXIT_STATUS_INVALID_INPUT = 3,
} VoxitStatus;

typedef enum VoxitPlatformTag {
	VOXIT_PLATFORM_MACOS = 0,
	VOXIT_PLATFORM_UNSUPPORTED = 1,
} VoxitPlatformTag;

typedef enum VoxitAuthMethod {
	VOXIT_AUTH_METHOD_CHATGPT_DEVICE_CODE = 0,
} VoxitAuthMethod;

typedef enum VoxitAuthState {
	VOXIT_AUTH_STATE_CHECKING = 0,
	VOXIT_AUTH_STATE_SIGNED_OUT = 1,
	VOXIT_AUTH_STATE_SIGNED_IN = 2,
	VOXIT_AUTH_STATE_BUSY = 3,
} VoxitAuthState;

typedef enum VoxitDictationState {
	VOXIT_DICTATION_STATE_IDLE = 0,
	VOXIT_DICTATION_STATE_LISTENING = 1,
	VOXIT_DICTATION_STATE_FINALIZING = 2,
	VOXIT_DICTATION_STATE_REWRITING = 3,
	VOXIT_DICTATION_STATE_DONE = 4,
} VoxitDictationState;

typedef enum VoxitHotkeyMode {
	VOXIT_HOTKEY_MODE_TOGGLE = 0,
	VOXIT_HOTKEY_MODE_HOLD = 1,
} VoxitHotkeyMode;

typedef struct VoxitHostConfig {
	enum VoxitPlatformTag platform;
} VoxitHostConfig;

typedef struct VoxitHostSnapshot {
	enum VoxitPlatformTag platform;
	enum VoxitAuthMethod auth_method;
	enum VoxitAuthState auth_state;
	enum VoxitDictationState dictation_state;
	enum VoxitHotkeyMode hotkey_mode;
	uint32_t panel_width_px;
	uint32_t panel_height_px;
	uint8_t rewrite_enabled;
} VoxitHostSnapshot;

uint32_t voxit_host_ffi_abi_version(void);
VoxitHostSessionHandle *voxit_host_session_create(struct VoxitHostConfig config);
void voxit_host_session_destroy(VoxitHostSessionHandle *handle);
enum VoxitStatus voxit_host_session_copy_snapshot(
	VoxitHostSessionHandle *handle,
	struct VoxitHostSnapshot *out
);

#ifdef __cplusplus
}
#endif

#endif
