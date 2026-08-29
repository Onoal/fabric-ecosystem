use fabric_component::Surface;
use fabric_component_namespace::NamespaceClaim;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Publication {
    claim: NamespaceClaim,
    surface: Surface,
}

impl Publication {
    pub fn new(claim: NamespaceClaim, surface: Surface) -> Self {
        Self { claim, surface }
    }

    pub fn claim(&self) -> &NamespaceClaim {
        &self.claim
    }

    pub fn surface(&self) -> &Surface {
        &self.surface
    }
}
