project(xrds)

add_executable(${PROJECT_NAME})

target_sources(${PROJECT_NAME}
    PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src/main.cpp
)

target_include_directories(${PROJECT_NAME}
    PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/include
    PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(${PROJECT_NAME}
    PUBLIC
    xrds::audio
    xrds::components
    xrds::core
    xrds::graphics
    xrds::net
    xrds::runtime
)

install(
    DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/include"
    DESTINATION "."
    FILES_MATCHING PATTERN "*.h" PATTERN "*.hpp"
    PERMISSIONS OWNER_READ OWNER_WRITE GROUP_READ GROUP_WRITE WORLD_READ
)

# add_custom_target(GenerateHeaders ALL DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/include/xrds/__xrds.h)

# add_custom_command(OUTPUT
# ${CMAKE_CURRENT_SOURCE_DIR}/include/xrds/__xrds.h
# ${CMAKE_CURRENT_SOURCE_DIR}/include/xrds/xrds.h

# # COMMAND cargo run --features core_headers --bin gen-headers
# COMMAND cargo run --features runtime_headers --bin gen-headers
# COMMAND cargo run --features headers --bin gen-headers
# COMMAND_EXPAND_LISTS
# DEPENDS ${DRM_ARG_SOURCES}
# WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
# VERBATIM
# )