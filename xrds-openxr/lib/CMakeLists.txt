cmake_minimum_required(VERSION 3.21)

project(openxr-wrapper CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_library(${PROJECT_NAME} SHARED)

find_package(Vulkan)

if(NOT VULKAN_FOUND)
    message(FATAL_ERROR "Vulkan SDK not found")
endif()

include(FetchContent)

FetchContent_Declare(openxr-sdk
    GIT_REPOSITORY https://github.com/KhronosGroup/OpenXR-SDK.git
    GIT_TAG release-1.1.43
)

FetchContent_MakeAvailable(openxr-sdk)
target_sources(${PROJECT_NAME}
    PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src/openxr-wrapper.cpp
)

target_include_directories(${PROJECT_NAME}
    PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(${PROJECT_NAME}
    PRIVATE
    OpenXR::openxr_loader
    Vulkan::Headers
)

install(TARGETS ${PROJECT_NAME} DESTINATION .)

add_custom_command(TARGET ${PROJECT_NAME} POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different $<TARGET_FILE:${PROJECT_NAME}> ${CARGO_TARGET_DIR}/$<TARGET_FILE_NAME:${PROJECT_NAME}>
)

# install(TARGETS OPENXR DESTINATION .)