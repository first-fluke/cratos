package com.cratos.node

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.cratos.node.core.A2uiClientMessage
import com.cratos.node.core.ConnectionState
import com.cratos.node.core.WebSocketManager
import com.cratos.node.core.BundleManager
import com.cratos.node.core.AudioStreamManager
import com.cratos.node.ui.A2uiWebView
import dagger.hilt.android.AndroidEntryPoint
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import javax.inject.Inject
import androidx.compose.ui.Alignment
import kotlinx.coroutines.flow.asStateFlow

@HiltViewModel
class MainViewModel @Inject constructor(
    private val webSocketManager: WebSocketManager,
    private val bundleManager: BundleManager,
    private val audioStreamManager: AudioStreamManager // Injected to ensure initialization
) : ViewModel() {
    
    private val _bundlePath = MutableStateFlow<String?>(null)
    val bundlePath: StateFlow<String?> = _bundlePath
    
    val connectionState = webSocketManager.connectionState
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), ConnectionState.Disconnected)

    fun connect() {
        viewModelScope.launch {
            // Check for updates first
            val hasUpdate = bundleManager.checkForUpdates()
            
            // Get local path (either new or existing)
            val path = bundleManager.getLocalBundleEntry()
            _bundlePath.value = path ?: "file:///android_asset/a2ui/index.html" // Fallback

            // Then connect WS
            webSocketManager.connect("ws://10.0.2.2:19527/ws/node") 
        }
    }

    fun handleEvent(componentId: String, eventType: String, payload: String?) {
        // Convert payload string back to JsonElement if needed, or pass as raw string if protocol allows
        // For MVP, simplistic handling
        val jsonPayload = payload?.let { try { Json.parseToJsonElement(it) } catch(e: Exception) { null } }
        
        if (webSocketManager.connectionState.value is ConnectionState.Connected) {
            webSocketManager.send(A2uiClientMessage(
                type = "event",
                component_id = componentId,
                event_type = eventType,
                payload = jsonPayload
            ))
        }
    }
    
    override fun onCleared() {
        super.onCleared()
        audioStreamManager.release()
    }
}

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    private val viewModel: MainViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme {
                Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
                    MainScreen(viewModel)
                }
            }
        }
        
        // Auto-connect on start
        viewModel.connect()
    }
}

@Composable
fun MainScreen(viewModel: MainViewModel) {
    val connectionState by viewModel.connectionState.collectAsState()
    val lastMessage by viewModel.messages.collectAsState()
    val bundlePath by viewModel.bundlePath.collectAsState()
    
    Column(modifier = Modifier.fillMaxSize()) {
        Card(
            modifier = Modifier.fillMaxWidth().padding(16.dp),
            colors = CardDefaults.cardColors(
                containerColor = when(connectionState) {
                    is ConnectionState.Connected -> MaterialTheme.colorScheme.primaryContainer
                    is ConnectionState.Error -> MaterialTheme.colorScheme.errorContainer
                    else -> MaterialTheme.colorScheme.secondaryContainer
                }
            )
        ) {
            Column(modifier = Modifier.padding(16.dp)) {
                Text(
                    text = "Status: ${connectionState::class.simpleName}",
                    style = MaterialTheme.typography.titleMedium
                )
                if (connectionState is ConnectionState.Error) {
                    Text(
                        text = (connectionState as ConnectionState.Error).message,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error
                    )
                }
            }
        }
        
        if (bundlePath != null) {
            A2uiWebView(
                url = bundlePath!!,
                lastMessage = lastMessage,
                onEvent = { cid, type, payload ->
                    viewModel.handleEvent(cid, type, payload)
                }
            )
        } else {
             Box(modifier = Modifier.fillMaxSize(), contentAlignment = androidx.compose.ui.Alignment.Center) {
                 CircularProgressIndicator()
                 Text("Checking for updates...", modifier = Modifier.padding(top = 48.dp))
             }
        }
    }
}
