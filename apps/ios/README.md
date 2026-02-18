# Cratos iOS Node

This directory contains the Swift source code for the Cratos iOS Client.

## Setup Instructions

Since the `.xcodeproj` file involves complex binary formats, you need to generate the project shell using Xcode and import the source files.

1.  **Open Xcode** and create a new project:
    *   **Platform**: iOS
    *   **Application**: App
    *   **Product Name**: `CratosNode`
    *   **Interface**: SwiftUI
    *   **Life Cycle**: SwiftUI App
    *   **Language**: Swift
    *   **Bundle Identifier**: `com.cratos.node`
    *   **Location**: Save it in this directory (`apps/ios/`).

2.  **Add Source Files**:
    *   Replace the generated `CratosApp.swift` and `ContentView.swift` with the ones in `Sources/CratosNode/App` and `Sources/CratosNode/Views`.
    *   Drag and drop the entire `Sources/CratosNode/Core` folder into the project navigator in Xcode.
    *   Drag and drop `Sources/CratosNode/Views/A2uiWebView.swift` into the Views group.

3.  **Add Resources**:
    *   Drag and drop the `Resources/a2ui` folder into the project navigator. Ensure "Create folder references" is selected (so it's added as a blue folder).

4.  **Permissions**:
    *   Open `Info.plist` (or the project settings > Info tab) and add:
        *   `Privacy - Microphone Usage Description`: "Required for voice commands"
        *   `App Transport Security Settings` > `Allow Arbitrary Loads`: `YES` (for local development connections).

5.  **Run**:
    *   Select a simulator and run the app. It should try to connect to `ws://localhost:8080/ws/node`.

## Architecture

*   **App**: Entry point via `CratosApp`.
*   **Core**: `WebSocketManager` handles the connection to Cratos Core.
*   **Views**: MVVM pattern with SwiftUI.
*   **A2uiWebView**: Wraps `WKWebView` to render the agent UI and handle the JS bridge.
