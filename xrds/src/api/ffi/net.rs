use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;

use xrds_net::client::{Client, ClientBuilder};
use xrds_net::client::webrtc_client::{WebRTCClient, StreamSource};
use xrds_net::common::enums::PROTOCOLS;
use xrds_net::common::data_structure::NetResponse;

// Add shutdown flag and operation counter
static SHUTDOWN_FLAG: AtomicBool = AtomicBool::new(false);
static ACTIVE_OPERATIONS: AtomicUsize = AtomicUsize::new(0);

// FFI-safe handle types
pub type ClientHandle = *mut c_void;
pub type WebRTCHandle = *mut c_void;

// Error codes for FFI
pub const NET_SUCCESS: c_int = 0;
pub const NET_ERROR_INVALID_HANDLE: c_int = -1;
pub const NET_ERROR_INVALID_PARAM: c_int = -2;
pub const NET_ERROR_CONNECTION_FAILED: c_int = -3;
pub const NET_ERROR_TIMEOUT: c_int = -4;
pub const NET_ERROR_SESSION_FAILED: c_int = -5;
pub const NET_ERROR_STREAM_FAILED: c_int = -6;

// Global runtime - create once, use everywhere
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn get_runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime")
    })
}
// Internal client storage - separate for each type
static mut NET_CLIENT_STORAGE: OnceLock<Arc<Mutex<HashMap<usize, ClientWrapper>>>> = OnceLock::new();
static mut WEBRTC_CLIENT_STORAGE: OnceLock<Arc<Mutex<HashMap<usize, WebRTCClient>>>> = OnceLock::new();
static NEXT_HANDLE_ID: AtomicUsize = AtomicUsize::new(1);

// ============================================================================
// INITIALIZATION AND CLEANUP
// ============================================================================

#[no_mangle]
pub extern "C" fn net_init() -> c_int {
    unsafe {
            // Reset shutdown flag
            SHUTDOWN_FLAG.store(false, Ordering::Release);
            
            if NET_CLIENT_STORAGE.get().is_none() {
                NET_CLIENT_STORAGE = OnceLock::new();
            }
            
            if WEBRTC_CLIENT_STORAGE.get().is_none() {
                WEBRTC_CLIENT_STORAGE = OnceLock::new();
            }
            
            // Initialize the runtime
            let _ = get_runtime();
            
            let _handle_id = NEXT_HANDLE_ID.fetch_add(1, Ordering::AcqRel);
            NET_SUCCESS
    }
}

#[no_mangle]
pub extern "C" fn net_cleanup() -> c_int {
    net_cleanup_with_timeout(30) // 30 second default timeout
}

#[no_mangle]
pub extern "C" fn net_cleanup_with_timeout(timeout_seconds: c_int) -> c_int {
    // Step 1: Set shutdown flag to prevent new operations
    SHUTDOWN_FLAG.store(true, Ordering::Release);
    
    // Step 2: Wait for active operations to complete
    let timeout_duration = Duration::from_secs(timeout_seconds as u64);
    let start_time = std::time::Instant::now();
    
    while ACTIVE_OPERATIONS.load(Ordering::Acquire) > 0 {
        if start_time.elapsed() > timeout_duration {
            // Force shutdown after timeout
            break;
        }
        
        // Brief sleep to avoid busy waiting
        // Step 3: Safely drop storage
        unsafe {
            if let Some(storage) = NET_CLIENT_STORAGE.get() {
                // Use timeout to avoid hanging on mutex locks
                if let Ok(runtime) = std::panic::catch_unwind(|| get_runtime()) {
                    let _ = runtime.block_on(async {
                        match timeout(Duration::from_secs(5), storage.lock()).await {
                            Ok(mut clients) => {
                                clients.clear();
                            }
                            Err(_) => {
                                // Timeout - force cleanup
                                eprintln!("Warning: Timeout during client storage cleanup");
                            }
                        }
                    });
                }
            }
            
            if let Some(storage) = WEBRTC_CLIENT_STORAGE.get() {
                if let Ok(runtime) = std::panic::catch_unwind(|| get_runtime()) {
                    let _ = runtime.block_on(async {
                        match timeout(Duration::from_secs(5), storage.lock()).await {
                            Ok(mut clients) => {
                                clients.clear();
                            }
                            Err(_) => {
                                eprintln!("Warning: Timeout during WebRTC storage cleanup");
                            }
                        }
                    });
                }
            }
        }
    }
    NET_SUCCESS
}

// ============================================================================
// BASIC CLIENT FUNCTIONS
// ============================================================================

#[no_mangle]
pub extern "C" fn client_create(protocol_val: c_int) -> ClientHandle {
    unsafe {
        // Initialize storage if needed
        let storage = NET_CLIENT_STORAGE.get_or_init(|| {
            Arc::new(Mutex::new(HashMap::new()))
        });
        
        let client_builder = ClientBuilder::new();
        let mut protocol_result = match_protocol_enum(protocol_val);
        if protocol_result.is_none() {
            return ptr::null_mut();
        }
        let protocol = protocol_result.unwrap();
        if protocol == PROTOCOLS::WEBRTC {
            return ptr::null_mut(); // Use webrtc_client_create instead
        }
        let client = client_builder.set_protocol(protocol).build();
        let wrapper = ClientWrapper::new(client);
        let handle_id = NEXT_HANDLE_ID.fetch_add(1, Ordering::AcqRel);
        
        get_runtime().block_on(async {
            let mut clients = storage.lock().await;
            clients.insert(handle_id, wrapper);
        });
        handle_id as ClientHandle
    }
}

#[no_mangle]
pub extern "C" fn client_destroy(handle: ClientHandle) -> c_int {
    if handle.is_null() {
        return NET_ERROR_INVALID_HANDLE;
    }
    
    unsafe {
        if let Some(storage) = NET_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                clients.remove(&handle_id);
            });
            NET_SUCCESS
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}


#[no_mangle]
pub extern "C" fn client_request(
    handle: ClientHandle,
) -> c_int {
    if handle.is_null() {
        return NET_ERROR_INVALID_HANDLE;
    }
    
    unsafe {
        if let Some(ref storage) = NET_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(wrapper) = clients.get_mut(&handle_id) {
                    let response = wrapper.client.clone().request();
                    
                    if response.error.is_some() {
                        wrapper.store_response(response);
                        NET_ERROR_CONNECTION_FAILED
                    } else {
                        wrapper.store_response(response);
                        NET_SUCCESS
                    }
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn client_connect(
    handle: ClientHandle,
    server_url: *const c_char
) -> c_int {
    if handle.is_null() || server_url.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        let url = match CStr::from_ptr(server_url).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        if let Some(ref storage) = NET_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(wrapper) = clients.get_mut(&handle_id) {
                    wrapper.client = wrapper.client.clone().set_url(url);
                    match wrapper.client.clone().connect() {
                        Ok(connected_client) => {
                            wrapper.client = connected_client;
                            NET_SUCCESS
                        },
                        Err(_) => NET_ERROR_CONNECTION_FAILED,
                    }
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn client_set_user(
    handle: ClientHandle,
    username: *const c_char
) -> c_int {
    if handle.is_null() || username.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        let user = match CStr::from_ptr(username).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        if let Some(ref storage) = NET_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(wrapper) = clients.get_mut(&handle_id) {
                    wrapper.client = wrapper.client.clone().set_user(user);
                    NET_SUCCESS
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn client_set_password(
    handle: ClientHandle,
    password: *const c_char
) -> c_int {
    if handle.is_null() || password.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        let pass = match CStr::from_ptr(password).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        if let Some(ref storage) = NET_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(wrapper) = clients.get_mut(&handle_id) {
                    wrapper.client = wrapper.client.clone().set_password(pass);
                    NET_SUCCESS
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn client_set_url(
    handle: ClientHandle,
    url: *const c_char
) -> c_int {
    if handle.is_null() || url.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        let url_str = match CStr::from_ptr(url).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        if let Some(ref storage) = NET_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(wrapper) = clients.get_mut(&handle_id) {
                    wrapper.client = wrapper.client.clone().set_url(url_str);
                    NET_SUCCESS
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn client_set_method(
    handle: ClientHandle,
    method: *const c_char
) -> c_int {
    if handle.is_null() || method.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        let method_str = match CStr::from_ptr(method).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        if let Some(ref storage) = NET_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(wrapper) = clients.get_mut(&handle_id) {
                    wrapper.client = wrapper.client.clone().set_method(method_str);
                    NET_SUCCESS
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn client_set_req_body(
    handle: ClientHandle,
    body: *const c_char
) -> c_int {
    if handle.is_null() || body.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        let request_body = match CStr::from_ptr(body).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        if let Some(ref storage) = NET_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(wrapper) = clients.get_mut(&handle_id) {
                    wrapper.client = wrapper.client.clone().set_req_body(request_body);
                    NET_SUCCESS
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn client_set_header(
    handle: ClientHandle,
    key: *const c_char,
    value: *const c_char
) -> c_int {
    if handle.is_null() || key.is_null() || value.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        let header_key = match CStr::from_ptr(key).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        let header_value = match CStr::from_ptr(value).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        if let Some(ref storage) = NET_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(wrapper) = clients.get_mut(&handle_id) {
                    wrapper.client = wrapper.client.clone().set_req_headers(vec![(header_key, header_value)]);
                    NET_SUCCESS
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn client_set_timeout(
    handle: ClientHandle,
    timeout_seconds: c_int
) -> c_int {
    if handle.is_null() || timeout_seconds < 0 {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        if let Some(ref storage) = NET_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(wrapper) = clients.get_mut(&handle_id) {
                    wrapper.client = wrapper.client.clone().set_timeout(timeout_seconds as u64);
                    NET_SUCCESS
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}


// ============================================================================
// WEBRTC CLIENT FUNCTIONS
// ============================================================================

#[no_mangle]
pub extern "C" fn webrtc_client_create() -> WebRTCHandle {
    unsafe {
        // Initialize storage if needed
        let storage = WEBRTC_CLIENT_STORAGE.get_or_init(|| {
            Arc::new(Mutex::new(HashMap::new()))
        });
        
        let client = WebRTCClient::new();
        let handle_id = NEXT_HANDLE_ID.fetch_add(1, Ordering::AcqRel);
        
        get_runtime().block_on(async {
            let mut clients = storage.lock().await;
            clients.insert(handle_id, client);
        });
        
        handle_id as WebRTCHandle
    }
}
pub extern "C" fn webrtc_client_destroy(handle: WebRTCHandle) -> c_int {
    if handle.is_null() {
        return NET_ERROR_INVALID_HANDLE;
    }
    
    unsafe {
        if let Some(ref storage) = WEBRTC_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                clients.remove(&handle_id);
            });
            NET_SUCCESS
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn webrtc_connect_to_signaling_server(
    handle: WebRTCHandle, 
    server_url: *const c_char
) -> c_int {
    if handle.is_null() || server_url.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        let url = match CStr::from_ptr(server_url).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        if let Some(ref storage) = WEBRTC_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(client) = clients.get_mut(&handle_id) {
                    match client.connect_to_signaling_server(url).await {
                        Ok(_) => NET_SUCCESS,
                        Err(_) => NET_ERROR_CONNECTION_FAILED,
                    }
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn webrtc_create_session(
    handle: WebRTCHandle,
    session_id_out: *mut c_char,
    session_id_len: c_int
) -> c_int {
    if handle.is_null() || session_id_out.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        if let Some(ref storage) = WEBRTC_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(client) = clients.get_mut(&handle_id) {
                    match client.create_session().await {
                        Ok(session_id) => {
                            let c_session_id = match CString::new(session_id) {
                                Ok(cstr) => cstr,
                                Err(_) => return NET_ERROR_SESSION_FAILED, // Contains null bytes
                            };
                            let session_bytes = c_session_id.as_bytes_with_nul();
                            
                            if session_bytes.len() <= session_id_len as usize {
                                ptr::copy_nonoverlapping(
                                    session_bytes.as_ptr() as *const c_char,
                                    session_id_out,
                                    session_bytes.len()
                                );
                                NET_SUCCESS
                            } else {
                                NET_ERROR_INVALID_PARAM // Buffer too small
                            }
                        }
                        Err(_) => NET_ERROR_SESSION_FAILED,
                    }
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn webrtc_join_session(
    handle: WebRTCHandle,
    session_id: *const c_char
) -> c_int {
    if handle.is_null() || session_id.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        let session = match CStr::from_ptr(session_id).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        if let Some(ref storage) = WEBRTC_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(client) = clients.get_mut(&handle_id) {
                    match client.join_session(session).await {
                        Ok(_) => NET_SUCCESS,
                        Err(_) => NET_ERROR_SESSION_FAILED,
                    }
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn webrtc_publish_session(
    handle: WebRTCHandle,
    session_id: *const c_char
) -> c_int {
    if handle.is_null() || session_id.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        let session = match CStr::from_ptr(session_id).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        if let Some(ref storage) = WEBRTC_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(client) = clients.get_mut(&handle_id) {
                    match client.publish(session).await {
                        Ok(_) => NET_SUCCESS,
                        Err(_) => NET_ERROR_SESSION_FAILED,
                    }
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn webrtc_start_webcam_stream(
    handle: WebRTCHandle,
    camera_index: c_int
) -> c_int {
    if handle.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        if let Some(ref storage) = WEBRTC_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(client) = clients.get_mut(&handle_id) {
                    let source = StreamSource::Webcam(camera_index as u32);
                    match client.start_streaming(Some(source)).await {
                        Ok(_) => NET_SUCCESS,
                        Err(_) => NET_ERROR_STREAM_FAILED,
                    }
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn webrtc_start_file_stream(
    handle: WebRTCHandle,
    file_path: *const c_char
) -> c_int {
    if handle.is_null() || file_path.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        let path = match CStr::from_ptr(file_path).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        if let Some(ref storage) = WEBRTC_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(client) = clients.get_mut(&handle_id) {
                    let source = StreamSource::File(path.to_string());
                    match client.start_streaming(Some(source)).await {
                        Ok(_) => NET_SUCCESS,
                        Err(_) => NET_ERROR_STREAM_FAILED,
                    }
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn webrtc_stop_stream(handle: WebRTCHandle) -> c_int {
    if handle.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        if let Some(ref storage) = WEBRTC_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(client) = clients.get_mut(&handle_id) {
                    match client.stop_stream().await {
                        Ok(_) => NET_SUCCESS,
                        Err(_) => NET_ERROR_STREAM_FAILED,
                    }
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

#[no_mangle]
pub extern "C" fn webrtc_wait_for_subscriber(
    handle: WebRTCHandle,
    timeout_seconds: c_int
) -> c_int {
    if handle.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        if let Some(ref storage) = WEBRTC_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let mut clients = storage.lock().await;
                if let Some(client) = clients.get_mut(&handle_id) {
                    match client.wait_for_subscriber(timeout_seconds as u64).await {
                        Ok(_) => NET_SUCCESS,
                        Err(_) => NET_ERROR_TIMEOUT,
                    }
                } else {
                    NET_ERROR_INVALID_HANDLE
                }
            })
        } else {
            NET_ERROR_INVALID_HANDLE
        }
    }
}

fn match_protocol_enum(protocol: c_int) -> Option<PROTOCOLS> {
    match protocol {
        0 => Some(PROTOCOLS::HTTP),
        1 => Some(PROTOCOLS::HTTPS),
        2 => Some(PROTOCOLS::FILE),
        3 => Some(PROTOCOLS::COAP),
        4 => Some(PROTOCOLS::MQTT),
        5 => Some(PROTOCOLS::FTP),
        6 => Some(PROTOCOLS::SFTP),
        7 => Some(PROTOCOLS::WS),
        8 => Some(PROTOCOLS::WSS),
        9 => Some(PROTOCOLS::WEBRTC),
        10 => Some(PROTOCOLS::HTTP3),
        11 => Some(PROTOCOLS::QUIC),
        _ => None,
    }
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

#[no_mangle]
pub extern "C" fn net_get_error_message(error_code: c_int) -> *const c_char {
    match error_code {
        NET_SUCCESS => "Success\0".as_ptr() as *const c_char,
        NET_ERROR_INVALID_HANDLE => "Invalid handle\0".as_ptr() as *const c_char,
        NET_ERROR_INVALID_PARAM => "Invalid parameter\0".as_ptr() as *const c_char,
        NET_ERROR_CONNECTION_FAILED => "Connection failed\0".as_ptr() as *const c_char,
        NET_ERROR_TIMEOUT => "Operation timed out\0".as_ptr() as *const c_char,
        NET_ERROR_SESSION_FAILED => "Session operation failed\0".as_ptr() as *const c_char,
        NET_ERROR_STREAM_FAILED => "Stream operation failed\0".as_ptr() as *const c_char,
        _ => "Unknown error\0".as_ptr() as *const c_char,
    }
}

// ============================================================================
// HIGH-LEVEL CONVENIENCE FUNCTIONS
// ============================================================================

#[no_mangle]
pub extern "C" fn webrtc_setup_publisher(
    server_url: *const c_char,
    camera_index: c_int,
    session_id_out: *mut c_char,
    session_id_len: c_int
) -> WebRTCHandle {
    if server_url.is_null() || session_id_out.is_null() {
        return ptr::null_mut();
    }
    
    let handle = webrtc_client_create();
    if handle.is_null() {
        return ptr::null_mut();
    }
    
    // Connect to server
    if webrtc_connect_to_signaling_server(handle, server_url) != NET_SUCCESS {
        webrtc_client_destroy(handle);
        return ptr::null_mut();
    }
    
    // Create session
    if webrtc_create_session(handle, session_id_out, session_id_len) != NET_SUCCESS {
        webrtc_client_destroy(handle);
        return ptr::null_mut();
    }
    
    // Publish session
    if webrtc_publish_session(handle, session_id_out) != NET_SUCCESS {
        webrtc_client_destroy(handle);
        return ptr::null_mut();
    }
    
    // Start streaming
    if webrtc_start_webcam_stream(handle, camera_index) != NET_SUCCESS {
        webrtc_client_destroy(handle);
        return ptr::null_mut();
    }
    
    handle
}

#[no_mangle]
pub extern "C" fn webrtc_setup_subscriber(
    server_url: *const c_char,
    session_id: *const c_char
) -> WebRTCHandle {
    if server_url.is_null() || session_id.is_null() {
        return ptr::null_mut();
    }
    
    let handle = webrtc_client_create();
    if handle.is_null() {
        return ptr::null_mut();
    }
    
    // Connect to server
    if webrtc_connect_to_signaling_server(handle, server_url) != NET_SUCCESS {
        webrtc_client_destroy(handle);
        return ptr::null_mut();
    }
    
    // Join session
    if webrtc_join_session(handle, session_id) != NET_SUCCESS {
        webrtc_client_destroy(handle);
        return ptr::null_mut();
    }
    
    handle
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_net_init_cleanup() {
        assert_eq!(net_init(), NET_SUCCESS);
        assert_eq!(net_cleanup(), NET_SUCCESS);
    }
    
    #[test]
    fn test_client_create_destroy() {
        net_init();
        
        let handle = client_create(1);
        assert!(!handle.is_null());
        
        assert_eq!(client_destroy(handle), NET_SUCCESS);
        
        net_cleanup();
    }
    
    #[test]
    fn test_webrtc_client_create_destroy() {
        net_init();
        
        let handle = webrtc_client_create();
        assert!(!handle.is_null());
        
        assert_eq!(webrtc_client_destroy(handle), NET_SUCCESS);
        
        net_cleanup();
    }
    
    #[test]
    fn test_error_messages() {
        let msg = net_get_error_message(NET_SUCCESS);
        assert!(!msg.is_null());
        
        unsafe {
            let c_str = CStr::from_ptr(msg);
            assert_eq!(c_str.to_str().unwrap(), "Success");
        }
    }
}

// ============================================================================
// C-COMPATIBLE RESPONSE STRUCTURES
// ============================================================================

#[repr(C)]
pub struct CNetResponse {
    pub status_code: c_int,
    pub body_ptr: *const c_char,
    pub body_len: c_int,
    pub headers_ptr: *const CNetHeader,
    pub headers_count: c_int,
    pub error_ptr: *const c_char,
    pub error_len: c_int,
}

#[repr(C)]
pub struct CNetHeader {
    pub name_ptr: *const c_char,
    pub name_len: c_int,
    pub value_ptr: *const c_char,
    pub value_len: c_int,
}

// ============================================================================
// CLIENT WRAPPER WITH RESPONSE STORAGE
// ============================================================================

pub struct ClientWrapper {
    client: Client,
    last_response: Option<NetResponse>,
    // Storage for C-compatible data (to keep pointers valid)
    response_body: Option<CString>,
    response_headers: Option<Vec<CNetHeader>>,
    response_header_strings: Option<Vec<CString>>,
    response_error: Option<CString>,
}

impl ClientWrapper {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            last_response: None,
            response_body: None,
            response_headers: None,
            response_header_strings: None,
            response_error: None,
        }
    }

    pub fn store_response(&mut self, response: NetResponse) {
        // Convert body to CString
        self.response_body = Some(CString::new(response.body.clone()).unwrap_or_default());
        
        // Convert error to CString if present
        self.response_error = if let Some(error) = &response.error {
            Some(CString::new(error.clone()).unwrap_or_default())
        } else {
            Some(CString::new("").unwrap())
        };
        
        // Convert headers to C-compatible format
        let mut header_strings = Vec::new();
        let mut c_headers = Vec::new();
        
        for (name, value) in &response.headers {
            let name_cstring = CString::new(name.clone()).unwrap_or_default();
            let value_cstring = CString::new(value.clone()).unwrap_or_default();
            
            let c_header = CNetHeader {
                name_ptr: name_cstring.as_ptr(),
                name_len: name.len() as c_int,
                value_ptr: value_cstring.as_ptr(),
                value_len: value.len() as c_int,
            };
            
            header_strings.push(name_cstring);
            header_strings.push(value_cstring);
            c_headers.push(c_header);
        }
        
        self.response_header_strings = Some(header_strings);
        self.response_headers = Some(c_headers);
        self.last_response = Some(response);
    }

    pub fn get_c_response(&self) -> CNetResponse {
        if let Some(ref response) = self.last_response {
            CNetResponse {
                status_code: response.status_code as c_int,
                body_ptr: self.response_body.as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(ptr::null()),
                body_len: response.body.len() as c_int,
                headers_ptr: self.response_headers.as_ref()
                    .map(|h| h.as_ptr())
                    .unwrap_or(ptr::null()),
                headers_count: response.headers.len() as c_int,
                error_ptr: self.response_error.as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(ptr::null()),
                error_len: response.error.as_ref()
                    .map(|e| e.len() as c_int)
                    .unwrap_or(0),
            }
        } else {
            CNetResponse {
                status_code: 0,
                body_ptr: ptr::null(),
                body_len: 0,
                headers_ptr: ptr::null(),
                headers_count: 0,
                error_ptr: ptr::null(),
                error_len: 0,
            }
        }
    }
}

// ============================================================================
// NEW STRUCT-BASED RESPONSE ACCESS
// ============================================================================

#[no_mangle]
pub extern "C" fn client_get_response(handle: ClientHandle) -> CNetResponse {
    if handle.is_null() {
        return CNetResponse {
            status_code: NET_ERROR_INVALID_HANDLE,
            body_ptr: ptr::null(),
            body_len: 0,
            headers_ptr: ptr::null(),
            headers_count: 0,
            error_ptr: ptr::null(),
            error_len: 0,
        };
    }
    
    unsafe {
        if let Some(storage) = NET_CLIENT_STORAGE.get() {
            let handle_id = handle as usize;
            
            get_runtime().block_on(async {
                let clients = storage.lock().await;
                if let Some(wrapper) = clients.get(&handle_id) {
                    wrapper.get_c_response()
                } else {
                    CNetResponse {
                        status_code: NET_ERROR_INVALID_HANDLE,
                        body_ptr: ptr::null(),
                        body_len: 0,
                        headers_ptr: ptr::null(),
                        headers_count: 0,
                        error_ptr: ptr::null(),
                        error_len: 0,
                    }
                }
            })
        } else {
            CNetResponse {
                status_code: NET_ERROR_INVALID_HANDLE,
                body_ptr: ptr::null(),
                body_len: 0,
                headers_ptr: ptr::null(),
                headers_count: 0,
                error_ptr: ptr::null(),
                error_len: 0,
            }
        }
    }
}

// Convenience HTTP method functions that return response directly
#[no_mangle]
pub extern "C" fn client_get_request(handle: ClientHandle) -> CNetResponse {
    client_set_method(handle, "GET\0".as_ptr() as *const c_char);
    client_request(handle);
    client_get_response(handle)
}

#[no_mangle]
pub extern "C" fn client_post_request(handle: ClientHandle) -> CNetResponse {
    client_set_method(handle, "POST\0".as_ptr() as *const c_char);
    client_request(handle);
    client_get_response(handle)
}

#[no_mangle]
pub extern "C" fn client_put_request(handle: ClientHandle) -> CNetResponse {
    client_set_method(handle, "PUT\0".as_ptr() as *const c_char);
    client_request(handle);
    client_get_response(handle)
}

#[no_mangle]
pub extern "C" fn client_delete_request(handle: ClientHandle) -> CNetResponse {
    client_set_method(handle, "DELETE\0".as_ptr() as *const c_char);
    client_request(handle);
    client_get_response(handle)
}

// ============================================================================
// MEMORY MANAGEMENT FOR C STRINGS
// ============================================================================

#[no_mangle]
pub extern "C" fn client_copy_response_body(
    handle: ClientHandle,
    buffer: *mut c_char,
    buffer_len: c_int
) -> c_int {
    let response = client_get_response(handle);
    
    if response.body_ptr.is_null() || buffer.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }
    
    unsafe {
        if response.body_len < buffer_len {
            ptr::copy_nonoverlapping(
                response.body_ptr,
                buffer,
                response.body_len as usize
            );
            *buffer.add(response.body_len as usize) = 0; // Null terminate
            response.body_len
        } else {
            NET_ERROR_INVALID_PARAM // Buffer too small
        }
    }
}

#[no_mangle]
pub extern "C" fn client_copy_response_error(
    handle: ClientHandle,
    buffer: *mut c_char,
    buffer_len: c_int
) -> c_int {
    let response = client_get_response(handle);
    
    if response.error_ptr.is_null() || buffer.is_null() {
        return 0; // No error
    }
    
    unsafe {
        if response.error_len < buffer_len {
            ptr::copy_nonoverlapping(
                response.error_ptr,
                buffer,
                response.error_len as usize
            );
            *buffer.add(response.error_len as usize) = 0; // Null terminate
            response.error_len
        } else {
            NET_ERROR_INVALID_PARAM // Buffer too small
        }
    }
}
// Update the rest of your existing functions (client_set_req_body, client_set_header, etc.)
// to work with ClientWrapper instead of Client directly...