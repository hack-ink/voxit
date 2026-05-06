#include "voxit_host_ffi.h"

int main(void) {
	VoxitHostConfig config = {
		.platform = VOXIT_PLATFORM_MACOS,
	};
	VoxitHostSnapshot snapshot = {0};
	VoxitHostSessionHandle *handle = voxit_host_session_create(config);

	if (handle == 0) {
		return 1;
	}
	if (voxit_host_ffi_abi_version() != VOXIT_HOST_FFI_ABI_VERSION) {
		voxit_host_session_destroy(handle);
		return 2;
	}
	if (voxit_host_session_copy_snapshot(handle, &snapshot) != VOXIT_STATUS_OK) {
		voxit_host_session_destroy(handle);
		return 3;
	}
	if (snapshot.platform != VOXIT_PLATFORM_MACOS) {
		voxit_host_session_destroy(handle);
		return 4;
	}
	if (snapshot.auth_method != VOXIT_AUTH_METHOD_CHATGPT_DEVICE_CODE) {
		voxit_host_session_destroy(handle);
		return 5;
	}

	voxit_host_session_destroy(handle);
	return 0;
}
