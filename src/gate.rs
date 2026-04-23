// gewyvern v0.03 - Intervention Safety Gate

#[derive(Debug)]
pub enum GateRefuse {
    IncompleteObservation,
    InsufficientEvidence,
    FlowTerminated,
    PolicyDenied,
}

pub struct InterventionGate;

impl InterventionGate {
    pub fn can_drop(
        scope_complete: bool,
        evidence_count: usize,
        terminated: bool,
    ) -> Result<(), GateRefuse> {
        if !scope_complete {
            return Err(GateRefuse::IncompleteObservation);
        }

        if evidence_count < 10 {
            return Err(GateRefuse::InsufficientEvidence);
        }

        if terminated {
            return Err(GateRefuse::FlowTerminated);
        }

        Ok(())
    }
}
