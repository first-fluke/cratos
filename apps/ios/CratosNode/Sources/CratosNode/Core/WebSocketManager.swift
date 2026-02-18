import Foundation
import Combine

struct A2uiServerMessage: Codable {
    let type: String
    let component_id: String?
    let props: [String: String]?
    let payload: String? // Base64 encoded audio
}

class WebSocketManager: ObservableObject {
    @Published var isConnected = false
    @Published var lastError: String?
    @Published var lastMessage: String?
    
    private var webSocket: URLSessionWebSocketTask?
    private let session: URLSession
    private var cancellables = Set<AnyCancellable>()
    
    init() {
        let config = URLSessionConfiguration.default
        config.waitsForConnectivity = true
        self.session = URLSession(configuration: config)
    }
    
    func connect(url: URL) {
        let request = URLRequest(url: url)
        webSocket = session.webSocketTask(with: request)
        webSocket?.resume()
        
        DispatchQueue.main.async {
            self.isConnected = true
        }
        receiveMessage()
    }
    
    func disconnect() {
        webSocket?.cancel(with: .normalClosure, reason: nil)
        DispatchQueue.main.async {
            self.isConnected = false
        }
    }
    
    func send(text: String) {
        let message = URLSessionWebSocketTask.Message.string(text)
        webSocket?.send(message) { error in
            if let error = error {
                print("WebSocket send error: \(error)")
            }
        }
    }
    
    private func receiveMessage() {
        webSocket?.receive { [weak self] result in
            switch result {
            case .success(let message):
                switch message {
                case .string(let text):
                    print("Received: \(text)")
                    DispatchQueue.main.async {
                        self?.lastMessage = text
                    }
                case .data(let data):
                    if let text = String(data: data, encoding: .utf8) {
                        DispatchQueue.main.async {
                            self?.lastMessage = text
                        }
                    }
                @unknown default:
                    break
                }
                self?.receiveMessage()
                
            case .failure(let error):
                print("WebSocket error: \(error)")
                DispatchQueue.main.async {
                    self?.isConnected = false
                    self?.lastError = error.localizedDescription
                }
            }
        }
    }
}
