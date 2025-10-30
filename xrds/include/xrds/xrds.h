// ***********************************
// Auto generated header
// ***********************************


#ifndef __XRDS_H__
#define __XRDS_H__

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

constexpr static const int NET_SUCCESS = 0;

constexpr static const int NET_ERROR_INVALID_HANDLE = -1;

constexpr static const int NET_ERROR_INVALID_PARAM = -2;

constexpr static const int NET_ERROR_CONNECTION_FAILED = -3;

constexpr static const int NET_ERROR_TIMEOUT = -4;

constexpr static const int NET_ERROR_SESSION_FAILED = -5;

constexpr static const int NET_ERROR_STREAM_FAILED = -6;

struct Runtime;

struct RuntimeBuilder;

struct CRuntimeHandler {
  void (*on_construct)(void*);
  void (*on_begin)(void*);
  void (*on_resumed)(void*);
  void (*on_suspended)(void*);
  void (*on_end)(void*);
  void (*on_update)(void*);
  void (*on_deconstruct)(void*);
};

using ClientHandle = void*;

using WebRTCHandle = void*;

struct CNetHeader {
  const char *name_ptr;
  int name_len;
  const char *value_ptr;
  int value_len;
};

struct CNetResponse {
  int status_code;
  const char *body_ptr;
  int body_len;
  const CNetHeader *headers_ptr;
  int headers_count;
  const char *error_ptr;
  int error_len;
};

extern "C" {

Runtime *xrds_Runtime_new();

RuntimeBuilder *xrds_Runtime_builder();

Runtime *xrds_RuntimeBuilder_build(RuntimeBuilder *builder);

void xrds_Runtime_Run(Runtime *runtime, CRuntimeHandler *runtime_handler, uint64_t user_private);

int net_init();

int net_cleanup();

int net_cleanup_with_timeout(int timeout_seconds);

ClientHandle client_create(int protocol_val);

int client_destroy(ClientHandle handle);

int client_request(ClientHandle handle);

int client_connect(ClientHandle handle, const char *server_url);

int client_set_user(ClientHandle handle, const char *username);

int client_set_password(ClientHandle handle, const char *password);

int client_set_url(ClientHandle handle, const char *url);

int client_set_method(ClientHandle handle, const char *method);

int client_set_req_body(ClientHandle handle, const char *body);

int client_set_header(ClientHandle handle, const char *key, const char *value);

int client_set_timeout(ClientHandle handle, int timeout_seconds);

WebRTCHandle webrtc_client_create();

int webrtc_connect_to_signaling_server(WebRTCHandle handle, const char *server_url);

int webrtc_create_session(WebRTCHandle handle, char *session_id_out, int session_id_len);

int webrtc_join_session(WebRTCHandle handle, const char *session_id);

int webrtc_publish_session(WebRTCHandle handle, const char *session_id);

int webrtc_start_webcam_stream(WebRTCHandle handle, int camera_index);

int webrtc_start_file_stream(WebRTCHandle handle, const char *file_path);

int webrtc_stop_stream(WebRTCHandle handle);

int webrtc_wait_for_subscriber(WebRTCHandle handle, int timeout_seconds);

const char *net_get_error_message(int error_code);

WebRTCHandle webrtc_setup_publisher(const char *server_url,
                                    int camera_index,
                                    char *session_id_out,
                                    int session_id_len);

WebRTCHandle webrtc_setup_subscriber(const char *server_url, const char *session_id);

CNetResponse client_get_response(ClientHandle handle);

CNetResponse client_get_request(ClientHandle handle);

CNetResponse client_post_request(ClientHandle handle);

CNetResponse client_put_request(ClientHandle handle);

CNetResponse client_delete_request(ClientHandle handle);

int client_copy_response_body(ClientHandle handle, char *buffer, int buffer_len);

int client_copy_response_error(ClientHandle handle, char *buffer, int buffer_len);

}  // extern "C"

#endif  // __XRDS_H__
