import SwiftUI

@main
struct CratosApp: App {
    @StateObject private var webSocketManager: WebSocketManager
    @StateObject private var bundleManager = BundleManager()
    @StateObject private var audioStreamManager: AudioStreamManager

    init() {
        let wsManager = WebSocketManager()
        _webSocketManager = StateObject(wrappedValue: wsManager)
        _audioStreamManager = StateObject(wrappedValue: AudioStreamManager(webSocketManager: wsManager))
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(webSocketManager)
                .environmentObject(bundleManager)
        }
    }
}
