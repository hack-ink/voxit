#ifndef VOXIT_HOST_FFI_SHIM_H
#define VOXIT_HOST_FFI_SHIM_H

#include "../../../../../packages/voxit-host-ffi/include/voxit_host_ffi.h"

static inline uint32_t voxit_status_code(enum VoxitStatus status) {
	return (uint32_t)status;
}

#endif
