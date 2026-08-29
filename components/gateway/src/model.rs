use fabric_component::{OperationKey, Surface};
use fabric_component_namespace::NamespaceName;
use fabric_component_publication::Publication;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayEntry {
    publication: Publication,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayRequest<I, O>
where
    I: Send + Sync + 'static,
    O: Send + Sync + 'static,
{
    operation: OperationKey<I, O>,
    input: I,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayResponse<O>
where
    O: Send + Sync + 'static,
{
    output: O,
}

impl GatewayEntry {
    pub fn new(publication: Publication) -> Self {
        Self { publication }
    }

    pub fn publication(&self) -> &Publication {
        &self.publication
    }

    pub fn name(&self) -> &NamespaceName {
        self.publication.claim().name()
    }

    pub fn surface(&self) -> &Surface {
        self.publication.surface()
    }
}

impl<I, O> GatewayRequest<I, O>
where
    I: Send + Sync + 'static,
    O: Send + Sync + 'static,
{
    pub fn new(operation: OperationKey<I, O>, input: I) -> Self {
        Self { operation, input }
    }

    pub fn operation(&self) -> &OperationKey<I, O> {
        &self.operation
    }

    pub fn into_parts(self) -> (OperationKey<I, O>, I) {
        (self.operation, self.input)
    }
}

impl<O> GatewayResponse<O>
where
    O: Send + Sync + 'static,
{
    pub fn new(output: O) -> Self {
        Self { output }
    }

    pub fn output(&self) -> &O {
        &self.output
    }

    pub fn into_output(self) -> O {
        self.output
    }
}
