project(xrds-core)
file(GLOB_RECURSE XRDS_CORE_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs")

add_library(${PROJECT_NAME} INTERFACE ${XRDS_CORE_SOURCES})
add_library(xrds::core ALIAS ${PROJECT_NAME})

if(WIN32)
    set(${PROJECT_NAME}_DYLIB_NAME "xrds_core.dll")
    set(${PROJECT_NAME}_LIB_NAME "xrds_core.dll.lib")
    set(${PROJECT_NAME}_PDB_NAME "xrds_core.pdb")
elseif(IOS OR APPLE)
    set(${PROJECT_NAME}_DYLIB_NAME "xrds_core.dylib")
    set(${PROJECT_NAME}_LIB_NAME "libxrds_core.lib")
elseif(LINUX)
    set(${PROJECT_NAME}_DYLIB_NAME "libxrds_core.so")
endif()

target_sources(${PROJECT_NAME}
    PRIVATE
    $<IF:$<CONFIG:Debug>,${CMAKE_RUNTIME_OUTPUT_DIRECTORY_DEBUG},${CMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE}>/${${PROJECT_NAME}_DYLIB_NAME}
)

target_include_directories(${PROJECT_NAME}
    INTERFACE
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
)

target_link_libraries(${PROJECT_NAME}
    INTERFACE
    ${CMAKE_BINARY_DIR}/rust-target/$<IF:$<CONFIG:Debug>,debug,release>/${${PROJECT_NAME}_LIB_NAME}
)

add_custom_command(OUTPUT $<IF:$<CONFIG:Debug>,${CMAKE_RUNTIME_OUTPUT_DIRECTORY_DEBUG},${CMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE}>/${${PROJECT_NAME}_DYLIB_NAME}
    COMMAND cargo build $<$<NOT:$<CONFIG:Debug>>:--release>
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
    ${CMAKE_BINARY_DIR}/rust-target/$<IF:$<CONFIG:Debug>,debug,release>/${${PROJECT_NAME}_DYLIB_NAME}
    $<IF:$<CONFIG:Debug>,${CMAKE_RUNTIME_OUTPUT_DIRECTORY_DEBUG},${CMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE}>/${${PROJECT_NAME}_DYLIB_NAME}
    COMMAND_EXPAND_LISTS
    DEPENDS ${XRDS_CORE_SOURCES}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    VERBATIM
)
