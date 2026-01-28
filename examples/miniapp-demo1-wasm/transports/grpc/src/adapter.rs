use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::{Stream, StreamExt, wrappers::ReceiverStream};
use tonic::{Request, Response, Status, Streaming, transport::Server};
use wasm_service_core::*;

// Include generated proto code
pub mod proto {
    tonic::include_proto!("wasm.service");
}

use proto::wasm_service_server::{WasmService, WasmServiceServer};
use proto::{Error as ProtoError, Request as ProtoRequest, Response as ProtoResponse};

pub struct GrpcTransport {
    runtime: Arc<WasmRuntime>,
    service_name: String,
}

impl GrpcTransport {
    pub fn new(runtime: Arc<WasmRuntime>, service_name: String) -> Self {
        GrpcTransport {
            runtime,
            service_name,
        }
    }

    pub async fn serve(self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        let service = WasmServiceServer::new(self);

        println!("gRPC server listening on {}", addr);

        Server::builder().add_service(service).serve(addr).await?;

        Ok(())
    }

    fn proto_to_canonical(&self, request: ProtoRequest) -> CanonicalRequest {
        let metadata: Vec<(String, String)> = request.metadata.into_iter().collect();

        CanonicalRequest {
            method: request.method.clone(),
            payload: if request.payload.is_empty() {
                None
            } else {
                Some(request.payload)
            },
            input_stream: None,
            metadata,
            context: RequestContext {
                request_id: uuid::Uuid::new_v4().to_string(),
                service_name: self.service_name.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                transport_info: Some(TransportInfo {
                    protocol: "grpc".to_string(),
                    endpoint: request.method,
                }),
            },
        }
    }

    fn canonical_to_proto(&self, response: CanonicalResponse) -> ProtoResponse {
        let error = response.error.map(|e| ProtoError {
            message: e.message,
            code: e.code,
            details: e.details,
        });

        let metadata: std::collections::HashMap<String, String> =
            response.metadata.into_iter().collect();

        ProtoResponse {
            code: response.code,
            payload: response.payload.unwrap_or_default(),
            metadata,
            error,
        }
    }

    fn code_to_grpc_status(code: u32) -> tonic::Code {
        match code {
            0 => tonic::Code::Ok,
            1 => tonic::Code::InvalidArgument,
            2 => tonic::Code::NotFound,
            3 => tonic::Code::Internal,
            4 => tonic::Code::Unauthenticated,
            5 => tonic::Code::PermissionDenied,
            _ => tonic::Code::Unknown,
        }
    }
}

#[tonic::async_trait]
impl WasmService for GrpcTransport {
    async fn call(
        &self,
        request: Request<ProtoRequest>,
    ) -> Result<Response<ProtoResponse>, Status> {
        let proto_req = request.into_inner();
        let canonical_req = self.proto_to_canonical(proto_req);

        match self.runtime.handle_request(canonical_req).await {
            Ok(canonical_resp) => {
                if canonical_resp.code != 0 {
                    if let Some(error) = &canonical_resp.error {
                        return Err(Status::new(
                            Self::code_to_grpc_status(canonical_resp.code),
                            error.message.clone(),
                        ));
                    }
                }

                let proto_resp = self.canonical_to_proto(canonical_resp);
                Ok(Response::new(proto_resp))
            }
            Err(e) => Err(Status::internal(format!("Runtime error: {}", e))),
        }
    }

    type ServerStreamStream = Pin<Box<dyn Stream<Item = Result<ProtoResponse, Status>> + Send>>;

    async fn server_stream(
        &self,
        request: Request<ProtoRequest>,
    ) -> Result<Response<Self::ServerStreamStream>, Status> {
        let proto_req = request.into_inner();
        let canonical_req = self.proto_to_canonical(proto_req);

        // For server streaming, we'd typically return a stream
        // This is a simplified implementation
        match self.runtime.handle_request(canonical_req).await {
            Ok(canonical_resp) => {
                let proto_resp = self.canonical_to_proto(canonical_resp);

                let stream = futures::stream::once(async move { Ok(proto_resp) });

                Ok(Response::new(Box::pin(stream) as Self::ServerStreamStream))
            }
            Err(e) => Err(Status::internal(format!("Runtime error: {}", e))),
        }
    }

    async fn client_stream(
        &self,
        request: Request<Streaming<ProtoRequest>>,
    ) -> Result<Response<ProtoResponse>, Status> {
        let mut stream = request.into_inner();

        // Collect all requests (simplified - in production, handle streaming properly)
        let mut requests = Vec::new();
        while let Some(req) = stream.next().await {
            match req {
                Ok(r) => requests.push(r),
                Err(e) => return Err(e),
            }
        }

        if let Some(first_req) = requests.first() {
            let canonical_req = self.proto_to_canonical(first_req.clone());

            match self.runtime.handle_request(canonical_req).await {
                Ok(canonical_resp) => {
                    let proto_resp = self.canonical_to_proto(canonical_resp);
                    Ok(Response::new(proto_resp))
                }
                Err(e) => Err(Status::internal(format!("Runtime error: {}", e))),
            }
        } else {
            Err(Status::invalid_argument("No requests received"))
        }
    }

    type BidirectionalStreamStream =
        Pin<Box<dyn Stream<Item = Result<ProtoResponse, Status>> + Send>>;

    async fn bidirectional_stream(
        &self,
        request: Request<Streaming<ProtoRequest>>,
    ) -> Result<Response<Self::BidirectionalStreamStream>, Status> {
        let mut in_stream = request.into_inner();
        let (tx, rx) = mpsc::channel(128);

        let runtime = self.runtime.clone();
        let service_name = self.service_name.clone();

        tokio::spawn(async move {
            while let Some(result) = in_stream.next().await {
                match result {
                    Ok(proto_req) => {
                        let metadata: Vec<(String, String)> = proto_req
                            .metadata
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();

                        let canonical_req = CanonicalRequest {
                            method: proto_req.method.clone(),
                            payload: if proto_req.payload.is_empty() {
                                None
                            } else {
                                Some(proto_req.payload.clone())
                            },
                            input_stream: None,
                            metadata,
                            context: RequestContext {
                                request_id: uuid::Uuid::new_v4().to_string(),
                                service_name: service_name.clone(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                transport_info: Some(TransportInfo {
                                    protocol: "grpc".to_string(),
                                    endpoint: proto_req.method,
                                }),
                            },
                        };

                        match runtime.handle_request(canonical_req).await {
                            Ok(canonical_resp) => {
                                let metadata: std::collections::HashMap<String, String> =
                                    canonical_resp.metadata.into_iter().collect();

                                let proto_resp = ProtoResponse {
                                    code: canonical_resp.code,
                                    payload: canonical_resp.payload.unwrap_or_default(),
                                    metadata,
                                    error: canonical_resp.error.map(|e| ProtoError {
                                        message: e.message,
                                        code: e.code,
                                        details: e.details,
                                    }),
                                };

                                if tx.send(Ok(proto_resp)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(Err(Status::internal(format!("Runtime error: {}", e))))
                                    .await;
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        break;
                    }
                }
            }
        });

        let out_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(out_stream) as Self::BidirectionalStreamStream
        ))
    }
}
