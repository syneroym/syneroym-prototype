use crate::capabilities::*;
use crate::types::*;
use anyhow::Result;
use wasmtime::{Caller, Linker};

pub struct HostState {
    pub capabilities: HostCapabilities,
}

// Helper functions for memory operations
fn read_string_from_memory(caller: &Caller<HostState>, ptr: i32, len: i32) -> Result<String> {
    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow::anyhow!("Memory not found"))?;

    let mut buffer = vec![0u8; len as usize];
    memory.read(caller, ptr as usize, &mut buffer)?;

    Ok(String::from_utf8(buffer)?)
}

fn read_bytes_from_memory(caller: &Caller<HostState>, ptr: i32, len: i32) -> Result<Vec<u8>> {
    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow::anyhow!("Memory not found"))?;

    let mut buffer = vec![0u8; len as usize];
    memory.read(caller, ptr as usize, &mut buffer)?;

    Ok(buffer)
}

fn write_bytes_to_memory(caller: &mut Caller<HostState>, data: &[u8]) -> Result<(i32, i32)> {
    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow::anyhow!("Memory not found"))?;

    // Allocate memory in WASM (simplified - in production use proper allocator)
    let alloc_fn = caller
        .get_export("alloc")
        .and_then(|e| e.into_func())
        .ok_or_else(|| anyhow::anyhow!("Alloc function not found"))?;

    let mut results = [wasmtime::Val::I32(0)];
    alloc_fn.call(
        caller,
        &[wasmtime::Val::I32(data.len() as i32)],
        &mut results,
    )?;

    let ptr = match results[0] {
        wasmtime::Val::I32(p) => p,
        _ => return Err(anyhow::anyhow!("Invalid alloc result")),
    };

    memory.write(caller, ptr as usize, data)?;

    Ok((ptr, data.len() as i32))
}

pub fn bind_all_capabilities(linker: &mut Linker<HostState>) {
    bind_db_capabilities(linker);
    bind_file_capabilities(linker);
    bind_stream_capabilities(linker);
    bind_message_stream_capabilities(linker);
}

fn bind_db_capabilities(linker: &mut Linker<HostState>) {
    // db-execute(sql_ptr, sql_len, params_ptr, params_len) -> result_ptr
    linker
        .func_wrap(
            "capabilities",
            "db-execute",
            |mut caller: Caller<HostState>,
             sql_ptr: i32,
             sql_len: i32,
             params_ptr: i32,
             params_len: i32|
             -> i32 {
                let result = (|| -> Result<Vec<u8>> {
                    let sql = read_string_from_memory(&caller, sql_ptr, sql_len)?;
                    let params_bytes = read_bytes_from_memory(&caller, params_ptr, params_len)?;
                    let params: Vec<String> = serde_json::from_slice(&params_bytes)?;

                    let conn = caller.data().capabilities.db.lock().unwrap();
                    let rows = db::DbOperations::execute(&conn, &sql, params)?;

                    Ok(serde_json::to_vec(&rows)?)
                })();

                match result {
                    Ok(data) => match write_bytes_to_memory(&mut caller, &data) {
                        Ok((ptr, _)) => ptr,
                        Err(_) => -1,
                    },
                    Err(_) => -1,
                }
            },
        )
        .unwrap();

    // db-insert(table_ptr, table_len, data_ptr, data_len) -> id
    linker
        .func_wrap(
            "capabilities",
            "db-insert",
            |mut caller: Caller<HostState>,
             table_ptr: i32,
             table_len: i32,
             data_ptr: i32,
             data_len: i32|
             -> i64 {
                let result = (|| -> Result<u64> {
                    let table = read_string_from_memory(&caller, table_ptr, table_len)?;
                    let data = read_bytes_from_memory(&caller, data_ptr, data_len)?;

                    let conn = caller.data().capabilities.db.lock().unwrap();
                    db::DbOperations::insert(&conn, &table, data)
                })();

                match result {
                    Ok(id) => id as i64,
                    Err(_) => -1,
                }
            },
        )
        .unwrap();
}

fn bind_file_capabilities(linker: &mut Linker<HostState>) {
    // file-list(prefix_ptr, prefix_len) -> result_ptr
    linker
        .func_wrap(
            "capabilities",
            "file-list",
            |mut caller: Caller<HostState>, prefix_ptr: i32, prefix_len: i32| -> i32 {
                let result = (|| -> Result<Vec<u8>> {
                    let prefix = if prefix_len > 0 {
                        Some(read_string_from_memory(&caller, prefix_ptr, prefix_len)?)
                    } else {
                        None
                    };

                    let data_dir = caller.data().capabilities.data_dir.clone();

                    // Use blocking task for async operation
                    let files = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current()
                            .block_on(files::FileOperations::list_files(&data_dir, prefix))
                    })?;

                    Ok(serde_json::to_vec(&files)?)
                })();

                match result {
                    Ok(data) => match write_bytes_to_memory(&mut caller, &data) {
                        Ok((ptr, _)) => ptr,
                        Err(_) => -1,
                    },
                    Err(_) => -1,
                }
            },
        )
        .unwrap();

    // file-read-small(path_ptr, path_len) -> result_ptr
    linker
        .func_wrap(
            "capabilities",
            "file-read-small",
            |mut caller: Caller<HostState>, path_ptr: i32, path_len: i32| -> i32 {
                let result = (|| -> Result<Vec<u8>> {
                    let path = read_string_from_memory(&caller, path_ptr, path_len)?;
                    let data_dir = caller.data().capabilities.data_dir.clone();

                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current()
                            .block_on(files::FileOperations::read_small(&data_dir, &path))
                    })
                })();

                match result {
                    Ok(data) => match write_bytes_to_memory(&mut caller, &data) {
                        Ok((ptr, _)) => ptr,
                        Err(_) => -1,
                    },
                    Err(_) => -1,
                }
            },
        )
        .unwrap();

    // file-write-small(path_ptr, path_len, data_ptr, data_len) -> success
    linker
        .func_wrap(
            "capabilities",
            "file-write-small",
            |caller: Caller<HostState>,
             path_ptr: i32,
             path_len: i32,
             data_ptr: i32,
             data_len: i32|
             -> i32 {
                let result = (|| -> Result<()> {
                    let path = read_string_from_memory(&caller, path_ptr, path_len)?;
                    let data = read_bytes_from_memory(&caller, data_ptr, data_len)?;
                    let data_dir = caller.data().capabilities.data_dir.clone();

                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current()
                            .block_on(files::FileOperations::write_small(&data_dir, &path, data))
                    })
                })();

                if result.is_ok() {
                    0
                } else {
                    -1
                }
            },
        )
        .unwrap();
}

fn bind_stream_capabilities(linker: &mut Linker<HostState>) {
    // stream-read(stream_id_ptr, stream_id_len, max_bytes) -> chunk_ptr
    linker
        .func_wrap(
            "capabilities",
            "stream-read",
            |mut caller: Caller<HostState>,
             stream_id_ptr: i32,
             stream_id_len: i32,
             max_bytes: i32|
             -> i32 {
                let result = (|| -> Result<Vec<u8>> {
                    let stream_id = read_string_from_memory(&caller, stream_id_ptr, stream_id_len)?;
                    let stream_manager = caller.data().capabilities.stream_manager.clone();

                    let chunk = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current()
                            .block_on(stream_manager.read_chunk(&stream_id, max_bytes as usize))
                    })?;

                    Ok(serde_json::to_vec(&chunk)?)
                })();

                match result {
                    Ok(data) => match write_bytes_to_memory(&mut caller, &data) {
                        Ok((ptr, _)) => ptr,
                        Err(_) => -1,
                    },
                    Err(_) => -1,
                }
            },
        )
        .unwrap();

    // stream-write(stream_id_ptr, stream_id_len, data_ptr, data_len) -> bytes_written
    linker
        .func_wrap(
            "capabilities",
            "stream-write",
            |caller: Caller<HostState>,
             stream_id_ptr: i32,
             stream_id_len: i32,
             data_ptr: i32,
             data_len: i32|
             -> i32 {
                let result = (|| -> Result<usize> {
                    let stream_id = read_string_from_memory(&caller, stream_id_ptr, stream_id_len)?;
                    let data = read_bytes_from_memory(&caller, data_ptr, data_len)?;
                    let stream_manager = caller.data().capabilities.stream_manager.clone();

                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current()
                            .block_on(stream_manager.write_chunk(&stream_id, data))
                    })
                })();

                match result {
                    Ok(n) => n as i32,
                    Err(_) => -1,
                }
            },
        )
        .unwrap();
}

fn bind_message_stream_capabilities(linker: &mut Linker<HostState>) {
    // message-stream-send(stream_id_ptr, stream_id_len, payload_ptr, payload_len) -> success
    linker
        .func_wrap(
            "capabilities",
            "message-stream-send",
            |caller: Caller<HostState>,
             stream_id_ptr: i32,
             stream_id_len: i32,
             payload_ptr: i32,
             payload_len: i32|
             -> i32 {
                let result = (|| -> Result<()> {
                    let stream_id = read_string_from_memory(&caller, stream_id_ptr, stream_id_len)?;
                    let payload = read_bytes_from_memory(&caller, payload_ptr, payload_len)?;

                    caller
                        .data()
                        .capabilities
                        .message_streams
                        .send(&stream_id, payload)
                })();

                if result.is_ok() {
                    0
                } else {
                    -1
                }
            },
        )
        .unwrap();

    // message-stream-broadcast(stream_type_ptr, stream_type_len, payload_ptr, payload_len) -> count
    linker
        .func_wrap(
            "capabilities",
            "message-stream-broadcast",
            |caller: Caller<HostState>,
             stream_type_ptr: i32,
             stream_type_len: i32,
             payload_ptr: i32,
             payload_len: i32|
             -> i32 {
                let result = (|| -> Result<usize> {
                    let stream_type =
                        read_string_from_memory(&caller, stream_type_ptr, stream_type_len)?;
                    let payload = read_bytes_from_memory(&caller, payload_ptr, payload_len)?;

                    caller
                        .data()
                        .capabilities
                        .message_streams
                        .broadcast(&stream_type, payload)
                })();

                match result {
                    Ok(count) => count as i32,
                    Err(_) => -1,
                }
            },
        )
        .unwrap();
}
