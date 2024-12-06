/*
 Copyright 2024 OpenXRDS

 Licensed under the Apache License, Version 2.0 (the "License");
 you may not use this file except in compliance with the License.
 You may obtain a copy of the License at

      https://www.apache.org/licenses/LICENSE-2.0

 Unless required by applicable law or agreed to in writing, software
 distributed under the License is distributed on an "AS IS" BASIS,
 WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 See the License for the specific language governing permissions and
 limitations under the License.
 */
#ifndef __XRDS_CORE_HPP__
#define __XRDS_CORE_HPP__

#include "xrds/core.h"

namespace xrds {

struct HelloStruct {
    HelloStruct_t* handle;
};

void hello_rust(HelloStruct st) { ::xrds_core_hello_rust(st.handle); }
HelloStruct new_hello(uint64_t x, uint64_t y) { return HelloStruct{::xrds_core_new_hello(x, y)}; }
void destroy_hello(HelloStruct st) { xrds_core_destroy_hello(st.handle); }

}  // namespace xrds

#endif  // __XRDS_CORE_HPP__
