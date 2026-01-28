use crate::types::{StreamChunk, StreamInfo};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

type BoxedAsyncRead = Box<dyn AsyncRead + Unpin + Send>;
type BoxedAsyncWrite = Box<dyn AsyncWrite + Unpin + Send>;

pub struct StreamManager {
    input_streams: Mutex<HashMap<String, BoxedAsyncRead>>,
    output_streams: Mutex<HashMap<String, BoxedAsyncWrite>>,
    stream_metadata: Mutex<HashMap<String, StreamInfo>>,
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamManager {
    pub fn new() -> Self {
        StreamManager {
            input_streams: Mutex::new(HashMap::new()),
            output_streams: Mutex::new(HashMap::new()),
            stream_metadata: Mutex::new(HashMap::new()),
        }
    }

    pub fn register_input_stream(
        &self,
        stream: impl AsyncRead + Unpin + Send + 'static,
        info: StreamInfo,
    ) -> String {
        let stream_id = uuid::Uuid::new_v4().to_string();

        let mut streams = self.input_streams.lock().unwrap();
        streams.insert(stream_id.clone(), Box::new(stream));

        let mut metadata = self.stream_metadata.lock().unwrap();
        let mut info = info;
        info.id = stream_id.clone();
        metadata.insert(stream_id.clone(), info);

        stream_id
    }

    pub fn register_output_stream(
        &self,
        stream: impl AsyncWrite + Unpin + Send + 'static,
        info: StreamInfo,
    ) -> String {
        let stream_id = uuid::Uuid::new_v4().to_string();

        let mut streams = self.output_streams.lock().unwrap();
        streams.insert(stream_id.clone(), Box::new(stream));

        let mut metadata = self.stream_metadata.lock().unwrap();
        let mut info = info;
        info.id = stream_id.clone();
        metadata.insert(stream_id.clone(), info);

        stream_id
    }

    pub async fn read_chunk(&self, stream_id: &str, max_bytes: usize) -> Result<StreamChunk> {
        // Remove stream temporarily for async operation
        let mut stream = {
            let mut streams = self.input_streams.lock().unwrap();
            streams
                .remove(stream_id)
                .ok_or_else(|| anyhow!("Stream not found"))?
        };

        let mut buffer = vec![0u8; max_bytes];
        let result = match stream.read(&mut buffer).await {
            Ok(0) => Ok(StreamChunk {
                data: vec![],
                eof: true,
            }),
            Ok(n) => {
                buffer.truncate(n);
                Ok(StreamChunk {
                    data: buffer,
                    eof: false,
                })
            }
            Err(e) => Err(anyhow!("Read error: {}", e)),
        };

        // Put stream back if not EOF
        if let Ok(chunk) = &result {
            if !chunk.eof {
                let mut streams = self.input_streams.lock().unwrap();
                streams.insert(stream_id.to_string(), stream);
            }
        }

        result
    }

    pub async fn write_chunk(&self, stream_id: &str, data: Vec<u8>) -> Result<usize> {
        let mut stream = {
            let mut streams = self.output_streams.lock().unwrap();
            streams
                .remove(stream_id)
                .ok_or_else(|| anyhow!("Stream not found"))?
        };

        let len = data.len();
        let result = stream.write_all(&data).await.map(|_| len);

        // Put stream back
        let mut streams = self.output_streams.lock().unwrap();
        streams.insert(stream_id.to_string(), stream);

        result.map_err(|e| anyhow!("Write error: {}", e))
    }

    pub fn close_input_stream(&self, stream_id: &str) {
        let mut streams = self.input_streams.lock().unwrap();
        streams.remove(stream_id);

        let mut metadata = self.stream_metadata.lock().unwrap();
        metadata.remove(stream_id);
    }

    pub async fn finish_output_stream(&self, stream_id: &str) -> Result<()> {
        let mut stream = {
            let mut streams = self.output_streams.lock().unwrap();
            streams
                .remove(stream_id)
                .ok_or_else(|| anyhow!("Stream not found"))?
        };

        stream.flush().await?;

        let mut metadata = self.stream_metadata.lock().unwrap();
        metadata.remove(stream_id);

        Ok(())
    }

    pub fn get_info(&self, stream_id: &str) -> Option<StreamInfo> {
        let metadata = self.stream_metadata.lock().unwrap();
        metadata.get(stream_id).cloned()
    }
}
