cmake_minimum_required(VERSION 3.20)

project(openxr CXX)

add_library(${PROJECT_NAME} STATIC ${CMAKE_CURRENT_SOURCE_DIR}/src/openxr.cpp)

install(TARGETS ${PROJECT_NAME} DESTINATION .)