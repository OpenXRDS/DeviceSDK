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

extern "C" {

Runtime *xrds_Runtime_new();

RuntimeBuilder *xrds_Runtime_builder();

Runtime *xrds_RuntimeBuilder_build(RuntimeBuilder *builder);

void xrds_Runtime_Run(Runtime *runtime, CRuntimeHandler *runtime_handler, uint64_t user_private);

}  // extern "C"

#endif  // __XRDS_H__
