use std::any::TypeId;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use fabric_component_namespace::NamespaceName;
use fabric_core::{ContractId, ContractKey};

use crate::{GatewayEntry, GatewayError, GatewayRequest, GatewayResponse};

type ErasedGatewayInput = Box<dyn std::any::Any + Send + Sync>;
type ErasedGatewayOutput = Box<dyn std::any::Any + Send + Sync>;
pub type GatewayFuture<T> = Pin<Box<dyn Future<Output = Result<T, GatewayError>> + Send + 'static>>;

const GATEWAY_CONTRACT_ID: &str = "fabric.component.gateway";

pub fn gateway_contract_id() -> ContractId {
    ContractId::new(GATEWAY_CONTRACT_ID).expect("static gateway contract id")
}

pub fn gateway_contract_key() -> ContractKey<Gateway> {
    ContractKey::provisional(gateway_contract_id())
}

pub trait GatewayService: Send + Sync {
    fn entry(&self, name: &NamespaceName) -> Result<GatewayEntry, GatewayError>;
    fn entries(&self) -> Vec<GatewayEntry>;
    fn invoke_erased(
        &self,
        context: Option<fabric_component::InvocationContext>,
        operation_id: &fabric_component::OperationId,
        input_type: TypeId,
        output_type: TypeId,
        input: ErasedGatewayInput,
    ) -> GatewayFuture<ErasedGatewayOutput>;
}

#[derive(Clone)]
pub struct Gateway {
    inner: Arc<dyn GatewayService>,
}

impl Gateway {
    pub fn new(inner: Arc<dyn GatewayService>) -> Self {
        Self { inner }
    }

    pub fn invoke<I, O>(&self, request: GatewayRequest<I, O>) -> GatewayFuture<GatewayResponse<O>>
    where
        I: Send + Sync + 'static,
        O: Send + Sync + 'static,
    {
        let (operation, input) = request.into_parts();
        let operation_id = operation.id().clone();
        let inner = Arc::clone(&self.inner);
        Box::pin(async move {
            let output = inner
                .invoke_erased(
                    None,
                    &operation_id,
                    TypeId::of::<I>(),
                    TypeId::of::<O>(),
                    Box::new(input),
                )
                .await?;
            let output = output.downcast::<O>().map(|value| *value).map_err(|_| {
                GatewayError::from(fabric_component::ComponentError::OperationTypeMismatch(
                    operation_id,
                ))
            })?;
            Ok(GatewayResponse::new(output))
        })
    }

    pub fn invoke_with_context<I, O>(
        &self,
        context: fabric_component::InvocationContext,
        request: GatewayRequest<I, O>,
    ) -> GatewayFuture<GatewayResponse<O>>
    where
        I: Send + Sync + 'static,
        O: Send + Sync + 'static,
    {
        let (operation, input) = request.into_parts();
        let operation_id = operation.id().clone();
        let inner = Arc::clone(&self.inner);
        Box::pin(async move {
            let output = inner
                .invoke_erased(
                    Some(context),
                    &operation_id,
                    TypeId::of::<I>(),
                    TypeId::of::<O>(),
                    Box::new(input),
                )
                .await?;
            let output = output.downcast::<O>().map(|value| *value).map_err(|_| {
                GatewayError::from(fabric_component::ComponentError::OperationTypeMismatch(
                    operation_id,
                ))
            })?;
            Ok(GatewayResponse::new(output))
        })
    }

    pub fn entry(&self, name: &NamespaceName) -> Result<GatewayEntry, GatewayError> {
        self.inner.entry(name)
    }

    pub fn entries(&self) -> Vec<GatewayEntry> {
        self.inner.entries()
    }
}
