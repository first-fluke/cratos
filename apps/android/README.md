# Cratos Android Node

This is the Android client for Cratos Node.

## Setup Instructions

1.  **Open in Android Studio**:
    *   Select "Open" and navigate to `apps/android/cratos-node`.
    *   Wait for Gradle sync to complete.

2.  **Configuration**:
    *   The project uses Gradle Version Catalogs (`gradle/libs.versions.toml`).
    *   Hilt is used for Dependency Injection.
    *   Jetpack Compose is used for UI.

3.  **Run**:
    *   Ensure your emulator or device is connected.
    *   Run the `app` configuration.
    *   The app will attempt to connect to `ws://10.0.2.2:8080/ws/node` (Android emulator's localhost loopback).

## Troubleshooting

*   **Connection Refused**: Ensure the Cratos Server is running on port 8080.
*   **Cleartext Traffic**: If connecting to a local IP other than localhost, check `network_security_config.xml` or ensure `usesCleartextTraffic="true"` is set in Manifest.

## Architecture

*   **MVVM**: `MainViewModel` manages state and `WebSocketManager`.
*   **Core**: `WebSocketManager` handles OkHttp WebSocket connections.
*   **UI**: `MainActivity` hosts the `MainScreen` composable and `A2uiWebView`.
*   **Bridge**: `A2uiBridgeInterface` handles communication between WebView JS and Kotlin.
