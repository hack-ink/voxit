#ifndef VOXIT_HOST_FFI_H
#define VOXIT_HOST_FFI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define VOXIT_HOST_FFI_ABI_VERSION 4u

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

typedef enum VoxitPromptProfileKind {
	VOXIT_PROMPT_PROFILE_FAST_DICTATION = 0,
	VOXIT_PROMPT_PROFILE_MESSAGING = 1,
	VOXIT_PROMPT_PROFILE_MAIL = 2,
	VOXIT_PROMPT_PROFILE_CODE_EDITOR = 3,
	VOXIT_PROMPT_PROFILE_TERMINAL = 4,
	VOXIT_PROMPT_PROFILE_WORK_TRACKER = 5,
} VoxitPromptProfileKind;

typedef enum VoxitVoiceInteractionTier {
	VOXIT_VOICE_TIER_FAST_DICTATION = 0,
	VOXIT_VOICE_TIER_CONTEXT_REWRITE = 1,
	VOXIT_VOICE_TIER_VOICE_INTENT = 2,
} VoxitVoiceInteractionTier;

typedef enum VoxitVoiceReasoningEffort {
	VOXIT_REASONING_EFFORT_MINIMAL = 0,
	VOXIT_REASONING_EFFORT_LOW = 1,
	VOXIT_REASONING_EFFORT_MEDIUM = 2,
	VOXIT_REASONING_EFFORT_HIGH = 3,
} VoxitVoiceReasoningEffort;

typedef enum VoxitVoiceOutputPolicy {
	VOXIT_OUTPUT_POLICY_INSERT_TEXT = 0,
	VOXIT_OUTPUT_POLICY_PREVIEW_BEFORE_INSERT = 1,
	VOXIT_OUTPUT_POLICY_CONFIRM_BEFORE_ACTION = 2,
} VoxitVoiceOutputPolicy;

typedef enum VoxitHostStringField {
	VOXIT_HOST_STRING_FOCUSED_BUNDLE_ID = 0,
	VOXIT_HOST_STRING_FOCUSED_APP_NAME = 1,
	VOXIT_HOST_STRING_FOCUSED_WINDOW_TITLE = 2,
	VOXIT_HOST_STRING_FOCUSED_URL_DOMAIN = 3,
	VOXIT_HOST_STRING_FOCUSED_ELEMENT_ROLE = 4,
	VOXIT_HOST_STRING_PROMPT_PROFILE_ID = 5,
	VOXIT_HOST_STRING_PROMPT_DIRECTIVE = 6,
	VOXIT_HOST_STRING_RAW_TRANSCRIPT = 7,
	VOXIT_HOST_STRING_FINAL_OUTPUT = 8,
	VOXIT_HOST_STRING_LAST_ERROR = 9,
} VoxitHostStringField;

typedef struct VoxitHostConfig {
	enum VoxitPlatformTag platform;
} VoxitHostConfig;

typedef struct VoxitHostPreferences {
	uint8_t start_hidden;
	enum VoxitHotkeyMode hotkey_mode;
	uint8_t paste_after_transcription;
	uint8_t rewrite_after_transcription;
} VoxitHostPreferences;

typedef struct VoxitHostSnapshot {
	enum VoxitPlatformTag platform;
	enum VoxitAuthMethod auth_method;
	enum VoxitAuthState auth_state;
	enum VoxitDictationState dictation_state;
	enum VoxitHotkeyMode hotkey_mode;
	uint32_t panel_width_px;
	uint32_t panel_height_px;
	uint8_t rewrite_enabled;
	uint8_t has_focused_context;
	uint8_t selected_text_present;
	uint8_t has_raw_transcript;
	uint8_t has_final_output;
	uint8_t has_error;
	uint64_t recording_duration_ms;
	enum VoxitPromptProfileKind prompt_profile_kind;
	enum VoxitVoiceInteractionTier voice_tier;
	enum VoxitVoiceReasoningEffort reasoning_effort;
	enum VoxitVoiceOutputPolicy output_policy;
} VoxitHostSnapshot;

uint32_t voxit_host_ffi_abi_version(void);
VoxitHostSessionHandle *voxit_host_session_create(struct VoxitHostConfig config);
void voxit_host_session_destroy(VoxitHostSessionHandle *handle);
enum VoxitStatus voxit_host_session_refresh_focused_context(VoxitHostSessionHandle *handle);
enum VoxitStatus voxit_host_session_start_dictation(VoxitHostSessionHandle *handle);
enum VoxitStatus voxit_host_session_stop_dictation(VoxitHostSessionHandle *handle);
enum VoxitStatus voxit_host_session_paste_final_output(VoxitHostSessionHandle *handle);
enum VoxitStatus voxit_host_session_save_preferences(
	VoxitHostSessionHandle *handle,
	struct VoxitHostPreferences preferences,
	const char *hotkey_chord
);
enum VoxitStatus voxit_host_session_copy_snapshot(
	VoxitHostSessionHandle *handle,
	struct VoxitHostSnapshot *out
);
enum VoxitStatus voxit_host_session_copy_string(
	VoxitHostSessionHandle *handle,
	enum VoxitHostStringField field,
	char *out,
	uintptr_t out_len
);

#ifdef __cplusplus
}
#endif

#endif
