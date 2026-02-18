import SwiftUI
import WebKit

struct A2uiWebView: UIViewRepresentable {
    let url: URL
    let lastMessage: String?
    let onEvent: (String, String, String?) -> Void
    
    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }
    
    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        let controller = WKUserContentController()
        
        // Add JS Bridge handler
        controller.add(context.coordinator, name: "a2uiBridge")
        config.userContentController = controller
        
        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        
        // Handle local file access
        if url.isFileURL {
            webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        } else {
            webView.load(URLRequest(url: url))
        }
        
        return webView
    }
    
    func updateUIView(_ uiView: WKWebView, context: Context) {
        if let message = lastMessage {
            let js = "if(window.handleA2uiMessage) { window.handleA2uiMessage(\(message)); }"
            uiView.evaluateJavaScript(js, completionHandler: nil)
        }
    }
    
    class Coordinator: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
        var parent: A2uiWebView
        
        init(_ parent: A2uiWebView) {
            self.parent = parent
        }
        
        func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
            guard message.name == "a2uiBridge",
                  let body = message.body as? [String: Any],
                  let componentId = body["componentId"] as? String,
                  let eventType = body["eventType"] as? String else {
                return
            }
            
            let payload = body["payload"] as? String
            parent.onEvent(componentId, eventType, payload)
        }
    }
}
