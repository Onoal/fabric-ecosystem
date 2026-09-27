use std::sync::Arc;

use fabric::authoring::{
    ResourceAugmentation, ResourceAugmentationDefinition, ResourceAugmentationSupportDefinition,
};
use fabric::core::{
    Health, ModuleBindings, ModuleContract, ModuleDeclaration, ModuleError, ModuleId, ModuleRuntime,
};
use fabric::prelude::*;
use fabric_package_key_value::{memory_key_value, KeyValue};

struct TestMarkerAugmentation;

#[derive(Clone)]
struct TestMarkerContract;

#[derive(Clone)]
struct TestMarkerSupport;

impl ResourceAugmentationDefinition<KeyValue> for TestMarkerAugmentation {
    type Config = ();
    type Contract = TestMarkerContract;

    fn contract_key() -> fabric::core::ContractKey<Self::Contract> {
        fabric::core::ContractKey::provisional(
            fabric::core::ContractId::new("onoal.package.data.key-value.test-marker".to_owned())
                .expect("static contract id"),
        )
    }
}

impl ResourceAugmentationSupportDefinition<KeyValue, TestMarkerAugmentation> for TestMarkerSupport {
    fn declaration(&self, provider_module_id: ModuleId) -> ModuleDeclaration {
        ModuleDeclaration::new(provider_module_id)
    }

    fn materialize(
        &self,
        _attachment: &ResourceAugmentation<KeyValue, TestMarkerAugmentation>,
        provider_module_id: ModuleId,
    ) -> Option<Box<dyn ModuleRuntime>> {
        Some(Box::new(TestMarkerRuntime {
            module_id: provider_module_id,
        }))
    }
}

struct TestMarkerRuntime {
    module_id: ModuleId,
}

impl ModuleRuntime for TestMarkerRuntime {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(vec![ModuleContract::new(
            &TestMarkerAugmentation::contract_key(),
            Arc::new(TestMarkerContract),
        )])
    }

    fn bind(&mut self, _bindings: &ModuleBindings) -> Result<(), ModuleError> {
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn stop(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn health(&self) -> Health {
        Health::Healthy
    }
}

fn marker_augmented_key_value(name: &'static str) -> impl IntoFabricContribution {
    let selected = KeyValue::select(name).expect("valid KeyValue resource name");
    let marker = ResourceAugmentation::<KeyValue, TestMarkerAugmentation>::attach(&selected, ())
        .expect("valid test marker attachment")
        .using(TestMarkerSupport);

    FabricContribution::new()
        .with(memory_key_value(name))
        .resource_augmentation(marker)
}

#[test]
fn external_augmentation_can_attach_without_becoming_key_value_semantics() {
    let composition = Fabric::new("onoal.package.test.key-value.augmentation")
        .expect("fabric")
        .with(marker_augmented_key_value("primary"))
        .build()
        .expect("composition");

    let resource = composition
        .resources()
        .find(|resource| resource.name().as_str() == "primary")
        .expect("key-value resource");
    assert_eq!(
        resource.resource_id().as_str(),
        "onoal.package.data.key-value"
    );
    assert_eq!(resource.augmentations().count(), 1);

    let mut instance = composition
        .materialize_on(
            "onoal.package.test.key-value.augmentation.instance",
            &HostDescriptor::native(),
        )
        .expect("instance");
    instance.start().expect("start");
    instance.stop().expect("stop");
}
