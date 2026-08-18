# XRDS Components

A collection of descriptors for basic components

#### Etc

In Bevy,

* **Entity** = the actual thing Bevy manages internally
* **Component** = data attached to an entity
* **Bundle** = convenient way to group components when spawning

Handle Concept

* entity identity → `Entity`
* asset identity → [Handle `<T>`](vscode-file://vscode-app/c:/Program%20Files/Microsoft%20VS%20Code/cfbea10c5f/resources/app/out/vs/code/electron-browser/workbench/workbench.html)

Project Concept

* Bevy: implementation engine
* [Runtime](vscode-file://vscode-app/c:/Program%20Files/Microsoft%20VS%20Code/cfbea10c5f/resources/app/out/vs/code/electron-browser/workbench/workbench.html): engine execution layer
* [XrdsAPI](vscode-file://vscode-app/c:/Program%20Files/Microsoft%20VS%20Code/cfbea10c5f/resources/app/out/vs/code/electron-browser/workbench/workbench.html): public 3D/runtime abstraction layer
* editor app: product built on top of [XrdsAPI](vscode-file://vscode-app/c:/Program%20Files/Microsoft%20VS%20Code/cfbea10c5f/resources/app/out/vs/code/electron-browser/workbench/workbench.html)
* editor users / plugin developers: mostly interact with XRDS concepts, not Bevy concepts

Maintainer note: when adding a new primitive descriptor, use [docs/adding-primitive-type.md](../../docs/adding-primitive-type.md) as the source of truth for which crate and file should own each part of the implementation.
