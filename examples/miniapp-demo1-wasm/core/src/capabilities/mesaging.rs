use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use tokio::sync::mpsc;

pub struct MessageStreamManager {
    // stream_id -> sender channel
    streams: Mutex<HashMap<String, mpsc::Sender<Vec<u8>>>>,
    // stream_type -> set of stream_ids
    stream_types: Mutex<HashMap<String, HashSet<String>>>,
}

impl MessageStreamManager {
    pub fn new() -> Self {
        MessageStreamManager {
            streams: Mutex::new(HashMap::new()),
            stream_types: Mutex::new(HashMap::new()),
        }
    }

    pub fn register_stream(
        &self,
        stream_id: String,
        stream_type: String,
        sender: mpsc::Sender<Vec<u8>>,
    ) {
        let mut streams = self.streams.lock().unwrap();
        streams.insert(stream_id.clone(), sender);

        let mut types = self.stream_types.lock().unwrap();
        types
            .entry(stream_type)
            .or_insert_with(HashSet::new)
            .insert(stream_id);
    }

    pub fn send(&self, stream_id: &str, payload: Vec<u8>) -> Result<()> {
        let streams = self.streams.lock().unwrap();
        if let Some(sender) = streams.get(stream_id) {
            sender
                .try_send(payload)
                .map_err(|e| anyhow!("Send failed: {}", e))?;
            Ok(())
        } else {
            Err(anyhow!("Stream not found"))
        }
    }

    pub fn broadcast(&self, stream_type: &str, payload: Vec<u8>) -> Result<usize> {
        let types = self.stream_types.lock().unwrap();
        if let Some(stream_ids) = types.get(stream_type) {
            let streams = self.streams.lock().unwrap();
            let mut count = 0;
            for stream_id in stream_ids {
                if let Some(sender) = streams.get(stream_id) {
                    if sender.try_send(payload.clone()).is_ok() {
                        count += 1;
                    }
                }
            }
            Ok(count)
        } else {
            Ok(0)
        }
    }

    pub fn unregister_stream(&self, stream_type: &str, stream_id: &str) {
        let mut streams = self.streams.lock().unwrap();
        streams.remove(stream_id);

        let mut types = self.stream_types.lock().unwrap();
        if let Some(stream_set) = types.get_mut(stream_type) {
            stream_set.remove(stream_id);
        }
    }

    pub fn stream_count(&self, stream_type: &str) -> usize {
        let types = self.stream_types.lock().unwrap();
        types.get(stream_type).map(|s| s.len()).unwrap_or(0)
    }
}
