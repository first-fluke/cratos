package com.cratos.node.core

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.util.Base64
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AudioStreamManager @Inject constructor(
    private val webSocketManager: WebSocketManager
) {
    private var audioTrack: AudioTrack? = null
    private val scope = CoroutineScope(Dispatchers.IO + Job())

    // Standard ElevenLabs / PCM format (usually 44.1kHz or 24kHz, 16bit mono)
    // Assuming 24kHz for example, need to match server config
    private val SAMPLE_RATE = 24000 
    
    init {
        initializeAudioTrack()
        startListening()
    }

    private fun initializeAudioTrack() {
        val bufferSize = AudioTrack.getMinBufferSize(
            SAMPLE_RATE,
            AudioFormat.CHANNEL_OUT_MONO,
            AudioFormat.ENCODING_PCM_16BIT
        ) * 2

        audioTrack = AudioTrack.Builder()
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_MEDIA)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                    .build()
            )
            .setAudioFormat(
                AudioFormat.Builder()
                    .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                    .setSampleRate(SAMPLE_RATE)
                    .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                    .build()
            )
            .setBufferSizeInBytes(bufferSize)
            .setTransferMode(AudioTrack.MODE_STREAM)
            .build()
            
        audioTrack?.play()
    }

    private fun startListening() {
        webSocketManager.messages.onEach { message ->
            if (message.type == "audio" && message.payload != null) {
                playChunk(message.payload)
            }
        }.launchIn(scope)
    }

    private fun playChunk(base64Data: String) {
        try {
            val audioData = Base64.decode(base64Data, Base64.DEFAULT)
            audioTrack?.write(audioData, 0, audioData.size)
        } catch (e: Exception) {
            e.printStackTrace()
        }
    }

    fun release() {
        audioTrack?.stop()
        audioTrack?.release()
    }
}
