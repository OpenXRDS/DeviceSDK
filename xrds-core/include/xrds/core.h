// ***********************************
// Auto generated header
// ***********************************


#ifndef __XRDS_CORE_H__
#define __XRDS_CORE_H__

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

struct xrds_HelloStruct;

extern "C" {

xrds_HelloStruct *xrds_core_new_hello(uint64_t x, uint64_t y);

void xrds_core_destroy_hello(xrds_HelloStruct *ptr);

void xrds_core_hello_rust(const xrds_HelloStruct *st);

}  // extern "C"

#endif  // __XRDS_CORE_H__
