use fabric_binding::{BindingConsumer, BindingId, BindingName};
use fabric_resource::{ResourceError, ResourceInstanceId};

use crate::database_resource_id;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatabaseRef {
    resource_id: ResourceInstanceId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedDatabase {
    pub resource_id: ResourceInstanceId,
    pub(crate) database_ref: DatabaseRef,
    pub name: fabric_resource::ResourceName,
    pub context: fabric_resource::ResourceContext,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedDatabaseState {
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatabaseBinding {
    pub(crate) consumer: BindingConsumer,
    pub(crate) resource_id: ResourceInstanceId,
    pub(crate) binding_id: BindingId,
    pub(crate) name: BindingName,
}

impl DatabaseRef {
    pub(crate) fn from_resource_id(resource_id: ResourceInstanceId) -> Self {
        Self { resource_id }
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, ResourceError> {
        const PREFIX: &str = "fabric-resource-database-ref-v1:";

        let value = value.into();
        let Some(resource_id) = value.strip_prefix(PREFIX) else {
            return Err(ResourceError::InvalidInput {
                message: "database reference must use the fabric-resource-database-ref-v1 prefix"
                    .to_owned(),
            });
        };

        let resource_id = ResourceInstanceId::parse(resource_id.to_owned())?;
        if resource_id.resource() != database_resource_id() {
            return Err(ResourceError::InvalidInput {
                message: "database reference must point to a database resource".to_owned(),
            });
        }

        Ok(Self { resource_id })
    }

    pub fn encode(&self) -> String {
        format!(
            "fabric-resource-database-ref-v1:{}",
            self.resource_id.as_str()
        )
    }

    pub fn resource_id(&self) -> &ResourceInstanceId {
        &self.resource_id
    }
}

impl PreparedDatabase {
    pub fn database_ref(&self) -> DatabaseRef {
        self.database_ref.clone()
    }
}

impl DatabaseBinding {
    pub fn consumer(&self) -> &BindingConsumer {
        &self.consumer
    }

    pub fn binding_id(&self) -> &BindingId {
        &self.binding_id
    }

    pub fn name(&self) -> &BindingName {
        &self.name
    }

    pub fn resource_id(&self) -> &ResourceInstanceId {
        &self.resource_id
    }

    pub fn database_ref(&self) -> DatabaseRef {
        DatabaseRef::from_resource_id(self.resource_id.clone())
    }
}
