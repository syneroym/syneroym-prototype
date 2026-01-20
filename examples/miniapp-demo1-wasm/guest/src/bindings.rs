use crate::types::*;

// Memory allocator for host-guest communication
#[no_mangle]
pub extern "C" fn alloc(size: i32) -> *mut u8 {
    let mut buf = Vec::with_capacity(size as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, size: i32) {
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, size as usize);
    }
}

// Database operations
pub struct DbQuery;

impl DbQuery {
    pub fn execute(&self, sql: &str, params: Vec<String>) -> Result<Vec<Vec<Vec<u8>>>, String> {
        let params_json = serde_json::to_vec(&params).map_err(|e| e.to_string())?;

        let result_ptr = unsafe {
            db_execute(
                sql.as_ptr() as i32,
                sql.len() as i32,
                params_json.as_ptr() as i32,
                params_json.len() as i32,
            )
        };

        if result_ptr < 0 {
            return Err("Database execute failed".to_string());
        }

        // Read result from returned pointer (simplified)
        let result_json = unsafe { read_host_memory(result_ptr) };
        serde_json::from_slice(&result_json).map_err(|e| e.to_string())
    }

    pub fn insert(&self, table: &str, data: Vec<u8>) -> Result<u64, String> {
        let id = unsafe {
            db_insert(
                table.as_ptr() as i32,
                table.len() as i32,
                data.as_ptr() as i32,
                data.len() as i32,
            )
        };

        if id < 0 {
            Err("Database insert failed".to_string())
        } else {
            Ok(id as u64)
        }
    }
}

// File operations
pub struct FileStore;

impl FileStore {
    pub fn list(&self, prefix: Option<String>) -> Result<Vec<FileInfo>, String> {
        let (prefix_ptr, prefix_len) = if let Some(p) = prefix {
            (p.as_ptr() as i32, p.len() as i32)
        } else {
            (0, 0)
        };

        let result_ptr = unsafe { file_list(prefix_ptr, prefix_len) };

        if result_ptr < 0 {
            return Err("File list failed".to_string());
        }

        let result_json = unsafe { read_host_memory(result_ptr) };
        serde_json::from_slice(&result_json).map_err(|e| e.to_string())
    }

    pub fn read_small(&self, path: &str) -> Result<Vec<u8>, String> {
        let result_ptr = unsafe { file_read_small(path.as_ptr() as i32, path.len() as i32) };

        if result_ptr < 0 {
            return Err("File read failed".to_string());
        }

        Ok(unsafe { read_host_memory(result_ptr) })
    }

    pub fn write_small(&self, path: &str, data: Vec<u8>) -> Result<(), String> {
        let result = unsafe {
            file_write_small(
                path.as_ptr() as i32,
                path.len() as i32,
                data.as_ptr() as i32,
                data.len() as i32,
            )
        };

        if result < 0 {
            Err("File write failed".to_string())
        } else {
            Ok(())
        }
    }
}

// Stream operations
pub struct InputStream;

impl InputStream {
    pub fn read(&self, stream_id: &str, max_bytes: u32) -> Result<StreamChunk, String> {
        let result_ptr = unsafe {
            stream_read(
                stream_id.as_ptr() as i32,
                stream_id.len() as i32,
                max_bytes as i32,
            )
        };

        if result_ptr < 0 {
            return Err("Stream read failed".to_string());
        }

        let chunk_json = unsafe { read_host_memory(result_ptr) };
        serde_json::from_slice(&chunk_json).map_err(|e| e.to_string())
    }
}

pub struct OutputStream;

impl OutputStream {
    pub fn write(&self, stream_id: &str, data: Vec<u8>) -> Result<usize, String> {
        let written = unsafe {
            stream_write(
                stream_id.as_ptr() as i32,
                stream_id.len() as i32,
                data.as_ptr() as i32,
                data.len() as i32,
            )
        };

        if written < 0 {
            Err("Stream write failed".to_string())
        } else {
            Ok(written as usize)
        }
    }
}

// Message stream operations
pub struct MessageStream;

impl MessageStream {
    pub fn send(&self, stream_id: &str, payload: Vec<u8>) -> Result<(), String> {
        let result = unsafe {
            message_stream_send(
                stream_id.as_ptr() as i32,
                stream_id.len() as i32,
                payload.as_ptr() as i32,
                payload.len() as i32,
            )
        };

        if result < 0 {
            Err("Message send failed".to_string())
        } else {
            Ok(())
        }
    }

    pub fn broadcast(&self, stream_type: &str, payload: Vec<u8>) -> Result<usize, String> {
        let count = unsafe {
            message_stream_broadcast(
                stream_type.as_ptr() as i32,
                stream_type.len() as i32,
                payload.as_ptr() as i32,
                payload.len() as i32,
            )
        };

        if count < 0 {
            Err("Broadcast failed".to_string())
        } else {
            Ok(count as usize)
        }
    }
}

// Host function imports
extern "C" {
    fn db_execute(sql_ptr: i32, sql_len: i32, params_ptr: i32, params_len: i32) -> i32;
    fn db_insert(table_ptr: i32, table_len: i32, data_ptr: i32, data_len: i32) -> i64;

    fn file_list(prefix_ptr: i32, prefix_len: i32) -> i32;
    fn file_read_small(path_ptr: i32, path_len: i32) -> i32;
    fn file_write_small(path_ptr: i32, path_len: i32, data_ptr: i32, data_len: i32) -> i32;

    fn stream_read(stream_id_ptr: i32, stream_id_len: i32, max_bytes: i32) -> i32;
    fn stream_write(stream_id_ptr: i32, stream_id_len: i32, data_ptr: i32, data_len: i32) -> i32;

    fn message_stream_send(
        stream_id_ptr: i32,
        stream_id_len: i32,
        payload_ptr: i32,
        payload_len: i32,
    ) -> i32;
    fn message_stream_broadcast(
        stream_type_ptr: i32,
        stream_type_len: i32,
        payload_ptr: i32,
        payload_len: i32,
    ) -> i32;
}

// Helper to read memory returned by host (simplified - needs proper implementation)
unsafe fn read_host_memory(_ptr: i32) -> Vec<u8> {
    // In production, this would read from the pointer and length returned by host
    // For now, return empty vec (this needs to be implemented based on your memory model)
    vec![]
}
