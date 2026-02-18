package com.cratos.node.core

import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import okhttp3.*
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import java.util.concurrent.TimeUnit
import javax.inject.Inject
import javax.inject.Singleton

@Serializable
data class A2uiServerMessage(
    val type: String,
    val component_id: String? = null,
    val props: JsonElement? = null,
    val payload: String? = null // Base64 encoded audio
)

@Serializable
data class A2uiClientMessage(
    val type: String,
    val component_id: String? = null,
    val event_type: String? = null,
    val payload: JsonElement? = null
)

sealed class ConnectionState {
    object Disconnected : ConnectionState()
    object Connecting : ConnectionState()
    object Connected : ConnectionState()
    data class Error(val message: String) : ConnectionState()
}

@Singleton
class WebSocketManager @Inject constructor() {
    private val client = OkHttpClient.Builder()
        .pingInterval(30, TimeUnit.SECONDS)
        .build()

    private var webSocket: WebSocket? = null

    private val _connectionState = MutableStateFlow<ConnectionState>(ConnectionState.Disconnected)
    val connectionState: StateFlow<ConnectionState> = _connectionState

    private val _messages = Channel<A2uiServerMessage>(Channel.BUFFERED)
    val messages = _messages.receiveAsFlow()

    fun connect(url: String) {
        _connectionState.value = ConnectionState.Connecting
        
        val request = Request.Builder()
            .url(url)
            .build()

        webSocket = client.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                _connectionState.value = ConnectionState.Connected
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                try {
                    val message = Json.decodeFromString<A2uiServerMessage>(text)
                    _messages.trySend(message)
                } catch (e: Exception) {
                    println("Failed to parse message: $text")
                }
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                _connectionState.value = ConnectionState.Error(t.message ?: "Unknown error")
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                _connectionState.value = ConnectionState.Disconnected
            }
        })
    }
    
    fun send(message: A2uiClientMessage) {
        val json = Json.encodeToString(A2uiClientMessage.serializer(), message)
        webSocket?.send(json)
    }

    fun disconnect() {
        webSocket?.close(1000, "User disconnect")
        _connectionState.value = ConnectionState.Disconnected
    }
}
