import SwiftUI

struct ContentView: View {
    @EnvironmentObject var wsManager: WebSocketManager
    
    var body: some View {
        VStack(spacing: 0) {
            // Status Bar
            HStack {
                Circle()
                    .fill(wsManager.isConnected ? Color.green : Color.red)
                    .frame(width: 10, height: 10)
                
                Text(wsManager.isConnected ? "Connected" : "Disconnected")
                    .font(.caption)
                    .fontWeight(.bold)
                
                Spacer()
                
                if !wsManager.isConnected {
                    Button("Connect") {
                        if let url = URL(string: "ws://localhost:19527/ws/node") {
                            wsManager.connect(url: url)
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.mini)
                }
            }
            .padding()
            .background(Color(.secondarySystemBackground))
            
            // WebView
            if let bundlePath = Bundle.main.path(forResource: "index", ofType: "html", inDirectory: "a2ui") {
                A2uiWebView(url: URL(fileURLWithPath: bundlePath), lastMessage: wsManager.lastMessage) { cid, type, payload in
                   // Handle outgoing events
                   let json = "{\"type\":\"event\",\"component_id\":\"\(cid)\",\"event_type\":\"\(type)\",\"payload\":\(payload ?? "null")}"
                   wsManager.send(text: json)
                }
            } else {
                // Fallback / Development path
                let localUrl = Bundle.main.bundleURL.appendingPathComponent("a2ui/index.html")
                A2uiWebView(url: localUrl, lastMessage: wsManager.lastMessage) { cid, type, payload in
                    let json = "{\"type\":\"event\",\"component_id\":\"\(cid)\",\"event_type\":\"\(type)\",\"payload\":\(payload ?? "null")}"
                    wsManager.send(text: json)
                }
            }
        }
    }
}
