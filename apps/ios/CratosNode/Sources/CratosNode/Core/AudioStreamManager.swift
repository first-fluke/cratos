import Foundation
import AVFoundation
import Combine

class AudioStreamManager: ObservableObject {
    private let engine = AVAudioEngine()
    private let playerNode = AVAudioPlayerNode()
    private let format: AVAudioFormat
    
    private var cancellables = Set<AnyCancellable>()
    
    init(webSocketManager: WebSocketManager) {
        // Configure for 24kHz Mono 16-bit PCM (Adjust based on server config)
        // Note: iOS AVAudioFormat commonFormat is .pcmFormatFloat32 usually for mixing
        // but we can set input format.
        
        let sampleRate: Double = 24000.0
        let channelCount: AVAudioChannelCount = 1
        
        // Target format for the engine mixing (Float32)
        guard let engineFormat = AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: channelCount) else {
            fatalError("Could not create audio format")
        }
        self.format = engineFormat
        
        setupEngine()
        
        // Subscribe to WS messages
        webSocketManager.$lastMessage
            .compactMap { $0 }
            .receive(on: DispatchQueue.global(qos: .userInitiated))
            .sink { [weak self] text in
                self?.handleMessage(text)
            }
            .store(in: &cancellables)
    }
    
    private func setupEngine() {
        engine.attach(playerNode)
        engine.connect(playerNode, to: engine.mainMixerNode, format: format)
        
        do {
            try engine.start()
            playerNode.play()
        } catch {
            print("AudioEngine start error: \(error)")
        }
    }
    
    private func handleMessage(_ text: String) {
        guard let data = text.data(using: .utf8),
              let message = try? JSONDecoder().decode(A2uiServerMessage.self, from: data),
              message.type == "audio",
              let base64Payload = message.payload,
              let pcmData = Data(base64Encoded: base64Payload)
        else { return }
        
        playChunk(pcmData: pcmData)
    }
    
    private func playChunk(pcmData: Data) {
        // Convert 16-bit int PCM to Float32 buffer for AVAudioEngine
        // This is necessary because standardFormatWithSampleRate uses Float32
        
        let frameCount = AVAudioFrameCount(pcmData.count / 2) // 2 bytes per sample (16-bit)
        guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frameCount) else { return }
        buffer.frameLength = frameCount
        
        let channels = buffer.floatChannelData
        let channelData = channels?[0] // Mono
        
        pcmData.withUnsafeBytes { body in
            let int16Ptr = body.bindMemory(to: Int16.self).baseAddress!
            for i in 0..<Int(frameCount) {
                // Normalize Int16 to Float32 (-1.0 to 1.0)
                channelData?[i] = Float(int16Ptr[i]) / Float(Int16.max)
            }
        }
        
        playerNode.scheduleBuffer(buffer, at: nil, options: [], completionHandler: nil)
        
        if !engine.isRunning {
             try? engine.start()
        }
        if !playerNode.isPlaying {
            playerNode.play()
        }
    }
}
