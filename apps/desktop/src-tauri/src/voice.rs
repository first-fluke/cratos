use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{StreamConfig, StreamError}; // Removed unused Stream
use ringbuf::HeapRb;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Runtime};
use tokio::sync::RwLock;
use std::sync::mpsc::{channel, Sender, Receiver};
use crate::websocket::WebSocketManager;

pub struct VoiceController<R: Runtime> {
    ws_manager: Arc<WebSocketManager<R>>,
    stop_sender: Arc<Mutex<Option<Sender<()>>>>,
    is_recording: Arc<RwLock<bool>>,
}

impl<R: Runtime> VoiceController<R> {
    pub fn new(_app_handle: AppHandle<R>, ws_manager: Arc<WebSocketManager<R>>) -> Self {
        Self {
            ws_manager,
            stop_sender: Arc::new(Mutex::new(None)),
            is_recording: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn start_capture(&self) -> Result<(), String> {
        {
            let mut recording = self.is_recording.write().await;
            if *recording {
                return Ok(()); // Already recording
            }
            *recording = true;
        }

        let ws_manager_task = self.ws_manager.clone();
        let is_recording_flag = self.is_recording.clone();
        
        let (tx, rx): (Sender<()>, Receiver<()>) = channel();
        
        {
            let mut sender_guard = self.stop_sender.lock().unwrap();
            *sender_guard = Some(tx);
        }

        let ring = HeapRb::<f32>::new(8192);
        let (mut producer, mut consumer) = ring.split();

        // Task to consume audio and send to WS
        let is_recording_flag_consume = is_recording_flag.clone();
        tokio::spawn(async move {
             let mut _chunks_sent = 0; // Fixed unused variable
             while *is_recording_flag_consume.read().await {
                if consumer.is_empty() {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    continue;
                }
                
                let chunk_size = 1024; 
                let mut temp_buf = vec![0.0; chunk_size];
                let count = consumer.pop_slice(&mut temp_buf);
                
                if count > 0 {
                    let pcm_bytes: Vec<u8> = temp_buf[..count].iter().flat_map(|&s| {
                         let clamped = s.clamp(-1.0, 1.0);
                         let sample_i16 = (clamped * 32767.0) as i16;
                         sample_i16.to_le_bytes().to_vec()
                    }).collect();
                    let _ = ws_manager_task.send_binary(pcm_bytes).await;
                    _chunks_sent += 1;
                }
             }
        });

        // Dedicated thread for Audio Stream (CPAL)
        thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(d) => d,
                None => {
                    eprintln!("No input device");
                    return;
                }
            };
            
            let mut config: StreamConfig = match device.default_input_config() {
                Ok(c) => c.into(),
                Err(e) => {
                    eprintln!("Config error: {}", e);
                    return;
                }
            };
            
            // Try 16kHz mono
            config.channels = 1;
            config.sample_rate = cpal::SampleRate(16000);

            let err_fn = move |err: StreamError| {
                eprintln!("Stream error: {}", err);
            };

            let stream = match device.build_input_stream(
                &config,
                move |data: &[f32], _: &_| {
                    let _ = producer.push_slice(data);
                },
                err_fn,
                None 
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Build stream error: {}", e);
                    return;
                }
            };
            
            if let Err(e) = stream.play() {
                eprintln!("Play error: {}", e);
                return;
            }
            
            // Block until stop signal
            let _ = rx.recv();
        });

        Ok(())
    }

    pub async fn stop_capture(&self) -> Result<(), String> {
        let mut recording = self.is_recording.write().await;
        *recording = false;
        
        let mut sender_guard = self.stop_sender.lock().unwrap();
        if let Some(tx) = sender_guard.take() {
            let _ = tx.send(()); 
        }
        
        Ok(())
    }
}
