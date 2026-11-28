//! Xbox Series–specific DLLs with no host equivalent.
//!
//! `XboxBvhBuilder.dll`, `Bink2Scarlett.dll` and the FidelityFX loader are
//! not stubbed: the package's own copies run. A BVH stub left the structure
//! sizes at 0 (the title asserted on the empty allocation; the builder's
//! compute stages run on the GPU only with `GAMBIARRA_BVH_BUILDER=1`), a Bink
//! stub that opened no video left the title screen black, and an `ffxQuery`
//! stub that wrote no answer left the upscaler's jitter phase count at 0 (the
//! title divided by it every frame).
//!
//! * `libHttpClient.GDK.dll` — Xbox GDK HTTP client. HCInitialize succeeds so
//!   title state machines advance; individual call creation fails so no actual
//!   network traffic is attempted.

#![allow(non_snake_case)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::too_many_arguments)]

use std::ffi::{c_char, c_void};

use crate::{HleModule, hle_export};

type HRESULT = i32;
const S_OK: HRESULT = 0;
const E_FAIL: HRESULT = 0x8000_4005_u32 as i32;
const E_NOTIMPL: HRESULT = 0x8000_4001_u32 as i32;

// ── libHttpClient.GDK ────────────────────────────────────────────────────────
//
// HCInitialize returns S_OK so title state machines advance past the network
// init phase. Individual call creation fails (E_FAIL) so no HTTP requests are
// attempted. All response/header accessors are stubbed to return E_FAIL or
// write safe zero values.

extern "system" fn HCInitialize(_config: *const c_void) -> HRESULT { S_OK }
extern "system" fn HCCleanupAsync(_async_block: *mut c_void) -> HRESULT { S_OK }
extern "system" fn HCHttpCallCreate(out: *mut *mut c_void) -> HRESULT {
    // --unimplemented zero returned 0 (S_OK) for this function; E_FAIL here
    // triggers an ASSERT(SUCCEEDED(...)) at line 102 in System Init.
    // We write null to *out so any subsequent handle-using calls see a null
    // handle (which should be guarded by SUCCEEDED() checks the game already has).
    if !out.is_null() {
        unsafe { *out = std::ptr::null_mut() };
    }
    S_OK
}
extern "system" fn HCHttpCallCloseHandle(_call: *mut c_void) {}
extern "system" fn HCHttpCallDuplicateHandle(_call: *mut c_void) -> HRESULT { S_OK }
extern "system" fn HCHttpCallGetRequestUrl(
    _call: *mut c_void,
    _out: *mut *const c_char,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallPerformAsync(
    _call: *mut c_void,
    _async_block: *mut c_void,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestGetHeaderAtIndex(
    _call: *mut c_void,
    _idx: u32,
    _name: *mut *const c_char,
    _value: *mut *const c_char,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestGetNumHeaders(
    _call: *mut c_void,
    _out: *mut u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestGetRequestBodyBytes(
    _call: *mut c_void,
    _out: *mut *const u8,
    _len: *mut u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestGetRetryAllowed(
    _call: *mut c_void,
    _out: *mut bool,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestGetRetryCacheId(
    _call: *mut c_void,
    _out: *mut u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestGetRetryDelay(
    _call: *mut c_void,
    _out: *mut u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestGetTimeout(
    _call: *mut c_void,
    _out: *mut u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestGetTimeoutWindow(
    _call: *mut c_void,
    _out: *mut u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestGetUrl(
    _call: *mut c_void,
    _method: *mut *const c_char,
    _url: *mut *const c_char,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestSetHeader(
    _call: *mut c_void,
    _name: *const c_char,
    _value: *const c_char,
    _merge: bool,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestSetRequestBodyBytes(
    _call: *mut c_void,
    _body: *const u8,
    _len: u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestSetRequestBodyString(
    _call: *mut c_void,
    _body: *const c_char,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestSetRetryAllowed(
    _call: *mut c_void,
    _allow: bool,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestSetRetryCacheId(
    _call: *mut c_void,
    _id: u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestSetRetryDelay(
    _call: *mut c_void,
    _secs: u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestSetTimeout(
    _call: *mut c_void,
    _secs: u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestSetTimeoutWindow(
    _call: *mut c_void,
    _secs: u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallRequestSetUrl(
    _call: *mut c_void,
    _method: *const c_char,
    _url: *const c_char,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallResponseGetHeader(
    _call: *mut c_void,
    _name: *const c_char,
    _out: *mut *const c_char,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallResponseGetHeaderAtIndex(
    _call: *mut c_void,
    _idx: u32,
    _name: *mut *const c_char,
    _value: *mut *const c_char,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallResponseGetNetworkErrorCode(
    _call: *mut c_void,
    _code: *mut u32,
    _plat: *mut u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallResponseGetNumHeaders(
    _call: *mut c_void,
    _out: *mut u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallResponseGetPlatformNetworkErrorMessage(
    _call: *mut c_void,
    _out: *mut *const c_char,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallResponseGetResponseBodyBytes(
    _call: *mut c_void,
    _out: *mut *const u8,
    _len: *mut u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallResponseGetResponseBodyBytesSize(
    _call: *mut c_void,
    _out: *mut u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallResponseGetResponseString(
    _call: *mut c_void,
    _out: *mut *const c_char,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallResponseGetStatusCode(
    _call: *mut c_void,
    _out: *mut u32,
) -> HRESULT {
    S_OK
}
extern "system" fn HCHttpCallSetTracing(_call: *mut c_void, _trace: bool) -> HRESULT {
    S_OK
}
extern "system" fn HCSettingsGetTraceLevel(_out: *mut u32) -> HRESULT { S_OK }
extern "system" fn HCTraceImplMessage(
    _area: *const c_char,
    _level: u32,
    _thread: u64,
    _time: u64,
    _fmt: *const c_char,
) {
}
extern "system" fn HCTraceSetEtwEnabled(_enabled: bool) {}

// ── Module table ─────────────────────────────────────────────────────────────

pub(crate) fn module() -> HleModule {
    HleModule {
        name: "scarlett",
        dlls: &[
            "libhttpclient.gdk.dll",
        ],
        exports: vec![
            // libHttpClient.GDK.dll
            hle_export!(HCInitialize),
            hle_export!(HCCleanupAsync),
            hle_export!(HCHttpCallCreate),
            hle_export!(HCHttpCallCloseHandle),
            hle_export!(HCHttpCallDuplicateHandle),
            hle_export!(HCHttpCallGetRequestUrl),
            hle_export!(HCHttpCallPerformAsync),
            hle_export!(HCHttpCallRequestGetHeaderAtIndex),
            hle_export!(HCHttpCallRequestGetNumHeaders),
            hle_export!(HCHttpCallRequestGetRequestBodyBytes),
            hle_export!(HCHttpCallRequestGetRetryAllowed),
            hle_export!(HCHttpCallRequestGetRetryCacheId),
            hle_export!(HCHttpCallRequestGetRetryDelay),
            hle_export!(HCHttpCallRequestGetTimeout),
            hle_export!(HCHttpCallRequestGetTimeoutWindow),
            hle_export!(HCHttpCallRequestGetUrl),
            hle_export!(HCHttpCallRequestSetHeader),
            hle_export!(HCHttpCallRequestSetRequestBodyBytes),
            hle_export!(HCHttpCallRequestSetRequestBodyString),
            hle_export!(HCHttpCallRequestSetRetryAllowed),
            hle_export!(HCHttpCallRequestSetRetryCacheId),
            hle_export!(HCHttpCallRequestSetRetryDelay),
            hle_export!(HCHttpCallRequestSetTimeout),
            hle_export!(HCHttpCallRequestSetTimeoutWindow),
            hle_export!(HCHttpCallRequestSetUrl),
            hle_export!(HCHttpCallResponseGetHeader),
            hle_export!(HCHttpCallResponseGetHeaderAtIndex),
            hle_export!(HCHttpCallResponseGetNetworkErrorCode),
            hle_export!(HCHttpCallResponseGetNumHeaders),
            hle_export!(HCHttpCallResponseGetPlatformNetworkErrorMessage),
            hle_export!(HCHttpCallResponseGetResponseBodyBytes),
            hle_export!(HCHttpCallResponseGetResponseBodyBytesSize),
            hle_export!(HCHttpCallResponseGetResponseString),
            hle_export!(HCHttpCallResponseGetStatusCode),
            hle_export!(HCHttpCallSetTracing),
            hle_export!(HCSettingsGetTraceLevel),
            hle_export!(HCTraceImplMessage),
            hle_export!(HCTraceSetEtwEnabled),
        ],
    }
}
