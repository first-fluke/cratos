package com.cratos.node.ui

import android.webkit.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.viewinterop.AndroidView
import com.cratos.node.core.WebSocketManager
import com.cratos.node.core.A2uiServerMessage
import android.annotation.SuppressLint
import kotlinx.serialization.json.Json

class A2uiBridgeInterface(
    private val onEvent: (String, String, String?) -> Unit
) {
    @JavascriptInterface
    fun postEvent(componentId: String, eventType: String, payload: String?) {
        onEvent(componentId, eventType, payload)
    }
}

@SuppressLint("SetJavaScriptEnabled")
@Composable
fun A2uiWebView(
    url: String,
    lastMessage: A2uiServerMessage?,
    onEvent: (String, String, String?) -> Unit
) {
    AndroidView(
        factory = { context ->
            WebView(context).apply {
                settings.apply {
                    javaScriptEnabled = true
                    domStorageEnabled = true
                    allowFileAccess = true // Needs to be true for file:///android_asset
                    allowContentAccess = true
                }
                
                // JavaScript Bridge
                addJavascriptInterface(A2uiBridgeInterface(onEvent), "a2uiBridge")
                
                // Load from dynamic path
                loadUrl(url)
            }
        },
        update = { webView ->
            lastMessage?.let { msg ->
                val json = Json.encodeToString(A2uiServerMessage.serializer(), msg)
                val js = "if(window.handleA2uiMessage) { window.handleA2uiMessage($json); }"
                webView.evaluateSwift(js)
            }
        }
    )
}

private fun WebView.evaluateSwift(js: String) {
    this.evaluateJavascript(js, null)
}
