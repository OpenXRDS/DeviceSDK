#include "openxr-wrapper.h"

#include <cstdlib>
#include <iostream>

#include "vulkan/vulkan.h"

#include "openxr/openxr.h"

#define XR_USE_GRAPHICS_API_VULKAN
#include "openxr/openxr_platform.h"

void initialize_openxr() {
    char errMsg[64];

    XrInstanceCreateInfo instanceInfo = {
        XR_TYPE_INSTANCE_CREATE_INFO,                                                                // type
        nullptr,                                                                                     // next
        0,                                                                                           // flags
        XrApplicationInfo{"xrds-device-sdk", 0, "xrds-device-engine", 0, XR_MAKE_VERSION(1, 0, 0)},  // application_info
        0,                                                                                           // layer_count
        nullptr,                                                                                     // layers
        0,                                                                                           // extension_count
        nullptr                                                                                      // extensions
    };

    XrInstance instance = nullptr;
    if (auto res = xrCreateInstance(&instanceInfo, &instance); XR_FAILED(res)) {
        std::cout << "Could not initialize xr instance" << std::endl;
        return;
    }
    std::cout << "Instance created successfully" << std::endl;
    XrSystemGetInfo systemGetInfo = {XR_TYPE_SYSTEM_GET_INFO, nullptr,
                                     XrFormFactor::XR_FORM_FACTOR_HEAD_MOUNTED_DISPLAY};
    XrSystemId systemId;
    xrGetSystem(instance, &systemGetInfo, &systemId);

    XrSessionCreateInfo sessionInfo = {XR_TYPE_SESSION_CREATE_INFO, nullptr,
                                       0,  // session_create_flags
                                       systemId};

    XrGraphicsBindingVulkanKHR vulkanBinding = {XR_TYPE_GRAPHICS_BINDING_VULKAN_KHR};

    XrSession session = nullptr;
    if (auto res = xrCreateSession(instance, &sessionInfo, &session); XR_FAILED(res)) {
        xrResultToString(instance, res, errMsg);
        std::cout << errMsg << std::endl;
        return;
    }
    std::cout << "Session created successfully" << std::endl;

    if (auto res = xrDestroySession(session); XR_FAILED(res)) {
        xrResultToString(instance, res, errMsg);
        std::cout << errMsg << std::endl;
        return;
    }
    std::cout << "Session destroyed successfully" << std::endl;

    if (auto res = xrDestroyInstance(instance); XR_FAILED(res)) {
        std::cout << "Could not destroy xr instance" << std::endl;
        return;
    }
}