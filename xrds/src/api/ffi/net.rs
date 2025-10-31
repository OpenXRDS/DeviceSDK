use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
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
use xrds_net::common::data_structure::{NetResponse};

// FFI-safe handle types
pub type ClientHandle = usize;
pub type WebRTCHandle = usize;

// Error codes for FFI
pub const NET_SUCCESS: c_int = 0;
pub const NET_ERROR_INVALID_HANDLE: c_int = -1;
pub const NET_ERROR_INVALID_PARAM: c_int = -2;
pub const NET_ERROR_CONNECTION_FAILED: c_int = -3;
pub const NET_ERROR_TIMEOUT: c_int = -4;
pub const NET_ERROR_SESSION_FAILED: c_int = -5;
pub const NET_ERROR_STREAM_FAILED: c_int = -6;

// ============================================================================
// FACTORY + SINGLETON PATTERN: NetManager
// ============================================================================

pub struct NetManager {
    clients: Arc<Mutex<HashMap<usize, ClientWrapper>>>,
    webrtc_clients: Arc<Mutex<HashMap<usize, WebRTCClient>>>,
    next_handle_id: AtomicUsize,
    shutdown_flag: AtomicBool,
    active_operations: AtomicUsize,
    runtime: tokio::runtime::Runtime,
}

impl NetManager {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let runtime = tokio::runtime::Runtime::new()?;
        
        Ok(NetManager {
            clients: Arc::new(Mutex::new(HashMap::new())),
            webrtc_clients: Arc::new(Mutex::new(HashMap::new())),
            next_handle_id: AtomicUsize::new(1),
            shutdown_flag: AtomicBool::new(false),
            active_operations: AtomicUsize::new(0),
            runtime,
        })
    }

    pub fn instance() -> &'static NetManager {
        static INSTANCE: OnceLock<NetManager> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            NetManager::new().expect("Failed to create NetManager")
        })
    }

    pub fn create_client(&self, protocol: PROTOCOLS) -> Result<ClientHandle, &'static str> {
        if self.is_shutdown_requested() {
            return Err("Shutdown in progress");
        }

        let client = ClientBuilder::new()
            .set_protocol(protocol)
            .build();
        
        let wrapper = ClientWrapper::new(client);
        let handle = self.next_handle();

        self.runtime.block_on(async {
            let mut clients = self.clients.lock().await;
            clients.insert(handle, wrapper);
        });

        Ok(handle)
    }

    pub fn create_webrtc_client(&self) -> Result<WebRTCHandle, &'static str> {
        if self.is_shutdown_requested() {
            return Err("Shutdown in progress");
        }

        let client = WebRTCClient::new();
        let handle = self.next_handle();

        self.runtime.block_on(async {
            let mut clients = self.webrtc_clients.lock().await;
            clients.insert(handle, client);
        });

        Ok(handle)
    }

    pub fn destroy_client(&self, handle: ClientHandle) -> bool {
        self.runtime.block_on(async {
            let mut clients = self.clients.lock().await;
            clients.remove(&handle).is_some()
        })
    }

    pub fn destroy_webrtc_client(&self, handle: WebRTCHandle) -> bool {
        self.runtime.block_on(async {
            let mut clients = self.webrtc_clients.lock().await;
            clients.remove(&handle).is_some()
        })
    }

    pub async fn with_client<F, R>(&self, handle: ClientHandle, f: F) -> Result<R, &'static str>
    where
        F: FnOnce(&mut ClientWrapper) -> R,
    {
        let mut clients = self.clients.lock().await;
        match clients.get_mut(&handle) {
            Some(wrapper) => Ok(f(wrapper)),
            None => Err("Invalid handle"),
        }
    }

    #[allow(dead_code)]
    pub async fn with_webrtc_client<F, R>(&self, handle: WebRTCHandle, f: F) -> Result<R, &'static str>
    where
        F: FnOnce(&mut WebRTCClient) -> R,
    {
        let mut clients = self.webrtc_clients.lock().await;
        match clients.get_mut(&handle) {
            Some(client) => Ok(f(client)),
            None => Err("Invalid handle"),
        }
    }

    #[allow(dead_code)]
    pub async fn with_webrtc_client_async<F, Fut, R>(&self, handle: WebRTCHandle, f: F) -> Result<R, &'static str>
    where
        F: FnOnce(&mut WebRTCClient) -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        let mut clients = self.webrtc_clients.lock().await;
        match clients.get_mut(&handle) {
            Some(client) => Ok(f(client).await),
            None => Err("Invalid handle"),
        }
    }

    // ========================================================================
    // LIFECYCLE MANAGEMENT
    // ========================================================================

    fn next_handle(&self) -> usize {
        self.next_handle_id.fetch_add(1, Ordering::AcqRel)
    }

    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_flag.load(Ordering::Acquire)
    }

    pub fn request_shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::Release);
    }

    pub fn reset_shutdown(&self) {
        self.shutdown_flag.store(false, Ordering::Release);
    }

    pub fn increment_operations(&self) {
        self.active_operations.fetch_add(1, Ordering::AcqRel);
    }

    pub fn decrement_operations(&self) {
        self.active_operations.fetch_sub(1, Ordering::AcqRel);
    }

    pub fn active_operations_count(&self) -> usize {
        self.active_operations.load(Ordering::Acquire)
    }

    pub async fn cleanup_with_timeout(&self, timeout_seconds: u64) -> Result<(), &'static str> {
        // Set shutdown flag
        self.request_shutdown();
        
        // Wait for active operations to complete
        let start = std::time::Instant::now();
        while self.active_operations_count() > 0 {
            if start.elapsed().as_secs() > timeout_seconds {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Clean up clients with timeout
        match timeout(Duration::from_secs(5), async {
            self.clients.lock().await.clear();
            self.webrtc_clients.lock().await.clear();
        }).await {
            Ok(_) => Ok(()),
            Err(_) => Err("Timeout during cleanup"),
        }
    }

    pub fn block_on<F>(&self, future: F) -> F::Output 
    where
        F: std::future::Future,
    {
        self.runtime.block_on(future)
    }
}

pub struct OperationGuard;

impl OperationGuard {
    pub fn new() -> Option<Self> {
        let manager = NetManager::instance();
        if manager.is_shutdown_requested() {
            return None;
        }
        
        manager.increment_operations();
        Some(OperationGuard)
    }
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        NetManager::instance().decrement_operations();
    }
}

// ============================================================================
// C-COMPATIBLE RESPONSE STRUCTURES (same as before)
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

unsafe impl Send for CNetHeader {}
unsafe impl Sync for CNetHeader {}

pub struct ClientWrapper {
    client: Client,
    last_response: Option<NetResponse>,
    response_body: Option<CString>,
    response_headers: Option<Vec<CNetHeader>>,
    response_header_strings: Option<Vec<CString>>,
    response_error: Option<CString>,
}

unsafe impl Sync for ClientWrapper {}

#[allow(dead_code)]
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

    // Direct client access methods
    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn client_mut(&mut self) -> &mut Client {
        &mut self.client
    }

    // Configuration methods that work directly on the client
    pub fn set_url(&mut self, url: &str) -> &mut Self {
        self.client = self.client.clone().set_url(url);
        self
    }

    pub fn set_method(&mut self, method: &str) -> &mut Self {
        self.client = self.client.clone().set_method(method);
        self
    }

    pub fn set_user(&mut self, user: &str) -> &mut Self {
        self.client = self.client.clone().set_user(user);
        self
    }

    pub fn set_password(&mut self, password: &str) -> &mut Self {
        self.client = self.client.clone().set_password(password);
        self
    }

    pub fn set_req_body(&mut self, body: &str) -> &mut Self {
        self.client = self.client.clone().set_req_body(body);
        self
    }

    pub fn set_header(&mut self, key: &str, value: &str) -> &mut Self {
        self.client = self.client.clone().set_req_headers(vec![(key, value)]);
        self
    }

    pub fn set_timeout(&mut self, timeout: u64) -> &mut Self {
        self.client = self.client.clone().set_timeout(timeout);
        self
    }

    // Request method that stores response
    pub fn request(&mut self) -> Result<(), String> {
        let response = self.client.clone().request();
        
        let has_error = response.error.is_some();
        let error_msg = response.error.clone();
        
        self.store_response(response);
        
        if has_error {
            Err(error_msg.unwrap())
        } else {
            Ok(())
        }
    }

    // Convenience HTTP methods
    pub fn get(&mut self) -> Result<(), String> {
        self.set_method("GET").request()
    }

    pub fn post(&mut self) -> Result<(), String> {
        self.set_method("POST").request()
    }

    pub fn put(&mut self) -> Result<(), String> {
        self.set_method("PUT").request()
    }

    pub fn delete(&mut self) -> Result<(), String> {
        self.set_method("DELETE").request()
    }

    // Connection method for persistent protocols
    pub fn connect(&mut self) -> Result<(), String> {
        match self.client.clone().connect() {
            Ok(connected_client) => {
                self.client = connected_client;
                Ok(())
            }
            Err(e) => Err(format!("Connection failed: {:?}", e)),
        }
    }

    // Response storage and access (same as before)
    pub fn store_response(&mut self, response: NetResponse) {
        // Safe CString creation
        self.response_body = match CString::new(response.body.clone()) {
            Ok(cstr) => Some(cstr),
            Err(_) => Some(CString::new("[Response contains null bytes]").unwrap()),
        };
        
        self.response_error = if let Some(error) = &response.error {
            match CString::new(error.clone()) {
                Ok(cstr) => Some(cstr),
                Err(_) => Some(CString::new("[Error contains null bytes]").unwrap()),
            }
        } else {
            Some(CString::new("").unwrap())
        };
        
        // Convert headers to C-compatible format
        let mut header_strings = Vec::new();
        let mut c_headers = Vec::new();
        
        for (name, value) in &response.headers {
            let name_cstring = match CString::new(name.clone()) {
                Ok(cstr) => cstr,
                Err(_) => continue, // Skip invalid headers
            };
            
            let value_cstring = match CString::new(value.clone()) {
                Ok(cstr) => cstr,
                Err(_) => CString::new("[Value contains null bytes]").unwrap(),
            };
            
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
// FFI FUNCTIONS - Now much cleaner using the factory pattern
// ============================================================================

#[no_mangle]
pub extern "C" fn net_init() -> c_int {
    let manager = NetManager::instance();
    manager.reset_shutdown();
    NET_SUCCESS
}

#[no_mangle]
pub extern "C" fn net_cleanup() -> c_int {
    net_cleanup_with_timeout(30)
}

#[no_mangle]
pub extern "C" fn net_cleanup_with_timeout(timeout_seconds: c_int) -> c_int {
    let manager = NetManager::instance();
    
    match manager.block_on(manager.cleanup_with_timeout(timeout_seconds as u64)) {
        Ok(_) => NET_SUCCESS,
        Err(_) => NET_ERROR_TIMEOUT,
    }
}

// ============================================================================
// CLIENT FACTORY FUNCTIONS
// ============================================================================

#[no_mangle]
pub extern "C" fn client_create(protocol_val: c_int) -> ClientHandle {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return 0,
    };

    let protocol = match match_protocol_enum(protocol_val) {
        Some(p) => p,
        None => return 0,
    };
    
    if protocol == PROTOCOLS::WEBRTC {
        return 0; // Use webrtc_client_create instead
    }

    let manager = NetManager::instance();
    manager.create_client(protocol).unwrap_or_default()
}

#[no_mangle]
pub extern "C" fn client_destroy(handle: ClientHandle) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 {
        return NET_ERROR_INVALID_HANDLE;
    }

    let manager = NetManager::instance();
    if manager.destroy_client(handle) {
        NET_SUCCESS
    } else {
        NET_ERROR_INVALID_HANDLE
    }
}

#[no_mangle]
pub extern "C" fn webrtc_client_create() -> WebRTCHandle {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return 0,
    };

    let manager = NetManager::instance();
    manager.create_webrtc_client().unwrap_or_default()
}

#[no_mangle]
pub extern "C" fn webrtc_client_destroy(handle: WebRTCHandle) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 {
        return NET_ERROR_INVALID_HANDLE;
    }

    let manager = NetManager::instance();
    if manager.destroy_webrtc_client(handle) {
        NET_SUCCESS
    } else {
        NET_ERROR_INVALID_HANDLE
    }
}

// ============================================================================
// CLIENT CONFIGURATION FUNCTIONS - Using direct client access
// ============================================================================

#[no_mangle]
pub extern "C" fn client_set_url(handle: ClientHandle, url: *const c_char) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || url.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }

    let url_str = unsafe {
        match CStr::from_ptr(url).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        }
    };

    let manager = NetManager::instance();
    
    match manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.set_url(url_str);
    })) {
        Ok(_) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn client_set_method(handle: ClientHandle, method: *const c_char) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || method.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }

    let method_str = unsafe {
        match CStr::from_ptr(method).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        }
    };

    let manager = NetManager::instance();
    
    match manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.set_method(method_str);
    })) {
        Ok(_) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn client_set_user(handle: ClientHandle, username: *const c_char) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || username.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }

    let user = unsafe {
        match CStr::from_ptr(username).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        }
    };

    let manager = NetManager::instance();
    
    match manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.set_user(user);
    })) {
        Ok(_) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn client_set_password(handle: ClientHandle, password: *const c_char) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || password.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }

    let pass = unsafe {
        match CStr::from_ptr(password).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        }
    };

    let manager = NetManager::instance();
    
    match manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.set_password(pass);
    })) {
        Ok(_) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn client_set_req_body(handle: ClientHandle, body: *const c_char) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || body.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }

    let body_str = unsafe {
        match CStr::from_ptr(body).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        }
    };

    let manager = NetManager::instance();
    
    match manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.set_req_body(body_str);
    })) {
        Ok(_) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn client_set_header(handle: ClientHandle, key: *const c_char, value: *const c_char) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || key.is_null() || value.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }

    let (key_str, value_str) = unsafe {
        let key = match CStr::from_ptr(key).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        let value = match CStr::from_ptr(value).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        };
        
        (key, value)
    };

    let manager = NetManager::instance();
    
    match manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.set_header(key_str, value_str);
    })) {
        Ok(_) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn client_set_timeout(handle: ClientHandle, timeout_seconds: c_int) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || timeout_seconds < 0 {
        return NET_ERROR_INVALID_PARAM;
    }

    let manager = NetManager::instance();
    
    match manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.set_timeout(timeout_seconds as u64);
    })) {
        Ok(_) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

// ============================================================================
// REQUEST METHODS - Using direct client access
// ============================================================================

#[no_mangle]
pub extern "C" fn client_request(handle: ClientHandle) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 {
        return NET_ERROR_INVALID_HANDLE;
    }

    let manager = NetManager::instance();
    
    match manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.request()
    })) {
        Ok(Ok(_)) => NET_SUCCESS,
        Ok(Err(_)) => NET_ERROR_CONNECTION_FAILED,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn client_connect(handle: ClientHandle, server_url: *const c_char) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || server_url.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }

    let url = unsafe {
        match CStr::from_ptr(server_url).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        }
    };

    let manager = NetManager::instance();
    
    match manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.set_url(url).connect()
    })) {
        Ok(Ok(_)) => NET_SUCCESS,
        Ok(Err(_)) => NET_ERROR_CONNECTION_FAILED,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}


// ============================================================================
// CONVENIENCE HTTP METHODS
// ============================================================================

#[no_mangle]
pub extern "C" fn client_get_request(handle: ClientHandle) -> CNetResponse {
    let manager = NetManager::instance();
    
    let result = manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.get()
    }));

    match result {
        Ok(Ok(_)) => manager.block_on(manager.with_client(handle, |wrapper| {
            wrapper.get_c_response()
        })).unwrap_or_else(|_| error_response(NET_ERROR_INVALID_HANDLE)),
        Ok(Err(_)) => manager.block_on(manager.with_client(handle, |wrapper| {
            wrapper.get_c_response()
        })).unwrap_or_else(|_| error_response(NET_ERROR_CONNECTION_FAILED)),
        Err(_) => error_response(NET_ERROR_INVALID_HANDLE),
    }
}

#[no_mangle]
pub extern "C" fn client_post_request(handle: ClientHandle) -> CNetResponse {
    let manager = NetManager::instance();
    
    let result = manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.post()
    }));

    match result {
        Ok(Ok(_)) => manager.block_on(manager.with_client(handle, |wrapper| {
            wrapper.get_c_response()
        })).unwrap_or_else(|_| error_response(NET_ERROR_INVALID_HANDLE)),
        Ok(Err(_)) => manager.block_on(manager.with_client(handle, |wrapper| {
            wrapper.get_c_response()
        })).unwrap_or_else(|_| error_response(NET_ERROR_CONNECTION_FAILED)),
        Err(_) => error_response(NET_ERROR_INVALID_HANDLE),
    }
}

#[no_mangle]
pub extern "C" fn client_put_request(handle: ClientHandle) -> CNetResponse {
    let manager = NetManager::instance();
    
    let result = manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.put()
    }));

    match result {
        Ok(Ok(_)) => manager.block_on(manager.with_client(handle, |wrapper| {
            wrapper.get_c_response()
        })).unwrap_or_else(|_| error_response(NET_ERROR_INVALID_HANDLE)),
        Ok(Err(_)) => manager.block_on(manager.with_client(handle, |wrapper| {
            wrapper.get_c_response()
        })).unwrap_or_else(|_| error_response(NET_ERROR_CONNECTION_FAILED)),
        Err(_) => error_response(NET_ERROR_INVALID_HANDLE),
    }
}

#[no_mangle]
pub extern "C" fn client_delete_request(handle: ClientHandle) -> CNetResponse {
    let manager = NetManager::instance();
    
    let result = manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.delete()
    }));

    match result {
        Ok(Ok(_)) => manager.block_on(manager.with_client(handle, |wrapper| {
            wrapper.get_c_response()
        })).unwrap_or_else(|_| error_response(NET_ERROR_INVALID_HANDLE)),
        Ok(Err(_)) => manager.block_on(manager.with_client(handle, |wrapper| {
            wrapper.get_c_response()
        })).unwrap_or_else(|_| error_response(NET_ERROR_CONNECTION_FAILED)),
        Err(_) => error_response(NET_ERROR_INVALID_HANDLE),
    }
}

// ============================================================================
// WEBRTC CLIENT FUNCTIONS
// ============================================================================

// ============================================================================
// WEBRTC FUNCTIONS - Using direct client access
// ============================================================================

#[no_mangle]
pub extern "C" fn webrtc_connect_to_signaling_server(handle: WebRTCHandle, server_url: *const c_char) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || server_url.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }

    let url = unsafe {
        match CStr::from_ptr(server_url).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        }
    };

    let manager = NetManager::instance();
    let url_owned = url.to_string(); // Move this outside the closure
    
    // Use the direct async block approach instead of with_webrtc_client_async
    match manager.block_on(async {
        let mut clients = manager.webrtc_clients.lock().await;
        match clients.get_mut(&handle) {
            Some(client) => {
                client.connect_to_signaling_server(&url_owned).await
            }
            None => Err("Invalid handle".into()),
        }
    }) {
        Ok(()) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn webrtc_create_session(
    handle: WebRTCHandle,
    session_id_out: *mut c_char,
    session_id_len: c_int
) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || session_id_out.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }

    let manager = NetManager::instance();

    match manager.block_on(async {
        let mut clients = manager.webrtc_clients.lock().await;
        match clients.get_mut(&handle) {
            Some(client) => {
                match client.create_session().await {
                    Ok(session_id) => Ok(session_id),
                    Err(e) => Err(e),
                }
            }
            None => Err("Invalid handle".into()),
        }
    }) {
        Ok(session_id) => {
            let c_session_id = match CString::new(session_id) {
                Ok(cstr) => cstr,
                Err(_) => return NET_ERROR_SESSION_FAILED,
            };
            let session_bytes = c_session_id.as_bytes_with_nul();
            
            if session_bytes.len() <= session_id_len as usize {
                unsafe {
                    ptr::copy_nonoverlapping(
                        session_bytes.as_ptr() as *const c_char,
                        session_id_out,
                        session_bytes.len()
                    );
                }
                NET_SUCCESS
            } else {
                NET_ERROR_INVALID_PARAM
            }
        }
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn webrtc_join_session(handle: WebRTCHandle, session_id: *const c_char) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || session_id.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }

    let session = unsafe {
        match CStr::from_ptr(session_id).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        }
    };

    let manager = NetManager::instance();
    let session_owned = session.to_string();
    
    match manager.block_on(async {
        let mut clients = manager.webrtc_clients.lock().await;
        match clients.get_mut(&handle) {
            Some(client) => {
                client.join_session(&session_owned).await
            }
            None => Err("Invalid handle".into()),
        }
    }) {
        Ok(()) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn webrtc_publish_session(handle: WebRTCHandle, session_id: *const c_char) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || session_id.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }

    let session = unsafe {
        match CStr::from_ptr(session_id).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        }
    };

    let manager = NetManager::instance();
    let session_owned = session.to_string();
    
    match manager.block_on(async {
        let mut clients = manager.webrtc_clients.lock().await;
        match clients.get_mut(&handle) {
            Some(client) => {
                match client.publish(&session_owned).await {
                    Ok(_msg) => {
                        Ok(())                  
                    },
                    Err(e) => Err(e),
                }
            }
            None => Err("Invalid handle".into()),
        }
    }) {
        Ok(()) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn webrtc_start_webcam_stream(handle: WebRTCHandle, camera_index: c_int) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 {
        return NET_ERROR_INVALID_PARAM;
    }

    let manager = NetManager::instance();
    
    match manager.block_on(async {
        let mut clients = manager.webrtc_clients.lock().await;
        match clients.get_mut(&handle) {
            Some(client) => {
                let source = StreamSource::Webcam(camera_index as u32);
                client.start_streaming(Some(source)).await
            }
            None => Err("Invalid handle".into()),
        }
    }) {
        Ok(()) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn webrtc_start_file_stream(handle: WebRTCHandle, file_path: *const c_char) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 || file_path.is_null() {
        return NET_ERROR_INVALID_PARAM;
    }

    let path = unsafe {
        match CStr::from_ptr(file_path).to_str() {
            Ok(s) => s,
            Err(_) => return NET_ERROR_INVALID_PARAM,
        }
    };

    let manager = NetManager::instance();
    let path_owned = path.to_string();
    
    match manager.block_on(async {
        let mut clients = manager.webrtc_clients.lock().await;
        match clients.get_mut(&handle) {
            Some(client) => {
                let source = StreamSource::File(path_owned);
                client.start_streaming(Some(source)).await
            }
            None => Err("Invalid handle".into()),
        }
    }) {
        Ok(()) => NET_SUCCESS,
        Err(_) => NET_ERROR_STREAM_FAILED,
    }
}

#[no_mangle]
pub extern "C" fn webrtc_stop_stream(handle: WebRTCHandle) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 {
        return NET_ERROR_INVALID_PARAM;
    }

    let manager = NetManager::instance();
    
    match manager.block_on(async {
        let mut clients = manager.webrtc_clients.lock().await;
        match clients.get_mut(&handle) {
            Some(client) => {
                match client.stop_stream().await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e),
                }
            }
            None => Err("Invalid handle".into()),
        }
    }) {
        Ok(()) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

#[no_mangle]
pub extern "C" fn webrtc_wait_for_subscriber(handle: WebRTCHandle, timeout_seconds: c_int) -> c_int {
    let _guard = match OperationGuard::new() {
        Some(guard) => guard,
        None => return NET_ERROR_INVALID_HANDLE,
    };

    if handle == 0 {
        return NET_ERROR_INVALID_PARAM;
    }

    let manager = NetManager::instance();
    
    match manager.block_on(async {
        let mut clients = manager.webrtc_clients.lock().await;
        match clients.get_mut(&handle) {
            Some(client) => {
                match client.wait_for_subscriber(timeout_seconds as u64).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e),
                }
            }
            None => Err("Invalid handle".into()),
        }
    }) {
        Ok(()) => NET_SUCCESS,
        Err(_) => NET_ERROR_INVALID_HANDLE,
    }
}

// ============================================================================
// RESPONSE ACCESS
// ============================================================================

#[no_mangle]
pub extern "C" fn client_get_response(handle: ClientHandle) -> CNetResponse {
    if handle == 0 {
        return error_response(NET_ERROR_INVALID_HANDLE);
    }

    let manager = NetManager::instance();
    
    match manager.block_on(manager.with_client(handle, |wrapper| {
        wrapper.get_c_response()
    })) {
        Ok(response) => response,
        Err(_) => error_response(NET_ERROR_INVALID_HANDLE),
    }
}

#[no_mangle]
pub extern "C" fn net_is_shutdown_requested() -> c_int {
    let manager = NetManager::instance();
    if manager.is_shutdown_requested() { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn net_get_active_operations_count() -> c_int {
    let manager = NetManager::instance();
    manager.active_operations_count() as c_int
}

#[no_mangle]
pub extern "C" fn net_force_shutdown() -> c_int {
    let manager = NetManager::instance();
    manager.request_shutdown();
    
    std::thread::sleep(Duration::from_millis(100));
    
    manager.block_on(async {
        manager.clients.lock().await.clear();
        manager.webrtc_clients.lock().await.clear();
    });
    
    NET_SUCCESS
}

#[no_mangle]
pub extern "C" fn net_get_error_message(error_code: c_int) -> *const c_char {
    get_error_cstring(error_code).as_ptr()
}

fn get_error_cstring(error_code: c_int) -> &'static CString {
    static SUCCESS: OnceLock<CString> = OnceLock::new();
    static INVALID_HANDLE: OnceLock<CString> = OnceLock::new();
    static INVALID_PARAM: OnceLock<CString> = OnceLock::new();
    static CONNECTION_FAILED: OnceLock<CString> = OnceLock::new();
    static TIMEOUT: OnceLock<CString> = OnceLock::new();
    static SESSION_FAILED: OnceLock<CString> = OnceLock::new();
    static STREAM_FAILED: OnceLock<CString> = OnceLock::new();
    static UNKNOWN_ERROR: OnceLock<CString> = OnceLock::new();

    match error_code {
        NET_SUCCESS => SUCCESS.get_or_init(|| CString::new("Success").unwrap()),
        NET_ERROR_INVALID_HANDLE => INVALID_HANDLE.get_or_init(|| CString::new("Invalid handle").unwrap()),
        NET_ERROR_INVALID_PARAM => INVALID_PARAM.get_or_init(|| CString::new("Invalid parameter").unwrap()),
        NET_ERROR_CONNECTION_FAILED => CONNECTION_FAILED.get_or_init(|| CString::new("Connection failed").unwrap()),
        NET_ERROR_TIMEOUT => TIMEOUT.get_or_init(|| CString::new("Operation timed out").unwrap()),
        NET_ERROR_SESSION_FAILED => SESSION_FAILED.get_or_init(|| CString::new("Session operation failed").unwrap()),
        NET_ERROR_STREAM_FAILED => STREAM_FAILED.get_or_init(|| CString::new("Stream operation failed").unwrap()),
        _ => UNKNOWN_ERROR.get_or_init(|| CString::new("Unknown error").unwrap()),
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

fn error_response(error_code: c_int) -> CNetResponse {
    CNetResponse {
        status_code: error_code,
        body_ptr: ptr::null(),
        body_len: 0,
        headers_ptr: ptr::null(),
        headers_count: 0,
        error_ptr: ptr::null(),
        error_len: 0,
    }
}